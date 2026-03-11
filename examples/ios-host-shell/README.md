# iOS Host-Shell Example

A SwiftUI host-shell app demonstrating two mini-app modules (Bookmarks and Contacts) sharing one embedded ZettelDB core.

## Prerequisites

1. Build the ZettelDB XCFramework:
   ```bash
   cd ../../
   dev/bin/build-xcframework
   ```

2. Generate Swift bindings:
   ```bash
   cargo run -p zdb-core --bin uniffi-bindgen -- generate \
     --library target/debug/libzdb_core.dylib \
     --language swift --out-dir examples/ios-host-shell/HostShell/Sources/Shared
   ```

3. Open in Xcode:
   ```bash
   open HostShell.xcodeproj
   ```

4. Add the XCFramework to the project (drag `out/ZdbCore.xcframework` into Xcode).

## Architecture

```
HostShellApp
├── AppState (owns ZettelDriver)
├── BookmarksModule
│   ├── bootstrap() — CREATE TABLE bookmark, category
│   ├── BookmarkListView
│   └── BookmarkDetailView
├── ContactsModule
│   ├── bootstrap() — CREATE TABLE contact
│   ├── ContactListView
│   └── ContactDetailView
└── SearchView (cross-module FTS5 search)
```

All modules share one `ZettelDriver` instance via `@EnvironmentObject`.

## Key patterns

- **Schema bootstrap**: each module checks `listTypeSchemas()` before creating tables
- **Shared driver**: injected via SwiftUI environment
- **Cross-module search**: FTS5 search spans all zettel types
- **Tab navigation**: each module is a tab; search is a shared tab
