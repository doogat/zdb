# Sync & Compaction

## Sync Manager

**Source**: `zdb-core/src/sync_manager.rs` (198 lines)

Orchestrates multi-device synchronization.

### SyncManager

```rust
pub struct SyncManager<'a> {
    pub repo: &'a GitRepo,
    pub node: NodeConfig,
}
```

### Node Registration

`register_node(repo, name) -> Result<NodeConfig>`

1. Generate UUIDv4
2. Create `NodeConfig` with name, uuid, empty `known_heads`
3. Write `.nodes/{uuid}.toml` (Git-tracked)
4. Write UUID to `.git/zdb-node` (local-only, not tracked)
5. Commit the `.nodes/` file

The local `.git/zdb-node` file identifies which node this device is. It must exist for `SyncManager::open()` to work.

### Opening

`SyncManager::open(repo) -> Result<Self>`

Reads the UUID from `.git/zdb-node`, then loads the corresponding `.nodes/{uuid}.toml` from the Git tree.

### Full Sync Cycle

`sync(remote, branch, index) -> Result<SyncReport>`

1. **Fetch**: `git fetch {remote} {branch}`
2. **Merge**: `merge_remote(remote, branch)` → get `MergeResult`
3. **Handle result**:
   - `AlreadyUpToDate` → report "up-to-date", skip push
   - `FastForward` → report 1 commit transferred
   - `Clean` → report 1 commit transferred (Git auto-committed)
   - `Conflicts` → three-step merge cascade (see below)
4. **Push**: `git push {remote} {branch}` (skip if up-to-date)
5. **Update state**: set `known_heads = [current HEAD]`, `last_sync = now`, commit `.nodes/{uuid}.toml`
6. **Push again**: propagate updated node registry
7. **Reindex**: `index.rebuild(repo)` to keep search current

### Three-Step Merge Cascade

When conflicts occur (or a clean merge produces invalid output):

1. **Step 1: Git merge** — already performed by `merge_remote()`. If clean, validate affected files with `parser::parse()`. Invalid → extract pre-merge versions, fall through.
2. **Step 2: CRDT resolve** — call `resolve_conflicts()` with the typedef's `crdt_strategy` (or repo `default_strategy`). Validate result. Invalid or error → fall through.
3. **Step 3: LWW by HLC** — whole-file last-writer-wins using HLC comparison. Always produces a valid file.

This replaces the previous "ours-wins" fallback with a proper HLC-based resolution.

### State Update

`update_sync_state() -> Result<()>`

Sets the node's `known_heads` to the current HEAD OID and `last_sync` to the current UTC timestamp (RFC3339). Commits the updated `.nodes/{uuid}.toml`.

This propagates to other nodes on their next fetch, allowing compaction to compute the shared head.

### Listing Nodes

`list_nodes() -> Result<Vec<NodeConfig>>`

Walks `.nodes/*.toml` files in the HEAD tree, deserializes each into `NodeConfig`.

## Hybrid Logical Clocks

**Source**: `zdb-core/src/hlc.rs`

HLC combines wall clock time, a logical counter, and node ID for causally-ordered timestamps across devices.

```rust
pub struct Hlc {
    pub wall_ms: u64,   // wall clock milliseconds since UNIX epoch
    pub counter: u32,   // logical counter for same-millisecond events
    pub node: String,   // first 8 chars of node UUID (deterministic tie-break)
}
```

### Operations

- **`Hlc::now(node_id, &last)`** — tick for local event: `max(wall_clock, last.wall_ms)`, increment counter if equal
- **`Hlc::recv(node_id, &local_last, &remote)`** — merge on receive: `max(wall, local, remote)`, bump counter on ties
- **`Hlc::parse(s)` / `Display`** — sortable format: `{wall_ms}-{counter:04}-{node}`
- **`Ord`** — compare wall_ms → counter → node (deterministic total order)

### Integration

- **Commit trailers**: merge commits include `\n\nHLC: {hlc}` trailer, parsed via `extract_hlc()`
- **SyncManager**: `tick_hlc()` on local merge, `recv_hlc()` on remote merge, persisted in `NodeConfig.hlc`
- **ConflictFile**: HLC fields populated from commit trailers for LWW resolution. `extract_conflicts()` calls `find_hlc_for_path()` to walk commit ancestry and extract HLC from the most recent commit touching each conflicting path. `validate_clean_merge_or_fallback()` does the same for post-merge validation conflicts.

## Compaction

**Source**: `zdb-core/src/compaction.rs`

Cleans up temporary CRDT files, merges per-zettel CRDT docs, and runs Git garbage collection. Reports before/after storage measurements.

### Shared Head Calculation

`shared_head(repo, nodes) -> Result<Option<Oid>>`

Finds the greatest common ancestor (GCA) commit across all **active** nodes' `known_heads`. Stale and retired nodes are excluded — this allows compaction to proceed even when some devices are offline.

1. Collect the first `known_head` from each active node
2. If only one node, return its head directly
3. Iteratively compute `merge_base()` across all heads

The shared head represents the latest commit that all active devices have synced. Anything before it is safe to compact.

### CRDT Temp Cleanup

`cleanup_crdt_temp(repo, shared_head) -> Result<usize>`

Removes files in `.crdt/temp/` whose commit OID is an ancestor of the shared head (i.e., all devices have already applied those changes). Preserves `.gitkeep` and files newer than the shared head.

### CRDT Doc Compaction

`compact_crdt_docs(repo) -> Result<usize>`

Groups remaining CRDT temp files by `(zettel_id, is_frontmatter)` and merges multiple Automerge documents per group into a single compacted doc. Body and frontmatter are compacted independently.

### Git GC

`run_gc(repo_path) -> Result<bool>`

Runs `git gc` (not `--aggressive`) for pack consolidation and object deduplication.

### Full Pipeline

`compact(repo, sync_mgr, force) -> Result<CompactionReport>`

1. **Threshold check**: skip if `.crdt/temp/` < `threshold_mb` (unless `force`)
2. Measure `.crdt/temp/` size and file count (before)
3. Measure `.git/` directory size (before)
4. Compute shared head from active nodes
5. Clean up CRDT temp files older than shared head
6. Compact CRDT docs per zettel
7. Measure `.crdt/temp/` size and file count (after)
8. Run `git gc`
9. Measure `.git/` directory size (after)

### CompactionReport

```rust
pub struct CompactionReport {
    pub files_removed: usize,        // temp files deleted in step 5
    pub crdt_docs_compacted: usize,  // zettels merged in step 6
    pub gc_success: bool,            // git gc exit status
    pub crdt_temp_bytes_before: u64, // .crdt/temp/ bytes before cleanup
    pub crdt_temp_bytes_after: u64,  // .crdt/temp/ bytes after compaction
    pub crdt_temp_files_before: usize,
    pub crdt_temp_files_after: usize,
    pub repo_bytes_before: u64,      // .git/ bytes before gc
    pub repo_bytes_after: u64,       // .git/ bytes after gc
}
```

## Test Coverage

### Sync Manager (4 tests)
- Register and open node
- List nodes
- Open without registration fails
- Sync state update

### Compaction (4 tests)
- GC runs successfully
- Cleanup empty temp directory
- Cleanup removes temp files (preserves `.gitkeep`)
- Full compact pipeline

### Integration (2 tests in `tests/sync_test.rs`)
- Two-node sync without conflicts
- Two-node sync with conflict resolution (both nodes reach identical state)

### Multi-device simulation (6 tests in `tests/e2e/multi_device.rs`)
- 3-node convergence (all edit, sync round-robin, verify identical state)
- Concurrent creates (all nodes create simultaneously, sync cascade)
- Stale node return (one node offline, others sync, stale returns)
- Network partition and reconnect (split groups, both edit, reconnect and merge)
- Bundle full bootstrap (export full from node, import on fresh node)
- Air-gapped delta transfer (export bundle, import on disconnected node)
- Stale node resync after compaction (conflict with compacted CRDT state, LWW fallback)
