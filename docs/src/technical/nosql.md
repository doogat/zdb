# NoSQL Index (redb)

ZettelDB includes an optional redb-based key-value index behind the `nosql` feature flag. It complements SQLite (which provides FTS5 full-text search and SQL queries) with fast O(1) key lookups and prefix scans.

## When to use

- **redb**: fast single-zettel lookups by ID, type, or tag; backlink traversal; mobile/embedded scenarios where SQLite overhead is unnecessary
- **SQLite**: full-text search, complex SQL queries, materialized type tables

## Build

```bash
cargo build -p zdb-core --features nosql
```

## Table design

| Table | Key | Value | Purpose |
|-------|-----|-------|---------|
| `zettels` | zettel ID | JSON-serialized `ParsedZettel` | Primary store |
| `by_type` | `{type}/{id}` | empty | Type index for prefix scan |
| `by_tag` | `{tag}/{id}` | empty | Tag index for prefix scan |
| `links` | `{target_id}/{source_id}` | empty | Backlink index |

Secondary tables use composite string keys with "/" separator. Prefix scans on `{prefix}/` efficiently return all matching IDs.

## API

```rust
use zdb_core::nosql::RedbIndex;

let idx = RedbIndex::open(Path::new(".zdb/index.redb"))?;

// Index a zettel (upsert — cleans old secondary entries first)
idx.index_zettel(&parsed_zettel)?;

// Single lookup
let zettel = idx.get("20240101120000")?;

// Prefix scans
let project_ids = idx.scan_by_type("project")?;
let rust_ids = idx.scan_by_tag("rust")?;
let backlink_ids = idx.backlinks("20240102000000")?;

// Remove
idx.remove_zettel("20240101120000")?;

// Full rebuild from git
let count = idx.rebuild(&git_repo)?;
```

## Serialization

Values use JSON (`serde_json`) rather than bincode. This avoids compatibility issues with ZettelDB's polymorphic deserializers (e.g., `ZettelId` accepts both string and integer formats from YAML frontmatter).

## Server Integration

The GraphQL server (`zdb-server`) enables `nosql` by default and provides:

- **Dual-write**: every create/update/delete that touches SQLite also writes to redb. The actor holds an `Option<RedbIndex>` alongside `Index`.
- **REST endpoints** at `/nosql/`:
  - `GET /nosql/:id` — get zettel by ID (O(1) lookup)
  - `GET /nosql?type=<type>` — scan by type prefix
  - `GET /nosql?tag=<tag>` — scan by tag prefix
  - `GET /nosql/:id/backlinks` — backlinks for a zettel

## CLI Integration

The CLI (`zdb-cli`) also enables `nosql` by default:

```bash
zdb get <id>              # fetch zettel by ID via redb
zdb scan --type project   # prefix scan by type
zdb scan --tag rust       # prefix scan by tag
zdb backlinks <id>        # list backlinks
```

CLI NoSQL commands rebuild the redb index on each invocation to ensure consistency with git. The server rebuilds once at startup and keeps in sync via dual-writes.

## Implementation

`zdb-core/src/nosql.rs` — gated behind `#[cfg(feature = "nosql")]`. All operations are single-transaction for consistency. Upserts clean old secondary index entries before re-inserting to handle type/tag/link changes.
