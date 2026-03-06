# SOLID Principles in Rust

SOLID originated in OOP, but the principles translate well to Rust — often enforced more strictly by the compiler than by convention.

## Single Responsibility (S)

Each module owns one concern. The natural unit in Rust is the module and its public API, not a class.

ZettelDB modules follow this: `parser.rs` parses, `git_ops.rs` handles git, `indexer.rs` handles SQLite. Each module exposes a focused public API and hides implementation details.

## Open/Closed (O)

In OOP: extend via inheritance without modifying. In Rust: extend via **traits** and **generics**.

Instead of subclassing, define a trait and let callers provide implementations. For example, a storage backend trait rather than hardcoding `git2` calls directly. New backends can be added without modifying existing code.

```rust
// Good: open for extension
trait ZettelSource {
    fn get(&self, id: &ZettelId) -> Result<ParsedZettel>;
    fn list(&self) -> Result<Vec<ZettelId>>;
}

// Bad: hardcoded to one implementation
fn rebuild_index(repo_path: &Path) -> Result<()> {
    let repo = git2::Repository::open(repo_path)?;
    // ...
}
```

## Liskov Substitution (L)

Any type implementing a trait must honor that trait's contract. Rust enforces the structural part at compile time — type signatures, lifetimes, Send/Sync bounds. The semantic part (behavior contracts) is still on you: document trait invariants.

```rust
/// Implementors MUST return zettels sorted by id.
/// Returning unsorted results violates the contract.
trait ZettelSource {
    fn list_sorted(&self) -> Result<Vec<ZettelId>>;
}
```

## Interface Segregation (I)

Don't force implementors to provide methods they don't need. Prefer **small, focused traits** over fat ones.

```rust
// Good: separated concerns
trait ReadStore {
    fn get(&self, id: &ZettelId) -> Result<ParsedZettel>;
}

trait WriteStore {
    fn put(&self, zettel: &ParsedZettel) -> Result<()>;
}

// Bad: forces read-only consumers to stub out writes
trait Store {
    fn get(&self, id: &ZettelId) -> Result<ParsedZettel>;
    fn put(&self, zettel: &ParsedZettel) -> Result<()>;
    fn delete(&self, id: &ZettelId) -> Result<()>;
}
```

## Dependency Inversion (D)

High-level modules depend on abstractions (traits), not concretions. Accept `impl Trait` or generic `T: Trait` parameters instead of concrete types.

```rust
// Good: indexer depends on an abstraction
fn rebuild_index(source: &impl ZettelSource, db: &Connection) -> Result<()> {
    for id in source.list()? {
        let zettel = source.get(&id)?;
        insert_into_index(db, &zettel)?;
    }
    Ok(())
}

// Bad: indexer directly calls git_ops
fn rebuild_index(repo_path: &Path, db: &Connection) -> Result<()> {
    let zettels = git_ops::list_all(repo_path)?;
    // ...
}
```

This enables testing the indexer without a real git repo — pass a mock `ZettelSource` instead.

## What the Rust compiler already enforces

| SOLID concern | Rust mechanism |
|---|---|
| Hidden mutation | Ownership + borrow checker |
| Inheritance spaghetti | No inheritance — traits only |
| Visibility | `pub`/`pub(crate)`/private modules |
| Contract violations | Type system, lifetimes, Send/Sync |

The remaining work is **architectural discipline** — the compiler can't check that your module boundaries make sense or that your traits are well-scoped.
