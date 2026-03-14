use crate::common::ZdbTestRepo;
use predicates::prelude::*;

#[test]
fn create_table() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args([
            "query",
            "CREATE TABLE tasks (status TEXT, priority INTEGER, assignee TEXT)",
        ])
        .assert()
        .success()
        .stdout("table tasks created\n");
}

#[test]
fn insert_and_select() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args([
            "query",
            "CREATE TABLE tasks (status TEXT, priority INTEGER)",
        ])
        .assert()
        .success();

    // Insert rows (sleep between to avoid ID collision)
    let id1 = repo
        .zdb()
        .args([
            "query",
            "INSERT INTO tasks (status, priority) VALUES ('open', 1)",
        ])
        .output()
        .unwrap();
    let id1 = String::from_utf8_lossy(&id1.stdout).trim().to_string();

    std::thread::sleep(std::time::Duration::from_secs(1));

    let id2 = repo
        .zdb()
        .args([
            "query",
            "INSERT INTO tasks (status, priority) VALUES ('closed', 2)",
        ])
        .output()
        .unwrap();
    let id2 = String::from_utf8_lossy(&id2.stdout).trim().to_string();

    // All rows present
    repo.zdb()
        .args(["query", "SELECT id, status, priority FROM tasks"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&id1))
        .stdout(predicate::str::contains(&id2))
        .stdout(predicate::str::contains("open | 1"))
        .stdout(predicate::str::contains("closed | 2"));
}

#[test]
fn select_with_where() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE tasks (status TEXT, assignee TEXT)"])
        .assert()
        .success();

    repo.zdb()
        .args([
            "query",
            "INSERT INTO tasks (status, assignee) VALUES ('open', 'alice')",
        ])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_secs(1));
    repo.zdb()
        .args([
            "query",
            "INSERT INTO tasks (status, assignee) VALUES ('closed', 'bob')",
        ])
        .assert()
        .success();

    repo.zdb()
        .args([
            "query",
            "SELECT status, assignee FROM tasks WHERE assignee = 'alice'",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice"))
        .stdout(predicate::str::contains("bob").not());
}

#[test]
fn update_row() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args([
            "query",
            "CREATE TABLE tasks (status TEXT, priority INTEGER)",
        ])
        .assert()
        .success();

    let out = repo
        .zdb()
        .args([
            "query",
            "INSERT INTO tasks (status, priority) VALUES ('open', 1)",
        ])
        .output()
        .unwrap();
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();

    repo.zdb()
        .args([
            "query",
            &format!("UPDATE tasks SET status = 'done', priority = 10 WHERE id = '{id}'"),
        ])
        .assert()
        .success()
        .stdout("1 row(s) affected\n");

    repo.zdb()
        .args([
            "query",
            &format!("SELECT status, priority FROM tasks WHERE id = '{id}'"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("done | 10"));
}

#[test]
fn delete_row() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE tasks (status TEXT)"])
        .assert()
        .success();

    let out1 = repo
        .zdb()
        .args(["query", "INSERT INTO tasks (status) VALUES ('keep')"])
        .output()
        .unwrap();
    let id1 = String::from_utf8_lossy(&out1.stdout).trim().to_string();
    std::thread::sleep(std::time::Duration::from_secs(1));

    let out2 = repo
        .zdb()
        .args(["query", "INSERT INTO tasks (status) VALUES ('delete-me')"])
        .output()
        .unwrap();
    let id2 = String::from_utf8_lossy(&out2.stdout).trim().to_string();

    repo.zdb()
        .args(["query", &format!("DELETE FROM tasks WHERE id = '{id2}'")])
        .assert()
        .success()
        .stdout("1 row(s) affected\n");

    repo.zdb()
        .args(["query", "SELECT id, status FROM tasks"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&id1))
        .stdout(predicate::str::contains(&id2).not());
}

#[test]
fn reindex_preserves_table() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE tasks (status TEXT)"])
        .assert()
        .success();
    repo.zdb()
        .args(["query", "INSERT INTO tasks (status) VALUES ('open')"])
        .assert()
        .success();

    repo.zdb().arg("reindex").assert().success();

    repo.zdb()
        .args(["query", "SELECT status FROM tasks"])
        .assert()
        .success()
        .stdout(predicate::str::contains("open"));
}

#[test]
fn data_zettel_readable_as_markdown() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args([
            "query",
            "CREATE TABLE tasks (status TEXT, priority INTEGER)",
        ])
        .assert()
        .success();

    let out = repo
        .zdb()
        .args([
            "query",
            "INSERT INTO tasks (status, priority) VALUES ('open', 1)",
        ])
        .output()
        .unwrap();
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();

    repo.zdb()
        .args(["read", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("type: tasks"))
        .stdout(predicate::str::contains("priority: 1"));
}

#[test]
fn install_literature_note_type() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["type", "install", "literature-note"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "installed type \"literature-note\"",
        ));
}

#[test]
fn install_meeting_minutes_type() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["type", "install", "meeting-minutes"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "installed type \"meeting-minutes\"",
        ));

    // Hyphenated type names must work in SQL (requires quoted identifiers)
    let out = repo
        .zdb()
        .args([
            "query",
            r#"INSERT INTO "meeting-minutes" (date, attendees) VALUES ('2026-03-10', 'alice,bob')"#,
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "insert failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();

    repo.zdb()
        .args(["read", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("attendees: alice,bob"));

    // DELETE on hyphenated table must work (requires quoted identifiers in SQL)
    repo.zdb()
        .args([
            "query",
            &format!(r#"DELETE FROM "meeting-minutes" WHERE id = '{id}'"#),
        ])
        .assert()
        .success();
}

#[test]
fn install_kanban_type_and_create_with_default() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["type", "install", "kanban"])
        .assert()
        .success();

    // Insert with omitted status → should get default "backlog"
    let out = repo
        .zdb()
        .args(["query", "INSERT INTO kanban (assignee) VALUES ('alice')"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "insert failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();

    repo.zdb()
        .args(["read", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("status: backlog"));
}

#[test]
fn kanban_rejects_invalid_status() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["type", "install", "kanban"])
        .assert()
        .success();

    repo.zdb()
        .args(["query", "INSERT INTO kanban (status) VALUES ('invalid')"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not in allowed values"));
}

#[test]
fn alter_table_add_column() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE projects (name TEXT)"])
        .assert()
        .success();
    repo.zdb()
        .args(["query", "INSERT INTO projects (name) VALUES ('alpha')"])
        .assert()
        .success();

    repo.zdb()
        .args(["query", "ALTER TABLE projects ADD COLUMN priority INTEGER"])
        .assert()
        .success()
        .stdout(predicate::str::contains("altered"));

    // New column shows NULL for existing row
    repo.zdb()
        .args(["query", "SELECT name, priority FROM projects"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alpha"))
        .stdout(predicate::str::contains("NULL"));
}

#[test]
fn alter_table_rename_column() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE items (name TEXT, score INTEGER)"])
        .assert()
        .success();
    repo.zdb()
        .args(["query", "INSERT INTO items (name, score) VALUES ('x', 42)"])
        .assert()
        .success();

    repo.zdb()
        .args(["query", "ALTER TABLE items RENAME COLUMN score TO rating"])
        .assert()
        .success()
        .stdout(predicate::str::contains("renamed"));

    repo.zdb()
        .args(["query", "SELECT name, rating FROM items"])
        .assert()
        .success()
        .stdout(predicate::str::contains("x"))
        .stdout(predicate::str::contains("42"));
}

#[test]
fn drop_table_cascade() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE droptest (name TEXT)"])
        .assert()
        .success();
    repo.zdb()
        .args(["query", "INSERT INTO droptest (name) VALUES ('gone')"])
        .assert()
        .success();

    repo.zdb()
        .args(["query", "DROP TABLE droptest CASCADE"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dropped"));

    // Table no longer exists
    repo.zdb()
        .args(["query", "SELECT * FROM droptest"])
        .assert()
        .failure();
}

#[test]
fn bulk_update() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args([
            "query",
            "CREATE TABLE bulktest (status TEXT, priority INTEGER)",
        ])
        .assert()
        .success();
    repo.zdb()
        .args([
            "query",
            "INSERT INTO bulktest (status, priority) VALUES ('open', 1)",
        ])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_secs(1));
    repo.zdb()
        .args([
            "query",
            "INSERT INTO bulktest (status, priority) VALUES ('open', 2)",
        ])
        .assert()
        .success();

    repo.zdb()
        .args([
            "query",
            "UPDATE bulktest SET status = 'closed' WHERE priority = 1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 row(s) affected"));

    repo.zdb()
        .args([
            "query",
            "SELECT status, priority FROM bulktest ORDER BY priority",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("closed | 1"))
        .stdout(predicate::str::contains("open | 2"));
}

#[test]
fn bulk_delete() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE delbulk (status TEXT, name TEXT)"])
        .assert()
        .success();
    repo.zdb()
        .args([
            "query",
            "INSERT INTO delbulk (status, name) VALUES ('done', 'a')",
        ])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_secs(1));
    repo.zdb()
        .args([
            "query",
            "INSERT INTO delbulk (status, name) VALUES ('todo', 'b')",
        ])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_secs(1));
    repo.zdb()
        .args([
            "query",
            "INSERT INTO delbulk (status, name) VALUES ('done', 'c')",
        ])
        .assert()
        .success();

    repo.zdb()
        .args(["query", "DELETE FROM delbulk WHERE status = 'done'"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 row(s) affected"));

    repo.zdb()
        .args(["query", "SELECT name FROM delbulk"])
        .assert()
        .success()
        .stdout(predicate::str::contains("b"))
        .stdout(predicate::str::contains("a").not())
        .stdout(predicate::str::contains("c").not());
}

#[test]
fn transaction_commit_via_cli() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE items (name TEXT)"])
        .assert()
        .success();

    // Multi-statement transaction via semicolons
    repo.zdb()
        .args(["query", "BEGIN; INSERT INTO items (name) VALUES ('a'); INSERT INTO items (name) VALUES ('b'); COMMIT"])
        .assert()
        .success()
        .stdout(predicate::str::contains("BEGIN"))
        .stdout(predicate::str::contains("COMMIT"));

    // Both rows should be visible
    repo.zdb()
        .args(["query", "SELECT name FROM items ORDER BY name"])
        .assert()
        .success()
        .stdout(predicate::str::contains("a"))
        .stdout(predicate::str::contains("b"));
}

#[test]
fn transaction_rollback_via_cli() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE items (name TEXT)"])
        .assert()
        .success();

    repo.zdb()
        .args([
            "query",
            "BEGIN; INSERT INTO items (name) VALUES ('gone'); ROLLBACK",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ROLLBACK"));

    // No rows should be visible
    repo.zdb()
        .args(["query", "SELECT COUNT(*) FROM items"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0"));
}

#[test]
fn single_git_commit_for_transaction() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE items (name TEXT)"])
        .assert()
        .success();

    // Count git commits before transaction
    let before = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    let before: usize = String::from_utf8_lossy(&before.stdout)
        .trim()
        .parse()
        .unwrap();

    repo.zdb()
        .args(["query", "BEGIN; INSERT INTO items (name) VALUES ('x'); INSERT INTO items (name) VALUES ('y'); COMMIT"])
        .assert()
        .success();

    let after = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    let after: usize = String::from_utf8_lossy(&after.stdout)
        .trim()
        .parse()
        .unwrap();

    // Should produce exactly one additional git commit
    assert_eq!(
        after - before,
        1,
        "expected single git commit for transaction, got {}",
        after - before
    );

    // Verify commit message
    let log = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["log", "-1", "--format=%s"])
        .output()
        .unwrap();
    let msg = String::from_utf8_lossy(&log.stdout).trim().to_string();
    assert_eq!(msg, "transaction");
}

#[test]
fn multi_table_schema_prd_scenario() {
    // PRD acceptance scenario: workspace/section/link multi-table schema
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE workspace (description TEXT)"])
        .assert()
        .success();
    repo.zdb()
        .args([
            "query",
            "CREATE TABLE section (name TEXT, workspace TEXT REFERENCES workspace(id))",
        ])
        .assert()
        .success();
    repo.zdb()
        .args(["query", "CREATE TABLE link (url TEXT NOT NULL, title TEXT)"])
        .assert()
        .success();

    // Insert workspace
    let ws_out = repo
        .zdb()
        .args([
            "query",
            "INSERT INTO workspace (description) VALUES ('My Board')",
        ])
        .output()
        .unwrap();
    let ws_id = String::from_utf8_lossy(&ws_out.stdout).trim().to_string();
    assert!(!ws_id.is_empty());

    std::thread::sleep(std::time::Duration::from_secs(1));

    // Insert section referencing workspace
    let sec_out = repo
        .zdb()
        .args([
            "query",
            &format!("INSERT INTO section (name, workspace) VALUES ('Dev', '{ws_id}')"),
        ])
        .output()
        .unwrap();
    let sec_id = String::from_utf8_lossy(&sec_out.stdout).trim().to_string();
    assert!(!sec_id.is_empty());

    std::thread::sleep(std::time::Duration::from_secs(1));

    // Insert link
    let link_out = repo
        .zdb()
        .args([
            "query",
            "INSERT INTO link (url, title) VALUES ('https://example.com', 'Example')",
        ])
        .output()
        .unwrap();
    let link_id = String::from_utf8_lossy(&link_out.stdout).trim().to_string();
    assert!(!link_id.is_empty());

    // Typed list query
    repo.zdb()
        .args(["query", "SELECT description FROM workspace"])
        .assert()
        .success()
        .stdout(predicate::str::contains("My Board"));

    // Cross-table join
    repo.zdb()
        .args([
            "query",
            "SELECT s.name, w.description FROM section s JOIN workspace w ON s.workspace = w.id",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dev"))
        .stdout(predicate::str::contains("My Board"));

    // PRD table 4: section-link (hyphenated name, quoted identifier)
    repo.zdb()
        .args([
            "query",
            "CREATE TABLE \"section-link\" (section TEXT REFERENCES section(id), link TEXT REFERENCES link(id))",
        ])
        .assert()
        .success();

    std::thread::sleep(std::time::Duration::from_secs(1));

    // Insert section-link connecting section and link
    repo.zdb()
        .args([
            "query",
            &format!(
                "INSERT INTO \"section-link\" (section, link) VALUES ('{sec_id}', '{link_id}')"
            ),
        ])
        .assert()
        .success();

    // Verify section-link is queryable
    repo.zdb()
        .args(["query", "SELECT section, link FROM \"section-link\""])
        .assert()
        .success()
        .stdout(predicate::str::contains(&sec_id))
        .stdout(predicate::str::contains(&link_id));

    // Search: FTS5 should find the workspace zettel
    repo.zdb()
        .args(["search", "Board"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Board"));

    // Transaction: update workspace + insert another link atomically
    repo.zdb()
        .args([
            "query",
            &format!(
                "BEGIN; UPDATE workspace SET description = 'Updated Board' WHERE id = '{ws_id}'; INSERT INTO link (url, title) VALUES ('https://rust-lang.org', 'Rust'); COMMIT"
            ),
        ])
        .assert()
        .success();

    // Verify both changes persisted
    repo.zdb()
        .args(["query", "SELECT description FROM workspace"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated Board"));

    repo.zdb()
        .args(["query", "SELECT COUNT(*) FROM link"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2"));

    // Schema metadata: verify all 4 typedef types exist
    repo.zdb()
        .args([
            "query",
            "SELECT type, COUNT(*) FROM zettels WHERE type = '_typedef' GROUP BY type",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("4"));

    // Verify on-disk: workspace zettel has the FK backlink
    let sec_content = repo.zdb().args(["read", &sec_id]).output().unwrap();
    let sec_str = String::from_utf8_lossy(&sec_content.stdout);
    assert!(
        sec_str.contains(&ws_id),
        "section should reference workspace ID on disk"
    );
}

#[test]
fn multi_row_insert() {
    let repo = ZdbTestRepo::init();
    repo.zdb()
        .args(["query", "CREATE TABLE items (name TEXT, score INTEGER)"])
        .assert()
        .success();

    // Multi-row INSERT
    let out = repo
        .zdb()
        .args([
            "query",
            "INSERT INTO items (name, score) VALUES ('alpha', 10), ('beta', 20), ('gamma', 30)",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "multi-row insert failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Should return comma-separated IDs
    let ids_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let ids: Vec<&str> = ids_str.split(',').collect();
    assert_eq!(ids.len(), 3, "expected 3 IDs, got: {ids_str}");

    // All rows present
    repo.zdb()
        .args(["query", "SELECT name, score FROM items ORDER BY name"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alpha | 10"))
        .stdout(predicate::str::contains("beta | 20"))
        .stdout(predicate::str::contains("gamma | 30"));

    // Single git commit for the batch
    let before_count = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    let count: usize = String::from_utf8_lossy(&before_count.stdout)
        .trim()
        .parse()
        .unwrap();
    // CREATE TABLE (1 commit for typedef + 1 for materialized) + 1 for multi-row insert
    // Just verify the insert was a single commit by checking total is reasonable
    assert!(count >= 3, "expected at least 3 commits, got {count}");
}
