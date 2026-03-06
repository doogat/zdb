# Doogat ZettelDB

A hybrid Git-CRDT decentralized Zettelkasten database written in Rust.

ZettelDB stores Markdown notes in a Git repository, syncs across personal devices without cloud infrastructure, and automatically resolves conflicting edits using Automerge CRDT.

## Key Properties

- **Git-native** — full version history, human-readable Markdown files, durable storage
- **Decentralized** — no server required; sync via any Git remote (bare repos, SSH, local paths)
- **Offline-first** — all functionality works without network; sync is explicit
- **Automatic conflict resolution** — Git handles non-overlapping edits (>99% of cases); Automerge CRDT resolves the rest at character/field/line level

## Current Status

MVP implementation. The core library (`zdb-core`) and CLI (`zdb`) are functional with 10 modules, 12 CLI commands (+ 2 subcommands), and full two-node sync with conflict resolution. Includes a SQL engine for typed zettel tables, implicit type inference, and bundled type definitions.

## Documentation Structure

This book is split into three parts:

1. **System Architecture** — high-level design, module relationships, data flow, and key design decisions
2. **Technical Design** — detailed documentation of each module's internals, data structures, and algorithms
3. **User Guide** — how to install, configure, and use ZettelDB day-to-day
