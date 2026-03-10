# Phase 2 Exit Gate

## Exit Summary

> **Status**: _pending audit_

_To be filled after inventory is complete._

## Checklist

### Air-Gapped Bundle Protocol (FR-30–FR-33)

| Req | Description | Status | Source | Tests | Docs |
|-----|-------------|--------|--------|-------|------|
| FR-30 | Air-gapped bundle protocol for offline nodes | | | | |
| FR-31 | Bundles contain: Git bundle, node registrations, Git objects | | | | |
| FR-32 | Bundle import triggers standard merge protocol | | | | |
| FR-33 | Bundle export targets specific nodes (delta); `--full` for bootstrapping | | | | |

### REST API (FR-50)

| Req | Description | Status | Source | Tests | Docs |
|-----|-------------|--------|--------|-------|------|
| FR-50 | REST API: /rest/zettels CRUD + query, pagination, FTS, type filtering | | | | |

### NoSQL Interface (FR-53)

| Req | Description | Status | Source | Tests | Docs |
|-----|-------------|--------|--------|-------|------|
| FR-53 | NoSQL (redb) key-value interface with prefix scans, type-extended schema | | | | |

### Storage and Compaction (FR-60–FR-65)

| Req | Description | Status | Source | Tests | Docs |
|-----|-------------|--------|--------|-------|------|
| FR-60 | Selective compaction of CRDT history | | | | |
| FR-61 | Compaction boundary: hash present in ALL active nodes' known_heads | | | | |
| FR-63 | Annual growth within NFR-02 targets with compaction | | | | |
| FR-64 | git gc after pruning (not --aggressive) | | | | |
| FR-64a | Export git bundle backup before compaction | | | | |
| FR-64b | Dry-run mode: report what would be compacted | | | | |
| FR-65 | Frontmatter CRDT separate compaction when all nodes sync past commit | | | | |

### Performance Targets (NFR-01–NFR-03)

| Req | Metric | 5K target | 50K target | Status | Evidence |
|-----|--------|-----------|------------|--------|----------|
| NFR-01 | Query latency (indexed) | < 10 ms | < 50 ms | | |
| NFR-02 | Repository growth | < 50 MB/year | < 200 MB/year | | |
| NFR-03 | Sync time (batch) | < 2 s | < 10 s | | |

### Additional Phase 2 Roadmap Items

| Item | Description | Status | Evidence |
|------|-------------|--------|----------|
| Bundled type: literature-note | Type definition via `zdb type install` | | |
| Bundled type: meeting-minutes | Type definition via `zdb type install` | | |
| Bundled type: kanban | Type definition via `zdb type install` | | |
| Sparse index | Audit libgit2/gitoxide coverage for sparse checkout | | |
| fsmonitor | OS file watchers (FSEvents, inotify) for O(changed) status | | |
| Background maintenance | Scheduled `git maintenance run --auto` | | |
| Multi-device simulation | Automated multi-node sync testing | | |

## Deviation Log

_Items that diverge from the initial spec. Each entry: what changed, why, replacement strategy._

| Req/Item | Deviation | Rationale | Replacement |
|----------|-----------|-----------|-------------|
| | | | |

## Phase 3 Entry Criteria

_To be defined after audit is complete._
