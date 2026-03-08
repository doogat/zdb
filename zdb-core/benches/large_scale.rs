use std::path::Path;

use criterion::{criterion_group, criterion_main, Criterion};
use tempfile::TempDir;
use zdb_core::git_ops::GitRepo;
use zdb_core::indexer::Index;

/// AC-19: query < 50ms at 50K zettels.
/// Separate benchmark target due to long setup time.
const ZETTEL_COUNT: usize = 50_000;

fn zettel_content(i: usize) -> String {
    let word = match i % 5 {
        0 => "architecture",
        1 => "refactoring",
        2 => "deployment",
        3 => "performance",
        _ => "documentation",
    };
    format!(
        "---\ntitle: Note about {word} {i}\ndate: 2026-01-01\ntags:\n  - bench\n  - {word}\n---\n\
         This zettel discusses {word} in the context of item {i}.\n\
         ---\n- source:: bench-{i}"
    )
}

fn zettel_path(i: usize) -> String {
    format!("zettelkasten/{:014}.md", 20260101000000u64 + i as u64)
}

fn populated_repo_and_index(repo_dir: &Path, db_path: &Path) -> (GitRepo, Index) {
    let repo = GitRepo::init(repo_dir).unwrap();

    // Commit in batches to avoid excessive memory usage
    let batch_size = 5000;
    for start in (0..ZETTEL_COUNT).step_by(batch_size) {
        let end = (start + batch_size).min(ZETTEL_COUNT);
        let files: Vec<(String, String)> = (start..end)
            .map(|i| (zettel_path(i), zettel_content(i)))
            .collect();
        let refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(p, c)| (p.as_str(), c.as_str()))
            .collect();
        repo.commit_files(&refs, &format!("batch {start}")).unwrap();
    }

    let index = Index::open(db_path).unwrap();
    index.rebuild(&repo).unwrap();
    (repo, index)
}

fn bench_fts_50k(c: &mut Criterion) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("index.db");
    let repo_dir = dir.path().join("repo");
    let (_repo, index) = populated_repo_and_index(&repo_dir, &db_path);

    let mut group = c.benchmark_group("large_scale");
    group.sample_size(20);

    group.bench_function("fts_50k", |b| {
        b.iter(|| {
            index.search("architecture").unwrap();
        });
    });

    group.bench_function("sql_select_50k", |b| {
        b.iter(|| {
            index
                .query_raw(
                    "SELECT id, title FROM zettels WHERE title LIKE '%architecture%' LIMIT 10",
                )
                .unwrap();
        });
    });

    group.finish();
}

criterion_group!(benches, bench_fts_50k);
criterion_main!(benches);
