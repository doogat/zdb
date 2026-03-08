use crate::common::ZdbTestRepo;
use predicates::prelude::*;

#[test]
fn delete_reports_broken_backlinks() {
    let repo = ZdbTestRepo::init();

    // Create target zettel A
    let a_out = repo
        .zdb()
        .args(["create", "--title", "Target"])
        .output()
        .unwrap();
    let a_id = String::from_utf8_lossy(&a_out.stdout).trim().to_string();
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Create zettel B that links to A
    repo.zdb()
        .args([
            "create",
            "--title",
            "Linker",
            "--body",
            &format!("See [[{a_id}|Target]]."),
        ])
        .assert()
        .success();

    // Reindex so wikilinks are in _zdb_links
    repo.zdb().arg("reindex").assert().success();

    // Delete A — should warn about B's broken backlink
    repo.zdb()
        .args(["delete", &a_id])
        .assert()
        .success()
        .stderr(predicate::str::contains("broken backlinks"));
}

#[test]
fn status_reports_broken_backlinks_after_delete() {
    let repo = ZdbTestRepo::init();

    // Create target zettel A
    let a_out = repo
        .zdb()
        .args(["create", "--title", "Target"])
        .output()
        .unwrap();
    let a_id = String::from_utf8_lossy(&a_out.stdout).trim().to_string();
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Create zettel B that links to A
    repo.zdb()
        .args([
            "create",
            "--title",
            "Linker",
            "--body",
            &format!("See [[{a_id}]]."),
        ])
        .assert()
        .success();

    // Reindex so wikilinks are in _zdb_links
    repo.zdb().arg("reindex").assert().success();

    // Delete A
    repo.zdb().args(["delete", &a_id]).assert().success();

    // Status should report broken backlinks
    repo.zdb()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("broken backlinks"));
}

#[test]
fn delete_no_backlinks_no_warning() {
    let repo = ZdbTestRepo::init();

    let out = repo
        .zdb()
        .args(["create", "--title", "Lonely"])
        .output()
        .unwrap();
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();

    repo.zdb()
        .args(["delete", &id])
        .assert()
        .success()
        .stderr(predicate::str::contains("broken backlinks").not());
}
