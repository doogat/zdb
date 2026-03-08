use std::sync::Arc;
use crate::common::{ServerGuard, ZdbTestRepo};

#[test]
fn compact_mutation_returns_result() {
    let repo = ZdbTestRepo::init();
    let server = ServerGuard::start(&repo);

    let result = server.graphql(
        r#"mutation { compact { filesRemoved crdtDocsCompacted gcSuccess } }"#,
    );
    assert!(result.get("errors").is_none(), "compact failed: {result}");
    let compact = &result["data"]["compact"];
    assert!(compact["filesRemoved"].is_i64());
    assert!(compact["crdtDocsCompacted"].is_i64());
    assert!(compact["gcSuccess"].is_boolean());
}

#[test]
fn compact_force_mutation() {
    let repo = ZdbTestRepo::init();
    let server = ServerGuard::start(&repo);

    let result = server.graphql(
        r#"mutation { compact(force: true) { filesRemoved crdtDocsCompacted gcSuccess } }"#,
    );
    assert!(result.get("errors").is_none(), "compact(force: true) failed: {result}");
}

#[test]
fn sync_mutation_no_remote_returns_error() {
    let repo = ZdbTestRepo::init();
    let server = ServerGuard::start(&repo);

    // No remote configured — should return an error, not panic
    let result = server.graphql(
        r#"mutation { sync { direction commitsTransferred conflictsResolved resurrected } }"#,
    );
    assert!(result.get("errors").is_some(), "sync without remote should error: {result}");
}

#[test]
fn sync_mutation_with_remote() {
    use tempfile::TempDir;

    // Set up a bare remote
    let remote_dir = TempDir::new().unwrap();
    let status = std::process::Command::new("git")
        .args(["init", "--bare"])
        .arg(remote_dir.path())
        .status()
        .expect("failed to spawn git init");
    assert!(status.success(), "git init --bare failed");

    let repo = ZdbTestRepo::init();

    // Add remote + register node
    let status = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["remote", "add", "origin"])
        .arg(remote_dir.path())
        .status()
        .expect("failed to spawn git remote add");
    assert!(status.success(), "git remote add failed");
    repo.zdb()
        .args(["register-node", "TestNode"])
        .assert()
        .success();

    // Push initial state
    let status = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["push", "-u", "origin", "master"])
        .status()
        .expect("failed to spawn git push");
    assert!(status.success(), "git push failed");

    let server = ServerGuard::start(&repo);

    let result = server.graphql(
        r#"mutation { sync { direction commitsTransferred conflictsResolved resurrected } }"#,
    );
    assert!(result.get("errors").is_none(), "sync with remote failed: {result}");
    let sync = &result["data"]["sync"];
    assert!(sync["direction"].is_string());
    assert!(sync["commitsTransferred"].is_i64());
    assert!(sync["conflictsResolved"].is_i64());
    assert!(sync["resurrected"].is_i64());
}

#[test]
fn sync_during_writes_serialized_through_actor() {
    use tempfile::TempDir;

    // Set up a bare remote
    let remote_dir = TempDir::new().unwrap();
    let status = std::process::Command::new("git")
        .args(["init", "--bare"])
        .arg(remote_dir.path())
        .status()
        .expect("failed to spawn git init");
    assert!(status.success(), "git init --bare failed");

    let repo = ZdbTestRepo::init();

    // Add remote + register node
    let status = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["remote", "add", "origin"])
        .arg(remote_dir.path())
        .status()
        .expect("failed to spawn git remote add");
    assert!(status.success(), "git remote add failed");
    repo.zdb()
        .args(["register-node", "TestNode"])
        .assert()
        .success();

    let status = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["push", "-u", "origin", "master"])
        .status()
        .expect("failed to spawn git push");
    assert!(status.success(), "git push failed");

    let server = Arc::new(ServerGuard::start(&repo));

    // Spawn concurrent writers + sync
    let mut handles = Vec::new();

    for i in 0..5 {
        let srv = Arc::clone(&server);
        handles.push(std::thread::spawn(move || {
            let result = srv.graphql_with_vars(
                r#"mutation($input: CreateZettelInput!) { createZettel(input: $input) { id } }"#,
                serde_json::json!({ "input": {
                    "title": format!("Concurrent Write {i}"),
                    "content": format!("body {i}"),
                } }),
            );
            assert!(
                result.get("errors").is_none(),
                "concurrent write {i} failed: {result}"
            );
        }));
    }

    // Sync concurrently with writes
    let srv = Arc::clone(&server);
    handles.push(std::thread::spawn(move || {
        let result = srv.graphql(
            r#"mutation { sync { direction commitsTransferred conflictsResolved resurrected } }"#,
        );
        assert!(
            result.get("errors").is_none(),
            "concurrent sync failed: {result}"
        );
    }));

    // All must complete without panic or error
    for h in handles {
        h.join().expect("thread panicked during concurrent mutations");
    }
}
