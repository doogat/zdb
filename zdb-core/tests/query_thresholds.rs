use std::time::Instant;

use tempfile::TempDir;
use zdb_core::git_ops::GitRepo;
use zdb_core::indexer::Index;

const ZETTEL_COUNT: usize = 5000;
const NFR01_THRESHOLD_MS: u128 = 10;
const WARMUP_ITERS: usize = 3;
const MEASURE_ITERS: usize = 10;

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
         Some additional body text for search indexing.\n\
         ---\n- source:: bench-{i}"
    )
}

fn zettel_path(i: usize) -> String {
    format!("zettelkasten/{:014}.md", 20260101000000u64 + i as u64)
}

fn setup_5k() -> (TempDir, Index) {
    let dir = TempDir::new().unwrap();
    let repo_dir = dir.path().join("repo");
    let db_path = dir.path().join("index.db");

    let repo = GitRepo::init(&repo_dir).unwrap();
    let files: Vec<(String, String)> = (0..ZETTEL_COUNT)
        .map(|i| (zettel_path(i), zettel_content(i)))
        .collect();
    let refs: Vec<(&str, &str)> = files.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    repo.commit_files(&refs, "seed").unwrap();

    let index = Index::open(&db_path).unwrap();
    index.rebuild(&repo).unwrap();
    (dir, index)
}

fn median_ms<F: FnMut()>(mut f: F) -> u128 {
    // warmup
    for _ in 0..WARMUP_ITERS {
        f();
    }
    // measure
    let mut times = Vec::with_capacity(MEASURE_ITERS);
    for _ in 0..MEASURE_ITERS {
        let start = Instant::now();
        f();
        times.push(start.elapsed().as_millis());
    }
    times.sort();
    times[MEASURE_ITERS / 2]
}

#[test]
fn nfr01_fts_query_under_10ms_at_5k() {
    let (_dir, index) = setup_5k();
    let ms = median_ms(|| {
        index.search("architecture").unwrap();
    });
    assert!(
        ms < NFR01_THRESHOLD_MS,
        "NFR-01: FTS query took {ms}ms, threshold is {NFR01_THRESHOLD_MS}ms"
    );
}

#[test]
fn nfr01_sql_query_under_10ms_at_5k() {
    let (_dir, index) = setup_5k();
    let ms = median_ms(|| {
        index
            .query_raw("SELECT id, title FROM zettels WHERE title LIKE '%architecture%' LIMIT 10")
            .unwrap();
    });
    assert!(
        ms < NFR01_THRESHOLD_MS,
        "NFR-01: SQL query took {ms}ms, threshold is {NFR01_THRESHOLD_MS}ms"
    );
}
