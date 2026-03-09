# FFI Bindings

**Source**: `zdb-core/src/ffi.rs`

UniFFI-based foreign function interface exposing ZettelDB to Swift and Kotlin via a high-level `ZettelDriver` facade.

## Architecture

```text
Swift/Kotlin app
      │
      ▼
ZettelDriver (ffi.rs)       ← UniFFI proc-macro boundary
      │
      ├── GitRepo            ← git_ops (storage)
      ├── Index              ← indexer (search/query)
      ├── SyncManager        ← sync_manager (compact)
      └── parser             ← parse/serialize
```

`ZettelDriver` wraps `GitRepo` and `Index` behind `Mutex` for thread safety. All methods take `&self` (shared reference via `Arc` on the foreign side).

## Interface

### Constructor

```rust
ZettelDriver::new(repo_path: String) -> Result<Self, ZdbError>
```

Opens an existing ZettelDB repo at `repo_path`. Opens the Git repo and SQLite index at `.zdb/index.db`.

### CRUD

| Method | Delegates to |
|--------|-------------|
| `create_zettel(content, message)` | `parser::parse` → `repo.commit_file` → `index.index_zettel` |
| `read_zettel(id)` | `index.resolve_path` → `repo.read_file` |
| `update_zettel(id, content, message)` | `index.resolve_path` → `repo.commit_file` → `index.index_zettel` |
| `delete_zettel(id, message)` | `index.resolve_path` → `repo.delete_file` → `index.remove_zettel` |

### Query

| Method | Delegates to |
|--------|-------------|
| `search(query)` | `search_paginated(query, MAX, 0)`, returns hits only |
| `search_paginated(query, limit, offset)` | `index.search_paginated` (FTS5 with LIMIT/OFFSET) |
| `list_zettels()` | `repo.list_zettels` |
| `execute_sql(sql)` | `index.execute_sql` (returns affected rows as string) |

### Attachments

| Method | Delegates to |
|--------|-------------|
| `attach_file(zettel_id, file_path)` | `fs::read` → `attachments::attach_file` |
| `detach_file(zettel_id, filename)` | `attachments::detach_file` |
| `list_attachments(zettel_id)` | `attachments::list_attachments` |

`attach_file` reads the file from disk, detects MIME type from the filename extension, stores the blob under `reference/{id}/`, updates frontmatter, and returns `AttachmentInfo`. Both repo and index locks are held for the duration.

### Maintenance

| Method | Delegates to |
|--------|-------------|
| `reindex()` | `index.rebuild` |
| `compact()` | `SyncManager::open` → `compaction::compact` |

## Error Mapping

`ZdbError` is a UniFFI-exported enum mirroring `ZettelError` variants. Each variant carries a `msg: String`. The `From<ZettelError>` impl maps internal errors to FFI-safe variants:

| ZettelError | ZdbError |
|------------|---------|
| `Git(msg)` | `Git { msg }` |
| `Yaml(msg)` | `Yaml { msg }` |
| `Sql(msg)` | `Sql { msg }` |
| `Io(e)` | `Io { msg: e.to_string() }` |
| `Toml(msg)` | `Config { msg }` |
| `VersionMismatch { repo, driver }` | `VersionMismatch { msg: "..." }` |

## FFI Records

- `SearchResult` — `{ id, title, path, snippet, rank }` (mirrors `types::SearchResult`)
- `PaginatedSearchResult` — `{ hits: Vec<SearchResult>, total_count: u64 }`
- `RebuildReport` — `{ indexed, tables_materialized, types_inferred }` (subset of `types::RebuildReport`, omits warnings)
- `AttachmentInfo` — `{ name, mime, size }` (mirrors `types::AttachmentInfo`)

## Binding Generation

Uses UniFFI proc-macro approach (`uniffi::setup_scaffolding!()` in `lib.rs`). No UDL-based code generation; `src/zdb.udl` is kept as interface documentation.

Generate bindings via the bundled `uniffi-bindgen` binary:

```bash
# Build the cdylib first
cargo build -p zdb-core

# Generate Swift
cargo run -p zdb-core --bin uniffi-bindgen -- generate \
  --library target/debug/libzdb_core.dylib \
  --language swift --out-dir out/swift

# Generate Kotlin
cargo run -p zdb-core --bin uniffi-bindgen -- generate \
  --library target/debug/libzdb_core.dylib \
  --language kotlin --out-dir out/kotlin
```

Output files:
- Swift: `zdb_core.swift`, `zdb_coreFFI.h`, `zdb_coreFFI.modulemap`
- Kotlin: `uniffi/zdb_core/zdb_core.kt`

## Thread Safety

`ZettelDriver` fields are wrapped in `Mutex`:
- `repo: Mutex<GitRepo>` — serializes all git operations
- `index: Mutex<Index>` — serializes all SQLite operations

Methods that need both locks acquire them sequentially and drop the first before acquiring the second where possible (e.g. `read_zettel` resolves path via index, drops index lock, then reads via repo).

## On-Device Verification

### Prerequisites

#### Swift / iOS / macOS

- **Xcode** (full install, not just Command Line Tools)
  - Switch active toolchain: `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`
  - Accept license: `sudo xcodebuild -license accept`
  - First launch: `xcodebuild -runFirstLaunch`
- **Rust targets**:
  ```bash
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-apple-darwin
  ```

#### Kotlin / Android

- **cargo-ndk**: `cargo install cargo-ndk`
- **Android NDK** (via Android Studio SDK Manager or standalone download)
  ```bash
  export ANDROID_NDK_HOME=$HOME/Library/Android/sdk/ndk/<version>
  ```
- **kotlinc**: `brew install kotlin`
- **Rust targets**:
  ```bash
  rustup target add aarch64-linux-android x86_64-linux-android
  ```

### Build

```bash
# XCFramework (iOS + macOS)
dev/bin/build-xcframework

# Android AAR
dev/bin/build-android
```

Both scripts use the `vendored` feature to compile OpenSSL and libgit2 from source for cross-compilation targets.

### Test Results

#### Swift on macOS (2026-03-09)

- **Platform**: macOS 26.2, Apple Silicon (arm64), Xcode 26.3, Swift 6.2
- **XCFramework slices**: ios-arm64, ios-arm64_x86_64-simulator, macos-arm64
- **Tests**: 3/3 passed
  - `testCreateAndReadZettel` — create zettel via FFI, reindex, read back, verify content
  - `testSearch` — create zettel, reindex, FTS5 search by title
  - `testListZettels` — create zettel, verify it appears in listing
- **Note**: Tests run on macOS (not iOS simulator) because `setUp` uses `Process()` for `zdb init`, which is unavailable on iOS. The static library and FFI bindings are verified on the macOS slice of the XCFramework.
