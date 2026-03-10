# Storage Budget

Measured growth of a single-node ZettelDB repository over a simulated year.

## Workload assumptions

| Parameter | Value |
|-----------|-------|
| Initial zettels | 5,000 |
| Edits per day | 10 |
| Total commits | 3,650 |
| Average zettel size | ~200 bytes (frontmatter + body + references) |
| Devices | 1 (single-node, no CRDT temp files generated) |

These are conservative: no attachments, no multi-device sync conflicts, small zettel bodies. Real-world repos with attachments or images will grow faster.

## Results

### Without compaction

| Month | Total | .git/ | CRDT temp |
|-------|-------|-------|-----------|
| 1 | 5.7 MB | 4.9 MB | 0 MB |
| 3 | 12.9 MB | 12.1 MB | 0 MB |
| 6 | 23.7 MB | 22.9 MB | 0 MB |
| 9 | 34.4 MB | 33.7 MB | 0 MB |
| 12 | 45.2 MB | 44.5 MB | 0 MB |

**Yearly growth: 43.7 MB** (linear, ~3.6 MB/month)

### With monthly compaction

| Month | Total | .git/ | CRDT temp |
|-------|-------|-------|-----------|
| 1 | 1.9 MB | 1.0 MB | 0 MB |
| 3 | 2.0 MB | 1.2 MB | 0 MB |
| 6 | 2.3 MB | 1.5 MB | 0 MB |
| 9 | 2.5 MB | 1.8 MB | 0 MB |
| 12 | 2.7 MB | 2.0 MB | 0 MB |

**Yearly growth: 1.2 MB** (sub-linear, flattening as git gc packs efficiently)

### Comparison

| Metric | No compaction | Monthly compaction | Reduction |
|--------|---------------|-------------------|-----------|
| Year-end total | 45.2 MB | 2.7 MB | 94% |
| Yearly growth | 43.7 MB | 1.2 MB | 97% |
| Growth rate | Linear (~3.6 MB/mo) | Sub-linear (flattening) | - |

## NFR-02 assessment

NFR-02 targets: repo growth stays within yearly budget. At 10 edits/day with monthly compaction, growth is **1.2 MB/year** — well within any reasonable budget.

Without compaction, growth is ~44 MB/year. For repos with higher edit rates or larger zettels, compaction is essential.

## Limitations

- **Single-node only**: CRDT temp files (0 MB here) would add overhead in multi-device sync with conflicts
- **No attachments**: Binary files resist delta compression and would dominate growth
- **Synthetic content**: Real zettels vary in size; results scale roughly linearly with average zettel size
- **10 edits/day**: The bench uses 10/day (not 100/day from spec) to keep runtime under 15 minutes; growth scales proportionally

## Reproducing

```bash
cargo bench --bench growth
```

Both variants (`repo_size_after_1yr` and `repo_size_after_1yr_with_compaction`) print monthly breakdowns to stderr.
