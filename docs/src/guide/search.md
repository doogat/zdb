# Search & Queries

## Full-Text Search

```bash
zdb search "your query"
```

Searches zettel titles, bodies, and tags using SQLite FTS5 with porter stemming. Results are ranked by relevance with highlighted snippets.

### Example

```bash
zdb search "conflict resolution"
```

Output:

```text
[20260226120000] CRDT Conflict Resolution (zettelkasten/20260226120000.md)
  CRDTs resolve <b>conflict</b>s by ensuring all replicas converge to the same state...
```

The index is automatically rebuilt if stale (Git HEAD has changed since last rebuild).

## Raw SQL Queries

```bash
zdb query "SQL"
```

Execute arbitrary SQL against the index database. Useful for advanced queries combining multiple tables.

### Available Tables

| Table | Columns |
|-------|---------|
| `zettels` | `id`, `title`, `date`, `type`, `path`, `body`, `updated_at` |
| `tags` | `zettel_id`, `tag` |
| `fields` | `zettel_id`, `key`, `value`, `zone` |
| `links` | `source_id`, `target_path`, `display`, `zone` |

### Examples

List all zettels:

```bash
zdb query "SELECT id, title FROM zettels ORDER BY date DESC"
```

Find zettels by tag:

```bash
zdb query "SELECT z.id, z.title FROM zettels z JOIN tags t ON t.zettel_id = z.id WHERE t.tag = 'crdt'"
```

Find backlinks to a zettel:

```bash
zdb query "SELECT z.title FROM zettels z JOIN links l ON l.source_id = z.id WHERE l.target_path = '20260226120000'"
```

Find zettels with a specific inline field:

```bash
zdb query "SELECT z.title, f.value FROM zettels z JOIN fields f ON f.zettel_id = z.id WHERE f.key = 'source'"
```

Count zettels by type:

```bash
zdb query "SELECT type, COUNT(*) FROM zettels GROUP BY type"
```

## Rebuilding the Index

```bash
zdb reindex
```

Forces a full rebuild — parses every zettel and repopulates all tables. The index is derived from Git; it can be safely deleted and rebuilt.
