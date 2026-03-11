# Mobile Constraints

This page covers platform-specific constraints for mobile host-shell apps. Read [Building Apps](./building-apps.md#mobile-mini-apps) first for the host-shell architecture.

## Background Execution

### iOS

iOS aggressively limits background execution:

| Mechanism | Duration | Guarantee |
|-----------|----------|-----------|
| Background task (UIApplication) | ~30 seconds | Best-effort, system can end early |
| BGAppRefreshTask | ~30 seconds | System-scheduled, not guaranteed |
| BGProcessingTask | Minutes | Only when plugged in, rare |
| Push notification trigger | ~30 seconds | Requires server-side push |

**Practical impact**: sync (`ZettelDriver` remote fetch/push) should run in the foreground or in a short background task. Do not rely on background execution for regular sync — it may not fire for hours or days.

**What not to attempt in background**:
- Full reindex (can take seconds at scale)
- Large sync operations (network + git merge + index update)
- CRDT conflict resolution (may require multiple git operations)

**What works in background**:
- Reading from SQLite index (instant, read-only)
- Scheduling a BGAppRefreshTask for next foreground sync
- Updating widget timelines from cached index data

### Android

Android background restrictions vary by OS version and manufacturer:

| Mechanism | Behavior |
|-----------|----------|
| WorkManager (periodic) | Minimum 15-minute interval, batched by system |
| WorkManager (one-time) | Runs when constraints met, may be deferred |
| Doze mode | Network and CPU suspended; only high-priority FCM breaks through |
| App Standby Buckets | Limits job frequency based on app usage |
| Manufacturer restrictions | Samsung, Xiaomi, etc. add extra kills |

**Practical impact**: same as iOS — sync in foreground, use WorkManager for opportunistic background sync, do not depend on timely execution.

## Widget Data Freshness

Widgets show data from the SQLite index. They cannot trigger sync or run the full ZettelDB core.

### iOS WidgetKit

- Widgets render from a **timeline** of entries
- The system decides when to refresh (budget-limited)
- Host app calls `WidgetCenter.shared.reloadAllTimelines()` after writes
- Widget reads `index.db` directly (read-only SQLite connection via App Group)
- Expect staleness: seconds (if app just wrote) to hours (if app hasn't run)

```swift
// In host app, after any write operation
WidgetCenter.shared.reloadAllTimelines()
```

```swift
// In widget TimelineProvider
func getTimeline(in context: Context, completion: @escaping (Timeline<Entry>) -> Void) {
    let dbPath = appGroupURL.appendingPathComponent(".zdb/index.db").path
    // Open read-only SQLite connection, query materialized tables
    let entries = queryRecentBookmarks(dbPath: dbPath)
    let timeline = Timeline(entries: entries, policy: .after(Date().addingTimeInterval(3600)))
    completion(timeline)
}
```

### Android AppWidgetProvider

- Widgets update via `onUpdate()` callback or explicit broadcast
- Host app sends `ACTION_APPWIDGET_UPDATE` after writes
- Widget reads `index.db` directly from app-private storage
- Use `RemoteViewsFactory` for collection widgets (ListView, GridView)

```kotlin
// In host app, after any write operation
val appWidgetManager = AppWidgetManager.getInstance(context)
val ids = appWidgetManager.getAppWidgetIds(ComponentName(context, BookmarkWidget::class.java))
appWidgetManager.notifyAppWidgetViewDataChanged(ids, R.id.bookmark_list)
```

## Extension Lifecycle

### iOS Share Extension

Share extensions let users send content to your app from other apps (Safari, Photos, etc.).

- Runs in a **separate process** from the host app
- Has access to App Group storage
- Can create a short-lived `ZettelDriver` to insert one zettel
- Must complete within the system time limit (~seconds)
- Host app reindexes on next launch to pick up the new zettel

```swift
// In ShareViewController
func didSelectPost() {
    let driver = try ZettelDriver(repoPath: appGroupRepoPath)
    try driver.executeSql("""
        INSERT INTO bookmark (title, url) VALUES ('\(title)', '\(url)')
    """)
    // Driver dropped, git commit is atomic
    extensionContext?.completeRequest(returningItems: nil)
}
```

**Warning**: do not hold the `ZettelDriver` open beyond the single operation. Extensions are killed without notice.

### iOS Action Extension

Action extensions (e.g., "Open in ZettelDB") are read-only — they display data but should not write. Open a read-only SQLite connection to the index.

### Android Share Target

```kotlin
// In ShareActivity
override fun onCreate(savedInstanceState: Bundle?) {
    val url = intent.getStringExtra(Intent.EXTRA_TEXT) ?: return finish()
    val driver = ZettelDriver(repoPath = appGroupRepoPath)
    driver.executeSql("INSERT INTO bookmark (title, url) VALUES ('Shared', '$url')")
    finish()
}
```

## Sync Strategy on Mobile

### Recommended approach

1. **Foreground sync**: trigger sync when the app becomes active (`onResume` / `scenePhase == .active`)
2. **User-initiated sync**: pull-to-refresh or explicit sync button
3. **Opportunistic background**: schedule via BGAppRefreshTask (iOS) or WorkManager (Android), but don't depend on it
4. **No always-on sync**: do not run a persistent background service or keep a network connection open

### Sync flow

```
App becomes active
  → check last sync timestamp
  → if stale (>N minutes), trigger sync
  → sync runs: fetch remote → merge → push → reindex
  → update widget timelines
  → save sync timestamp
```

### Conflict handling

Conflicts are resolved automatically by the CRDT resolver — no user intervention needed. On mobile, conflicts are more likely because sync is infrequent. The CRDT model handles this gracefully: all devices converge to the same state regardless of merge order.

## Performance Considerations

### Cold start

ZettelDriver initialization includes:
- Opening the git repo (~1-5ms)
- Opening/creating the SQLite index (~1-5ms)
- Schema bootstrap per module (~10-50ms per CREATE TABLE check)

At 100 zettels, total cold start is under 100ms. At 1K zettels, under 200ms. These numbers are from the FFI performance tests (see [FFI docs](../technical/ffi.md)).

### Index reuse

The SQLite index persists across app launches. A full reindex is only needed when:
- The index file is deleted or corrupted
- The app detects the git HEAD has changed since last index update (new commits from sync)
- The user explicitly requests it

For normal app launches with no sync changes, the existing index is used as-is — no reindex cost.

### Memory pressure

On iOS, respond to `didReceiveMemoryWarning` by dropping any cached data. The `ZettelDriver` itself holds minimal memory (just Mutex-wrapped handles). SQLite's page cache is the main consumer — it releases memory automatically under pressure.

On Android, consider closing the `ZettelDriver` in `onTrimMemory(TRIM_MEMORY_BACKGROUND)` and reopening on next use.
