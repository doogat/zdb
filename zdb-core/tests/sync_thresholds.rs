use std::time::Instant;

use tempfile::TempDir;
use zdb_core::git_ops::GitRepo;
use zdb_core::indexer::Index;
use zdb_core::sync_manager::{register_node, SyncManager};

const ZETTEL_COUNT: usize = 5000;
const NFR03_THRESHOLD_MS: u128 = 2000;

fn zettel_content(i: usize) -> String {
    format!(
        "---\ntitle: Note {i}\ndate: 2026-01-01\ntags:\n  - bench\n---\nBody of zettel {i}.\n---\n- source:: bench-{i}"
    )
}

fn zettel_path(i: usize) -> String {
    format!("zettelkasten/{:014}.md", 20260101000000u64 + i as u64)
}

/// NFR-03 target: sync < 2s at 5K on LAN.
/// Currently measured at ~12.6s (release, localhost). Needs optimization.
/// Run with: cargo test --release --test sync_thresholds -- --ignored
#[test]
#[ignore = "NFR-03 not yet met: sync ~12.6s vs 2s target"]
fn nfr03_sync_under_2s_at_5k() {
    let bare = TempDir::new().unwrap();
    let da = TempDir::new().unwrap();
    let db = TempDir::new().unwrap();

    git2::Repository::init_bare(bare.path()).unwrap();

    // Repo A: init, populate, push
    let repo_a = GitRepo::init(da.path()).unwrap();
    let files: Vec<(String, String)> = (0..ZETTEL_COUNT)
        .map(|i| (zettel_path(i), zettel_content(i)))
        .collect();
    let refs: Vec<(&str, &str)> = files.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    repo_a.commit_files(&refs, "seed").unwrap();
    repo_a
        .add_remote("origin", bare.path().to_str().unwrap())
        .unwrap();
    repo_a.push("origin", "master").unwrap();
    register_node(&repo_a, "NodeA").unwrap();

    // Repo B: clone
    let _raw = git2::Repository::clone(bare.path().to_str().unwrap(), db.path()).unwrap();
    let repo_b = GitRepo::open(db.path()).unwrap();
    register_node(&repo_b, "NodeB").unwrap();
    let db_path = db.path().join("index.db");
    let index_b = Index::open(&db_path).unwrap();
    index_b.rebuild(&repo_b).unwrap();

    // A adds 10 new zettels and pushes
    let new_files: Vec<(String, String)> = (ZETTEL_COUNT..ZETTEL_COUNT + 10)
        .map(|i| (zettel_path(i), zettel_content(i)))
        .collect();
    let refs: Vec<(&str, &str)> = new_files
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();
    repo_a.commit_files(&refs, "add 10").unwrap();
    repo_a.push("origin", "master").unwrap();

    // Measure sync on B
    let start = Instant::now();
    let mut mgr = SyncManager::open(&repo_b).unwrap();
    mgr.sync("origin", "master", &index_b).unwrap();
    let elapsed_ms = start.elapsed().as_millis();

    eprintln!("NFR-03: sync took {elapsed_ms}ms (threshold: {NFR03_THRESHOLD_MS}ms)");
    assert!(
        elapsed_ms < NFR03_THRESHOLD_MS,
        "NFR-03: sync took {elapsed_ms}ms, threshold is {NFR03_THRESHOLD_MS}ms"
    );
}
