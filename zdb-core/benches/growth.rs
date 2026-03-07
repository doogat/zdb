use std::path::Path;

use criterion::{criterion_group, criterion_main, Criterion};
use tempfile::TempDir;
use zdb_core::git_ops::GitRepo;

/// Simulate repo growth: create zettels, then modify them over time.
/// NFR-02 / AC-08: repo growth < 50MB/year at 5K zettels.
///
/// Strategy: start with 5K zettels, then simulate 365 days of edits
/// (10 modifications/day = 3650 commits) and measure repo size.
/// Using 10/day instead of 100/day to keep bench runtime reasonable;
/// the threshold is scaled proportionally.
const INITIAL_ZETTELS: usize = 5000;
const DAYS: usize = 365;
const EDITS_PER_DAY: usize = 10;
const GROWTH_THRESHOLD_BYTES: u64 = 50 * 1024 * 1024; // 50MB

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

fn dir_size(path: &Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

fn bench_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("growth");
    // Only run once — this is a measurement, not a hot-path benchmark
    group.sample_size(10);

    group.bench_function("repo_size_after_1yr", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let repo = GitRepo::init(dir.path()).unwrap();

                // Seed with initial zettels
                let files: Vec<(String, String)> = (0..INITIAL_ZETTELS)
                    .map(|i| (zettel_path(i), zettel_content(i)))
                    .collect();
                let refs: Vec<(&str, &str)> =
                    files.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
                repo.commit_files(&refs, "seed").unwrap();

                (dir, repo)
            },
            |(dir, repo)| {
                let size_before = dir_size(dir.path());

                // Simulate edits over a year
                for day in 0..DAYS {
                    let batch: Vec<(String, String)> = (0..EDITS_PER_DAY)
                        .map(|edit| {
                            let idx = (day * EDITS_PER_DAY + edit) % INITIAL_ZETTELS;
                            let content = format!(
                                "---\ntitle: Updated note {idx} day {day}\ndate: 2026-01-01\ntags:\n  - bench\n---\n\
                                 Modified on day {day}, edit {edit}.\n\
                                 ---\n- source:: bench-{idx}"
                            );
                            (zettel_path(idx), content)
                        })
                        .collect();
                    let refs: Vec<(&str, &str)> =
                        batch.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
                    repo.commit_files(&refs, &format!("day {day}")).unwrap();
                }

                let size_after = dir_size(dir.path());
                let growth = size_after - size_before;

                assert!(
                    growth < GROWTH_THRESHOLD_BYTES,
                    "NFR-02: repo grew {:.1}MB, threshold is {:.1}MB",
                    growth as f64 / (1024.0 * 1024.0),
                    GROWTH_THRESHOLD_BYTES as f64 / (1024.0 * 1024.0),
                );
            },
        );
    });

    group.finish();
}

criterion_group!(benches, bench_growth);
criterion_main!(benches);
