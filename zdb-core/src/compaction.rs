use std::collections::HashMap;
use std::path::Path;

use crate::error::{Result, ZettelError};
use crate::git_ops::GitRepo;
use crate::sync_manager::SyncManager;
use crate::types::CompactionReport;

/// Find the greatest common ancestor commit reachable from all active nodes' known_heads.
/// Stale and retired nodes are excluded from the calculation.
pub fn shared_head(repo: &GitRepo, nodes: &[crate::types::NodeConfig]) -> Result<Option<git2::Oid>> {
    let heads: Vec<git2::Oid> = nodes
        .iter()
        .filter(|n| n.status == crate::types::NodeStatus::Active)
        .filter_map(|n| n.known_heads.first())
        .filter_map(|h| git2::Oid::from_str(h).ok())
        .collect();

    if heads.is_empty() {
        return Ok(None);
    }
    if heads.len() == 1 {
        return Ok(Some(heads[0]));
    }

    // Iteratively compute merge-base across all heads
    let mut base = repo.repo.merge_base(heads[0], heads[1])?;
    for head in &heads[2..] {
        base = repo.repo.merge_base(base, *head)?;
    }

    Ok(Some(base))
}

/// Parse zettel ID from CRDT temp filename.
/// Supports formats: `{oid}_{zettel_id}.crdt`, `{oid}_{zettel_id}_fm.crdt`,
/// and legacy `{oid}` or `{oid}.crdt`.
/// Returns `(oid, zettel_id, is_frontmatter)`.
fn parse_crdt_temp_name(name: &str) -> Option<(git2::Oid, Option<String>, bool)> {
    let stem = name.strip_suffix(".crdt").unwrap_or(name);

    if let Some((oid_part, rest)) = stem.split_once('_') {
        let oid = git2::Oid::from_str(oid_part).ok()?;
        if let Some(zettel_id) = rest.strip_suffix("_fm") {
            Some((oid, Some(zettel_id.to_string()), true))
        } else {
            Some((oid, Some(rest.to_string()), false))
        }
    } else {
        let oid = git2::Oid::from_str(stem).ok()?;
        Some((oid, None, false))
    }
}

/// Remove temporary CRDT files older than the shared sync point.
pub fn cleanup_crdt_temp(repo: &GitRepo, shared_head: Option<git2::Oid>) -> Result<usize> {
    let temp_dir = repo.path.join(".crdt/temp");
    if !temp_dir.exists() {
        return Ok(0);
    }

    let Some(shared_head) = shared_head else {
        return Ok(0);
    };

    let mut removed = 0;
    for entry in std::fs::read_dir(&temp_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".gitkeep" {
            continue;
        }
        let Some((temp_commit_oid, _zettel_id, _is_fm)) = parse_crdt_temp_name(&name) else {
            continue;
        };

        if repo.repo.merge_base(shared_head, temp_commit_oid).ok() == Some(temp_commit_oid) {
            std::fs::remove_file(entry.path())?;
            removed += 1;
        }
    }

    Ok(removed)
}

/// Compact CRDT temp files by grouping per zettel and merging Automerge changes.
/// Returns the number of zettels whose CRDT docs were compacted.
pub fn compact_crdt_docs(repo: &GitRepo) -> Result<usize> {
    let temp_dir = repo.path.join(".crdt/temp");
    if !temp_dir.exists() {
        return Ok(0);
    }

    // Group files by (zettel_id, is_frontmatter) so fm and body compact independently
    let mut by_key: HashMap<(String, bool), Vec<std::path::PathBuf>> = HashMap::new();
    for entry in std::fs::read_dir(&temp_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".gitkeep" {
            continue;
        }
        if let Some((_oid, Some(zettel_id), is_fm)) = parse_crdt_temp_name(&name) {
            by_key
                .entry((zettel_id, is_fm))
                .or_default()
                .push(entry.path());
        }
    }

    let mut compacted = 0;
    for ((zettel_id, is_fm), files) in &by_key {
        if files.len() < 2 {
            continue; // nothing to compact
        }

        // Load all Automerge changes and merge into a single doc
        let mut doc = automerge::AutoCommit::new();
        for file in files {
            if let Ok(data) = std::fs::read(file) {
                if let Ok(other) = automerge::AutoCommit::load(&data) {
                    doc.merge(&mut other.clone())
                        .map_err(|e| ZettelError::Automerge(e.to_string()))?;
                }
            }
        }

        // Save compacted doc with appropriate suffix
        let compacted_data = doc.save();
        let fm_suffix = if *is_fm { "_fm" } else { "" };
        let compacted_name = format!("compacted_{zettel_id}{fm_suffix}.crdt");
        std::fs::write(temp_dir.join(&compacted_name), compacted_data)?;

        // Remove original files
        for file in files {
            let _ = std::fs::remove_file(file);
        }

        compacted += 1;
    }

    Ok(compacted)
}

/// Compact CRDT docs for a single zettel.
pub fn compact_zettel(repo: &GitRepo, zettel_id: &str) -> Result<usize> {
    let temp_dir = repo.path.join(".crdt/temp");
    if !temp_dir.exists() {
        return Ok(0);
    }

    let mut files = Vec::new();
    for entry in std::fs::read_dir(&temp_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some((_oid, Some(zid), _is_fm)) = parse_crdt_temp_name(&name) {
            if zid == zettel_id {
                files.push(entry.path());
            }
        }
    }

    if files.len() < 2 {
        return Ok(0);
    }

    let mut doc = automerge::AutoCommit::new();
    for file in &files {
        if let Ok(data) = std::fs::read(file) {
            if let Ok(other) = automerge::AutoCommit::load(&data) {
                doc.merge(&mut other.clone())
                    .map_err(|e| ZettelError::Automerge(e.to_string()))?;
            }
        }
    }

    let compacted_data = doc.save();
    let compacted_name = format!("compacted_{zettel_id}.crdt");
    std::fs::write(temp_dir.join(&compacted_name), compacted_data)?;

    for file in &files {
        let _ = std::fs::remove_file(file);
    }

    Ok(1)
}

/// Get total size of `.crdt/temp/` directory in bytes.
fn crdt_temp_size(repo: &GitRepo) -> u64 {
    let temp_dir = repo.path.join(".crdt/temp");
    if !temp_dir.exists() {
        return 0;
    }
    std::fs::read_dir(&temp_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

/// Run `git gc` on the repository.
pub fn run_gc(repo_path: &Path) -> Result<bool> {
    let output = std::process::Command::new("git")
        .args(["gc"])
        .current_dir(repo_path)
        .output()
        .map_err(ZettelError::Io)?;

    Ok(output.status.success())
}


/// Full compaction pipeline: threshold check → shared head → cleanup → crdt doc compact → gc.
#[cfg_attr(feature = "profiling", tracing::instrument(skip_all))]
pub fn compact(repo: &GitRepo, sync_mgr: &SyncManager, force: bool) -> Result<CompactionReport> {
    // Threshold check: skip if under threshold (unless forced)
    if !force {
        let config = repo.load_config()?;
        let size_mb = crdt_temp_size(repo) as f64 / (1024.0 * 1024.0);
        if size_mb < config.compaction.threshold_mb as f64 {
            tracing::debug!(size_mb, threshold_mb = config.compaction.threshold_mb, "below_threshold_skip");
            return Ok(CompactionReport {
                files_removed: 0,
                crdt_docs_compacted: 0,
                gc_success: true,
            });
        }
    }

    let nodes = sync_mgr.list_nodes()?;
    let head = shared_head(repo, &nodes)?;
    tracing::debug!(shared_head = ?head, node_count = nodes.len(), "shared_head_computed");
    let files_removed = cleanup_crdt_temp(repo, head)?;
    if files_removed > 0 {
        tracing::info!(files_removed, "crdt_temp_cleanup");
    }

    let crdt_docs_compacted = compact_crdt_docs(repo)?;
    if crdt_docs_compacted > 0 {
        tracing::info!(crdt_docs_compacted, "crdt_docs_compacted");
    }

    let gc_success = run_gc(&repo.path)?;
    tracing::info!(gc_success, "gc_result");

    Ok(CompactionReport {
        files_removed,
        crdt_docs_compacted,
        gc_success,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use automerge::transaction::Transactable;

    fn temp_repo() -> (tempfile::TempDir, GitRepo) {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = GitRepo::init(dir.path()).unwrap();
        (dir, repo)
    }

    #[test]
    fn gc_runs_on_test_repo() {
        let (dir, _repo) = temp_repo();
        let success = run_gc(dir.path()).unwrap();
        assert!(success);
    }

    #[test]
    fn cleanup_empty_temp() {
        let (_dir, repo) = temp_repo();
        let removed = cleanup_crdt_temp(&repo, None).unwrap();
        assert_eq!(removed, 0);
    }

    #[test]
    fn cleanup_removes_temp_files() {
        let (_dir, repo) = temp_repo();
        let c1 = repo.commit_file("zettelkasten/a.md", "a", "c1").unwrap();
        let c2 = repo.commit_file("zettelkasten/b.md", "b", "c2").unwrap();
        let c3 = repo.commit_file("zettelkasten/c.md", "c", "c3").unwrap();
        let temp_dir = repo.path.join(".crdt/temp");
        std::fs::write(temp_dir.join(c1.0.clone()), "data").unwrap();
        std::fs::write(temp_dir.join(c2.0.clone()), "data").unwrap();
        std::fs::write(temp_dir.join(c3.0.clone()), "data").unwrap();

        let c2_oid = git2::Oid::from_str(&c2.0).unwrap();
        let removed = cleanup_crdt_temp(&repo, Some(c2_oid)).unwrap();
        assert_eq!(removed, 2);
        assert!(!temp_dir.join(&c1.0).exists());
        assert!(!temp_dir.join(&c2.0).exists());
        assert!(temp_dir.join(&c3.0).exists());
    }

    #[test]
    fn cleanup_handles_new_naming_format() {
        let (_dir, repo) = temp_repo();
        let c1 = repo.commit_file("zettelkasten/a.md", "a", "c1").unwrap();
        let c2 = repo.commit_file("zettelkasten/b.md", "b", "c2").unwrap();
        let temp_dir = repo.path.join(".crdt/temp");

        // New format: {oid}_{zettel_id}.crdt
        std::fs::write(temp_dir.join(format!("{}_20260301120000.crdt", c1.0)), "data").unwrap();
        std::fs::write(temp_dir.join(format!("{}_20260301120100.crdt", c2.0)), "data").unwrap();

        let c2_oid = git2::Oid::from_str(&c2.0).unwrap();
        let removed = cleanup_crdt_temp(&repo, Some(c2_oid)).unwrap();
        assert_eq!(removed, 2);
    }

    #[test]
    fn parse_crdt_temp_name_formats() {
        // Legacy: bare OID
        let (oid, zid, is_fm) = parse_crdt_temp_name("abc123def456abc123def456abc123def456abcd").unwrap();
        assert!(zid.is_none());
        assert!(!is_fm);
        assert_eq!(oid.to_string(), "abc123def456abc123def456abc123def456abcd");

        // Legacy: OID.crdt
        let (_, zid, is_fm) = parse_crdt_temp_name("abc123def456abc123def456abc123def456abcd.crdt").unwrap();
        assert!(zid.is_none());
        assert!(!is_fm);

        // New: OID_zettelid.crdt
        let (_, zid, is_fm) = parse_crdt_temp_name("abc123def456abc123def456abc123def456abcd_20260301120000.crdt").unwrap();
        assert_eq!(zid.as_deref(), Some("20260301120000"));
        assert!(!is_fm);

        // Frontmatter: OID_zettelid_fm.crdt
        let (_, zid, is_fm) = parse_crdt_temp_name("abc123def456abc123def456abc123def456abcd_20260301120000_fm.crdt").unwrap();
        assert_eq!(zid.as_deref(), Some("20260301120000"));
        assert!(is_fm);
    }

    #[test]
    fn compact_crdt_docs_groups_by_zettel() {
        let (_dir, repo) = temp_repo();
        let c1 = repo.commit_file("zettelkasten/a.md", "a", "c1").unwrap();

        std::thread::sleep(std::time::Duration::from_secs(1));
        let c2 = repo.commit_file("zettelkasten/b.md", "b", "c2").unwrap();
        let temp_dir = repo.path.join(".crdt/temp");

        // Create dummy automerge docs for the same zettel
        let mut doc1 = automerge::AutoCommit::new();
        doc1.put(automerge::ROOT, "key", "val1").unwrap();
        std::fs::write(temp_dir.join(format!("{}_20260301120000.crdt", c1.0)), doc1.save()).unwrap();

        let mut doc2 = automerge::AutoCommit::new();
        doc2.put(automerge::ROOT, "key", "val2").unwrap();
        std::fs::write(temp_dir.join(format!("{}_20260301120000.crdt", c2.0)), doc2.save()).unwrap();

        let compacted = compact_crdt_docs(&repo).unwrap();
        assert_eq!(compacted, 1);

        // Should have one compacted file
        let files: Vec<_> = std::fs::read_dir(&temp_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy() != ".gitkeep")
            .collect();
        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().to_string_lossy().starts_with("compacted_"));
    }

    #[test]
    fn compact_zettel_targets_single_zettel() {
        let (_dir, repo) = temp_repo();
        let c1 = repo.commit_file("zettelkasten/a.md", "a", "c1").unwrap();

        std::thread::sleep(std::time::Duration::from_secs(1));
        let c2 = repo.commit_file("zettelkasten/b.md", "b", "c2").unwrap();
        let temp_dir = repo.path.join(".crdt/temp");

        // Zettel A: two files
        let mut doc = automerge::AutoCommit::new();
        doc.put(automerge::ROOT, "k", "v").unwrap();
        std::fs::write(temp_dir.join(format!("{}_A.crdt", c1.0)), doc.save()).unwrap();
        std::fs::write(temp_dir.join(format!("{}_A.crdt", c2.0)), doc.save()).unwrap();

        // Zettel B: one file (should not be touched)
        std::fs::write(temp_dir.join(format!("{}_B.crdt", c1.0)), doc.save()).unwrap();

        let compacted = compact_zettel(&repo, "A").unwrap();
        assert_eq!(compacted, 1);

        // B's file should still exist
        assert!(temp_dir.join(format!("{}_B.crdt", c1.0)).exists());
    }

    #[test]
    fn threshold_check_skips_when_under() {
        let (_dir, repo) = temp_repo();
        crate::sync_manager::register_node(&repo, "Test").unwrap();
        let mgr = SyncManager::open(&repo).unwrap();

        // No CRDT files → under threshold → should skip but still report success
        let report = compact(&repo, &mgr, false).unwrap();
        assert_eq!(report.files_removed, 0);
        assert_eq!(report.crdt_docs_compacted, 0);
        assert!(report.gc_success);
    }

    #[test]
    fn full_compact_pipeline() {
        let (_dir, repo) = temp_repo();
        crate::sync_manager::register_node(&repo, "Test").unwrap();
        let mgr = SyncManager::open(&repo).unwrap();

        let report = compact(&repo, &mgr, true).unwrap();
        assert!(report.gc_success);
    }

    #[test]
    fn compact_crdt_docs_separates_fm_and_body() {
        let (_dir, repo) = temp_repo();
        let c1 = repo.commit_file("zettelkasten/a.md", "a", "c1").unwrap();

        std::thread::sleep(std::time::Duration::from_secs(1));
        let c2 = repo.commit_file("zettelkasten/b.md", "b", "c2").unwrap();
        let temp_dir = repo.path.join(".crdt/temp");

        let mut doc = automerge::AutoCommit::new();
        doc.put(automerge::ROOT, "k", "v").unwrap();
        let bytes = doc.save();

        // Two body files for same zettel
        std::fs::write(temp_dir.join(format!("{}_20260301120000.crdt", c1.0)), &bytes).unwrap();
        std::fs::write(temp_dir.join(format!("{}_20260301120000.crdt", c2.0)), &bytes).unwrap();
        // Two fm files for same zettel
        std::fs::write(temp_dir.join(format!("{}_20260301120000_fm.crdt", c1.0)), &bytes).unwrap();
        std::fs::write(temp_dir.join(format!("{}_20260301120000_fm.crdt", c2.0)), &bytes).unwrap();

        let compacted = compact_crdt_docs(&repo).unwrap();
        // Should compact body and fm independently → 2 groups compacted
        assert_eq!(compacted, 2);

        let files: Vec<String> = std::fs::read_dir(&temp_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n != ".gitkeep")
            .collect();
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f == "compacted_20260301120000.crdt"));
        assert!(files.iter().any(|f| f == "compacted_20260301120000_fm.crdt"));
    }

    #[test]
    fn cleanup_handles_fm_naming_format() {
        let (_dir, repo) = temp_repo();
        let c1 = repo.commit_file("zettelkasten/a.md", "a", "c1").unwrap();
        let c2 = repo.commit_file("zettelkasten/b.md", "b", "c2").unwrap();
        let temp_dir = repo.path.join(".crdt/temp");

        // Create _fm.crdt files
        std::fs::write(temp_dir.join(format!("{}_20260301120000_fm.crdt", c1.0)), "data").unwrap();
        std::fs::write(temp_dir.join(format!("{}_20260301120000_fm.crdt", c2.0)), "data").unwrap();

        let c2_oid = git2::Oid::from_str(&c2.0).unwrap();
        let removed = cleanup_crdt_temp(&repo, Some(c2_oid)).unwrap();
        assert_eq!(removed, 2);
    }

}
