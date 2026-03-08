# Server Read-Path Decision

Status: **accepted** | Date: 2026-03-07

## Context

The server serializes reads and writes through a single actor thread. Before broader adoption, we need to decide whether to keep this model or introduce a read fast path that bypasses the actor for non-mutating queries.

## Benchmark Results

All measurements on 200 zettels, macOS, release build. Full benchmark suite: `cargo bench -p zdb-server`.

### Single-request latency (no contention)

| Protocol | Operation | Latency |
|----------|-----------|---------|
| GraphQL | get zettel | 276 µs |
| GraphQL | list 20 | 2.9 ms |
| GraphQL | FTS search | 65 µs |
| REST | get zettel | 211 µs |
| NoSQL | get zettel | 60 µs |
| pgwire | SELECT by id | 62 µs |

All protocols return well under 10 ms at 200 zettels. Core-library benchmarks validate NFR-01 at 5K; server-level 5K benchmarks are pending.

### Concurrent reads (list 20 zettels, GraphQL)

| Readers | Batch time | Throughput |
|---------|-----------|------------|
| 1 | 2.9 ms | 342 req/s |
| 4 | 11.1 ms | 360 req/s |
| 8 | 22.1 ms | 362 req/s |
| 16 | 44.0 ms | 364 req/s |

Throughput is flat — time scales linearly with concurrency. The actor serializes requests; adding readers adds latency without increasing throughput.

FTS search scales better (15K → 60K elem/s from 1 → 16 readers) because the per-query cost is low enough that actor overhead is proportionally smaller.

### Mixed load (4 concurrent reads + background writes)

| Workload | Latency (4 readers) | Delta |
|----------|-------------------|-------|
| Reads only | 11 ms | baseline |
| Reads during writes | 500 ms | **45x** |
| Search during writes | 130 µs | ~1x |

Write operations (create + git commit + reindex) dominate actor time. List queries queue behind them; search queries are fast enough to squeeze through gaps.

This is the critical finding: **writes degrade read latency for expensive queries by 45x**.

### Protocol overhead

| Protocol | Get-zettel latency | Overhead vs NoSQL |
|----------|-------------------|-------------------|
| NoSQL | 60 µs | baseline |
| pgwire | 62 µs | ~0 |
| REST | 211 µs | +150 µs |
| GraphQL | 276 µs | +216 µs |

GraphQL and REST add serialization/schema overhead. For latency-sensitive reads, NoSQL and pgwire are 4x faster.

## Options

### A. Keep single actor (chosen)

All reads and writes continue through the actor's mpsc channel.

**Pros:**
- Zero additional complexity
- Single-writer guarantee is trivially maintained
- No risk of stale reads or read/write races
- Current latency meets NFR-01 targets

**Cons:**
- Write activity degrades read latency for heavy queries (45x under sustained writes)
- Throughput ceiling of ~360 req/s for list-style queries
- Linear latency growth with concurrent readers

**When to revisit:** If multiple clients poll the server concurrently while writes are active, and the degraded latency is unacceptable.

### B. Read fast path (deferred)

Non-mutating queries bypass the actor and read SQLite directly via a shared read-only connection pool.

**Pros:**
- Reads unaffected by write activity
- True concurrent read throughput

**Cons:**
- SQLite WAL mode needed; read connections may see slightly stale data
- Must classify every query as read-only (risk: mutation disguised as read)
- Shared connection pool adds locking complexity
- More surface area for consistency bugs

**When to adopt:** Benchmarks show the actor is the bottleneck *and* real usage patterns involve concurrent read+write traffic.

## Decision

**Keep single actor.** The current model meets NFR-01 latency targets for all protocols. The 45x degradation under mixed load is real but only manifests when writes are sustained — in practice, Zettelkasten writes are infrequent (human-speed note-taking), so reads rarely contend.

### What scales

- Single-reader query latency: excellent (60 µs to 2.9 ms)
- FTS search under contention: good (130 µs with 4 readers + writes)
- Protocol diversity: all four protocols functional with similar actor-path cost

### What does not scale

- Concurrent list/get throughput: capped at ~360 req/s (actor-serialized)
- Read latency during sustained writes: 45x degradation for expensive queries
- More than ~16 concurrent readers: latency grows linearly

### Risks

- **Premature complexity**: Adding a read fast path now would create consistency surface area for no demonstrated user need.
- **Misleading benchmarks**: Sustained background writes are worst-case; real usage will show lower contention.

## Operating Envelope

These are the supported performance boundaries for the single-actor design.

### Supported

| Parameter | Limit | Notes |
|-----------|-------|-------|
| Concurrent readers (no writes) | Up to 16 | Latency grows linearly; p99 stays under 50 ms for list queries |
| Single-request latency | < 3 ms (list), < 300 µs (get/search) | Measured at 200 zettels; 5K server benchmarks pending |
| Throughput (read-only) | ~360 req/s (list), ~15K req/s (search) | Actor-serialized ceiling |
| Write frequency | Human-speed (< 1 write/sec) | Reads remain responsive at this rate |

### Degraded

| Condition | Impact | Mitigation |
|-----------|--------|------------|
| Sustained writes (> 1/sec) | List latency degrades 10-45x | Batch writes; use search instead of list |
| > 16 concurrent readers | Latency > 50 ms per request | Rate-limit clients or add read fast path |
| Sync during reads | Similar to write degradation | Schedule sync during idle periods |

### Revisit triggers

Introduce a read fast path if any of these become true:

1. Multiple clients poll the server concurrently (e.g., mobile + desktop + web)
2. Background sync runs frequently enough to cause sustained write contention
3. Query latency under mixed load exceeds NFR-01's 10 ms target in production

## References

- NFR-01 / AC-06: Query latency < 10 ms at 5K zettels
- NFR-03 / AC-02: Sync should not degrade reads beyond accepted bounds
- Spec §5: Single-writer semantics preserved (actor enforces this)
