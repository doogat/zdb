# Android Host-Shell Example

A Jetpack Compose host-shell app demonstrating two mini-app modules (Bookmarks and Contacts) sharing one embedded ZettelDB core.

## Prerequisites

1. Build the Android AAR:
   ```bash
   cd ../../
   dev/bin/build-android
   ```

2. Generate Kotlin bindings:
   ```bash
   cargo run -p zdb-core --bin uniffi-bindgen -- generate \
     --library target/debug/libzdb_core.dylib \
     --language kotlin --out-dir examples/android-host-shell/core-zdb/src/main/java
   ```

3. Open in Android Studio and sync Gradle.

## Architecture

```
HostShellApp (Application)
├── ZettelDriver (one instance, app-scoped)
├── :feature-bookmarks
│   ├── BookmarksModule.bootstrap()
│   ├── BookmarkListScreen
│   └── BookmarkDetailScreen
├── :feature-contacts
│   ├── ContactsModule.bootstrap()
│   ├── ContactListScreen
│   └── ContactDetailScreen
└── :core-zdb
    └── ZDBModule interface + shared ZettelDriver access
```

## Module structure

```
android-host-shell/
├── app/                      Main app module
├── core-zdb/                 Shared ZettelDriver wrapper + module interface
├── feature-bookmarks/        Bookmarks mini-app
├── feature-contacts/         Contacts mini-app
├── build.gradle.kts          Root build file
└── settings.gradle.kts       Module declarations
```

## Key patterns

- **Schema bootstrap**: each module checks `listTypeSchemas()` before creating tables
- **Shared driver**: provided by `ZDBApplication`, accessed via `(application as ZDBApplication).driver`
- **Bottom navigation**: each module is a destination
- **Cross-module search**: FTS5 search spans all zettel types
