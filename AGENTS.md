# ZettelDB

Hybrid Git-CRDT decentralized Zettelkasten database. Git is source of truth; SQLite index is derived/rebuildable.

## Stack

- Rust 2021 edition, workspace with three crates
- Git storage via `git2`, CRDT via `automerge`
- SQLite index via `rusqlite` (FTS5), SQL parsing via `sqlparser`
- CLI via `clap` (binary: `zdb`)
- GraphQL server via `axum` + `async-graphql` (dynamic schema)
- FFI via `uniffi` (proc-macro approach, generates Swift/Kotlin bindings)

## Structure

```
zdb-core/src/       Library crate
  parser.rs         Three-zone Markdown parsing (frontmatter/body/references)
  git_ops.rs        Git repository CRUD, merge, remote sync
  crdt_resolver.rs  Automerge CRDT conflict resolution
  indexer.rs        SQLite FTS5 index, type inference, materialization
  sql_engine.rs     SQL DDL/DML translation (tables as zettel types)
  bundled_types.rs  Built-in type templates (project, contact)
  sync_manager.rs   Multi-device sync orchestration
  compaction.rs     CRDT temp cleanup and git gc
  hlc.rs            Hybrid Logical Clock for causal ordering
  traits.rs         Core trait abstractions (ZettelSource, ZettelStore, etc.)
  ffi.rs            UniFFI ZettelDriver facade for Swift/Kotlin bindings
  types.rs          Shared data structures (ZettelId, ParsedZettel, ZettelMeta)
  error.rs          Error types and Result alias
  zdb.udl           UniFFI interface definition (documentation reference)
zdb-core/benches/   Criterion benchmarks
  crud.rs           CRUD operations at 1K zettels
  search.rs         FTS5 search, SQL SELECT, reindex at 1K zettels
zdb-cli/src/        Binary crate (single main.rs)
zdb-server/src/     GraphQL server crate
  lib.rs            Server entrypoint (axum router, actor spawn)
  actor.rs          Thread-safe core bridge (mpsc + oneshot)
  schema.rs         Dynamic GraphQL schema from _typedef zettels
  auth.rs           Bearer token generation + middleware
  config.rs         Server config (~/.config/zetteldb/)
  error.rs          ZettelError → GraphQL error mapping
tests/e2e/          E2E tests (assert_cmd, exercises zdb binary)
tests/smoke.sh      CLI smoke test (init, CRUD, search, SQL, sync, compact)
tests/fixtures/     Test fixtures
dev/bin/             Developer scripts
  release              Version bump, tag, push
  build-xcframework    iOS/macOS XCFramework from UniFFI bindings
  build-android        Android .aar from UniFFI bindings
docs/src/           mdbook documentation (architecture, technical, guide)
```

## Design Principles

Follow SOLID and Clean Architecture principles as adapted for Rust. These are mandatory for all code changes:

- `technical/solid.md` - SOLID principles translated to Rust idioms (traits over inheritance, small focused traits, dependency inversion via generics)
- `technical/clean-architecture.md` - Layer boundaries, dependency direction, I/O at the edges, no panics in library code

## Conventions

- All modules return `error::Result<T>`
- ZettelId: 14-digit timestamp string (YYYYMMDDHHmmss)
- Zettels stored at `zettelkasten/{id}.md`, typedefs at `zettelkasten/_typedef/{id}.md`
- Data dir: `.zdb/`, node file: `.git/zdb-node`, git signature: `zdb`
- Plan documents go in `.local/plans/` (gitignored), NOT in `docs/`
- Git worktrees go in `.local/worktrees/` (gitignored), nowhere else

## Definition of Done

A task is NOT complete unless ALL of these pass:

1. **Tests** — unit tests in the module AND integration/e2e tests in `tests/` (not just unit tests)
2. **Smoke test** — if the change adds a CLI command, server endpoint, or user-facing behavior, add a corresponding scenario to BOTH `tests/smoke.sh` (bash, runs on Linux/macOS) and `tests/smoke.ps1` (PowerShell, runs on Windows) following each file's existing numbered-section + `pass` helper pattern
3. **Docs** — update relevant files in `docs/src/` to reflect any behavioral or API changes
4. **Build** — `cargo clippy --workspace` and `cargo test` both pass
5. **Walkthrough** — update `docs/src/technical/walkthrough.md` (see below)

## Code Walkthrough

After every task, update the code walkthrough at `docs/src/technical/walkthrough.md`. This is a linear, detailed explanation of how the codebase works. Never replace the file wholesale — incrementally update or extend it.

### Process

**NEVER edit `walkthrough.md` directly with Edit/Write tools.** All changes MUST go through `showboat` so code snippets stay executable and verifiable.

1. Read the source files touched by the task plus any related modules
2. Plan a linear walkthrough order that explains how the changed code fits into the whole
3. Run `uvx showboat --help` to learn the tool, then use it to build the walkthrough:
   - `showboat note` for commentary sections
   - `showboat exec` with `cat`, `sed`, `grep`, etc. to include real code snippets
4. Always update in place — add, revise, or reorder sections as needed
5. For updating existing sections: build replacement in a temp file with showboat, then splice into the walkthrough
6. Run `showboat verify` after changes to confirm code blocks still match

### Code block rules

- Use the correct language label: `` ```rust `` for Rust, `` ```toml `` for Cargo files, `` ```bash `` for shell, etc.
- Never use `` ```output `` — if the output is Rust code/config, label it as such
- **Gotcha:** `showboat` generates `` ```output `` labels by default — post-process to the correct language label

## Gotchas

- E2E tests require the `zdb` binary — run `cargo build -p zdb-cli` before `cargo test -p zdb-e2e`
- `head_oid()` returns `CommitHash` (a String newtype, access inner via `.0`), not `git2::Oid`
- `merge_frontmatter` is called from both `resolve_conflicts` and `resolve_append_log`

## Commands

```
cargo build                           Build all
cargo test                            Run all tests (unit + e2e)
cargo test -p zdb-e2e                 Run e2e tests only
cargo bench                           Run criterion benchmarks (CRUD + search)
cargo bench --no-run                  Compile benchmarks without running
cargo build -p zdb-core --features profiling   Build with tracing instrumentation
./tests/smoke.sh                      CLI smoke test
cargo clippy --workspace              Lint
cargo doc --no-deps --document-private-items   Generate rustdoc
cd docs && mdbook build               Build documentation
cd docs && mdbook serve               Serve documentation locally

# UniFFI binding generation
cargo run -p zdb-core --bin uniffi-bindgen -- generate \
  --library target/debug/libzdb_core.dylib \
  --language swift --out-dir out/swift
cargo run -p zdb-core --bin uniffi-bindgen -- generate \
  --library target/debug/libzdb_core.dylib \
  --language kotlin --out-dir out/kotlin
```

## Documentation

Read from `docs/src/` before working on related modules:

- `architecture/overview.md` - System design and data flow
- `architecture/modules.md` - Module responsibilities and boundaries
- `architecture/design-decisions.md` - Key architectural choices
- `technical/data-model.md` - Zettel format, frontmatter schema
- `technical/parser.md` - Three-zone Markdown parsing details
- `technical/git-ops.md` - Git storage layer
- `technical/crdt-resolver.md` - Conflict resolution strategy
- `technical/indexer.md` - SQLite index, FTS5, type inference
- `technical/sql-engine.md` - SQL translation layer
- `technical/sync.md` - Multi-device sync protocol
- `technical/server.md` - GraphQL server architecture and API
- `technical/ffi.md` - UniFFI bindings (ZettelDriver facade)
- `technical/errors.md` - Error handling patterns
- `technical/solid.md` - SOLID principles in Rust
- `technical/clean-architecture.md` - Clean Architecture in Rust
