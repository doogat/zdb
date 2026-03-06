# Creating & Managing Zettels

## Create

```bash
zdb create --title "Note Title" [--tags "tag1,tag2"] [--type "permanent"] [--body "Content"]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--title` | Yes | Zettel title |
| `--tags` | No | Comma-separated tags |
| `--type` | No | Zettel type (e.g., permanent, literature, fleeting) |
| `--body` | No | Body text (default: empty) |

The command:
1. Generates a timestamp ID
2. Serializes to Markdown with YAML frontmatter
3. Commits to Git
4. Indexes in SQLite

Returns the ID on stdout.

### Example

```bash
zdb create --title "CRDT Conflict Resolution" \
  --tags "distributed-systems,crdt" \
  --type "permanent" \
  --body "CRDTs resolve conflicts by..."
```

## Read

```bash
zdb read <ID>
```

Prints the raw Markdown content from the Git tree.

## Update

```bash
zdb update <ID> [--title "New Title"] [--tags "new,tags"] [--type "new-type"] [--body "New body"]
```

Only specified fields are changed. Unspecified fields remain as-is. The update:
1. Reads the current zettel from Git
2. Parses it
3. Applies changes
4. Re-serializes to Markdown
5. Commits
6. Updates the index

### Example

```bash
zdb update 20260226153042 --title "Updated Title" --tags "revised,learning"
```

## Delete

```bash
zdb delete <ID>
```

Removes the zettel from Git (as a new commit) and the SQLite index. No output on success; exits non-zero if the ID doesn't exist.

Recoverable: since deletion is a Git commit, `git revert <commit>` restores the zettel.

## Editing Content Directly

For longer edits, modify the Markdown file directly:

```bash
$EDITOR zettelkasten/20260226153042.md
git add zettelkasten/20260226153042.md
git commit -m "edit zettel 20260226153042"
zdb reindex
```

After editing files manually, run `reindex` to update the search index. The index detects staleness automatically on `search` and `query` commands, but `reindex` forces an immediate rebuild.

## Tags

Tags are YAML list items in frontmatter:

```yaml
tags:
  - distributed-systems
  - crdt
  - client/acme
```

Hierarchical tags use `/` separators. The `by_tag` query supports prefix matching (e.g., `client/` matches all client tags).

## Types

Zettels can have a `type` field for structured data. Typed zettels are materialized into queryable SQLite tables during reindex.

```bash
zdb create --title "Ship v1" --type "project" --body "## Description\n\nShip the first version"
```

See [Type Definitions](./types.md) for creating type schemas and using bundled types.

## Wikilinks

Link to other zettels using `[[target]]` or `[[target|Display Text]]`:

```markdown
See [[20260226120000]] for details.
Related: [[20260101000000|Original Research]]
```

Wikilinks work in all three zones (frontmatter values, body, reference section). They're indexed for backlink queries.
