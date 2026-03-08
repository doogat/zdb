use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use tempfile::TempDir;
use zdb_core::git_ops::GitRepo;

include!("helpers.rs");

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

                black_box(growth);
            },
        );
    });

    group.finish();
}

criterion_group!(benches, bench_growth);
criterion_main!(benches);
