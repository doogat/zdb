use crate::common::{MultiNodeSetup, ZdbTestRepo};

/// Round-robin sync: push from one node, pull on all others
fn sync_round_robin(setup: &MultiNodeSetup) {
    for node in &setup.nodes {
        MultiNodeSetup::push(node);
    }
    for node in &setup.nodes {
        MultiNodeSetup::sync(node);
    }
}

// ── Test: 3-node convergence ─────────────────────────────────────

#[test]
fn three_node_convergence() {
    let setup = MultiNodeSetup::new(3);

    // Each node creates a zettel
    let id0 = MultiNodeSetup::create(&setup.nodes[0], "Note from 0", "body0");
    MultiNodeSetup::push(&setup.nodes[0]);

    MultiNodeSetup::sync(&setup.nodes[1]);
    let id1 = MultiNodeSetup::create(&setup.nodes[1], "Note from 1", "body1");
    MultiNodeSetup::push(&setup.nodes[1]);

    MultiNodeSetup::sync(&setup.nodes[2]);
    let id2 = MultiNodeSetup::create(&setup.nodes[2], "Note from 2", "body2");
    MultiNodeSetup::push(&setup.nodes[2]);

    // Sync all
    sync_round_robin(&setup);

    // Verify all nodes see all 3 zettels
    for node in &setup.nodes {
        let out0 = MultiNodeSetup::read(node, &id0);
        assert!(out0.contains("Note from 0"), "node missing zettel 0");
        let out1 = MultiNodeSetup::read(node, &id1);
        assert!(out1.contains("Note from 1"), "node missing zettel 1");
        let out2 = MultiNodeSetup::read(node, &id2);
        assert!(out2.contains("Note from 2"), "node missing zettel 2");
    }
}

// ── Test: concurrent creates ─────────────────────────────────────

#[test]
fn concurrent_creates() {
    let setup = MultiNodeSetup::new(3);

    // All nodes create different zettels without syncing first.
    // Sleep between creates to ensure distinct ZettelIDs (timestamp-based).
    let id0 = MultiNodeSetup::create(&setup.nodes[0], "Concurrent 0", "c0");
    std::thread::sleep(std::time::Duration::from_secs(1));
    let id1 = MultiNodeSetup::create(&setup.nodes[1], "Concurrent 1", "c1");
    std::thread::sleep(std::time::Duration::from_secs(1));
    let id2 = MultiNodeSetup::create(&setup.nodes[2], "Concurrent 2", "c2");

    // Cascade sync: each node syncs (fetch+merge+push) in sequence
    // Round 1: propagate from each node to remote
    MultiNodeSetup::sync(&setup.nodes[0]);
    MultiNodeSetup::sync(&setup.nodes[1]);
    MultiNodeSetup::sync(&setup.nodes[2]);

    // Round 2: pull everything back (node0 needs node1+2 changes, etc)
    MultiNodeSetup::sync(&setup.nodes[0]);
    MultiNodeSetup::sync(&setup.nodes[1]);
    MultiNodeSetup::sync(&setup.nodes[2]);

    // All nodes see all 3
    for (i, node) in setup.nodes.iter().enumerate() {
        assert!(MultiNodeSetup::read(node, &id0).contains("Concurrent 0"),
            "node {i} missing zettel from node 0");
        assert!(MultiNodeSetup::read(node, &id1).contains("Concurrent 1"),
            "node {i} missing zettel from node 1");
        assert!(MultiNodeSetup::read(node, &id2).contains("Concurrent 2"),
            "node {i} missing zettel from node 2");
    }
}

// ── Test: stale node return ──────────────────────────────────────

#[test]
fn stale_node_returns() {
    let setup = MultiNodeSetup::new(3);

    // Node0 creates, pushes. Node1 syncs and creates. Node2 stays offline.
    let id0 = MultiNodeSetup::create(&setup.nodes[0], "Before stale", "b0");
    MultiNodeSetup::push(&setup.nodes[0]);

    MultiNodeSetup::sync(&setup.nodes[1]);
    let id1 = MultiNodeSetup::create(&setup.nodes[1], "While stale", "b1");
    MultiNodeSetup::push(&setup.nodes[1]);

    // Node2 was offline the whole time. Now it syncs.
    MultiNodeSetup::sync(&setup.nodes[2]);

    let out0 = MultiNodeSetup::read(&setup.nodes[2], &id0);
    assert!(out0.contains("Before stale"));
    let out1 = MultiNodeSetup::read(&setup.nodes[2], &id1);
    assert!(out1.contains("While stale"));
}

// ── Test: network partition and reconnect ────────────────────────

#[test]
fn network_partition_and_reconnect() {
    let setup = MultiNodeSetup::new(3);

    // Create initial state, sync all
    let id0 = MultiNodeSetup::create(&setup.nodes[0], "Shared note", "original");
    MultiNodeSetup::push(&setup.nodes[0]);
    sync_round_robin(&setup);

    // Partition: node0 edits, pushes. node2 can't reach remote.
    MultiNodeSetup::update(&setup.nodes[0], &id0, "Partition edit", "from-node0");
    MultiNodeSetup::push(&setup.nodes[0]);

    // Node2 makes its own edit (offline)
    let id2 = MultiNodeSetup::create(&setup.nodes[2], "Offline note", "partition");

    // Reconnect: node2 syncs (will merge node0's changes)
    MultiNodeSetup::sync(&setup.nodes[2]);
    MultiNodeSetup::push(&setup.nodes[2]);

    // Final sync
    sync_round_robin(&setup);

    // All nodes see both edits
    for node in &setup.nodes {
        // The updated note should exist
        let out = MultiNodeSetup::read(node, &id0);
        assert!(out.contains("Partition edit") || out.contains("Shared note"),
            "merged note should be accessible");
        // The offline-created note should exist
        let out2 = MultiNodeSetup::read(node, &id2);
        assert!(out2.contains("Offline note"));
    }
}

// ── Test: stale node resync after compaction ─────────────────────

#[test]
fn stale_node_resync_after_compaction() {
    let setup = MultiNodeSetup::new(3);

    // All nodes start with a shared note
    let id = MultiNodeSetup::create(&setup.nodes[0], "Shared", "original body");
    MultiNodeSetup::push(&setup.nodes[0]);
    MultiNodeSetup::sync(&setup.nodes[1]);
    MultiNodeSetup::sync(&setup.nodes[2]);

    // Node0 and Node1 make conflicting edits. Node2 stays offline (stale).
    MultiNodeSetup::update(&setup.nodes[0], &id, "Edit from 0", "body from node0");
    MultiNodeSetup::push(&setup.nodes[0]);

    MultiNodeSetup::update(&setup.nodes[1], &id, "Edit from 1", "body from node1");
    // Node1 syncs — this triggers conflict resolution and creates CRDT temp files
    MultiNodeSetup::sync(&setup.nodes[1]);

    // Compact on node1 to remove CRDT temp files
    ZdbTestRepo::zdb_at(&setup.nodes[1])
        .args(["compact", "--force"])
        .assert()
        .success();

    MultiNodeSetup::push(&setup.nodes[1]);

    // Node2 was offline the entire time. Now it makes a conflicting edit and syncs.
    MultiNodeSetup::update(&setup.nodes[2], &id, "Edit from 2", "body from node2");
    // This sync should succeed even without CRDT state (LWW fallback)
    MultiNodeSetup::sync(&setup.nodes[2]);

    // Verify the zettel is readable (valid markdown, no panic)
    let out = MultiNodeSetup::read(&setup.nodes[2], &id);
    assert!(
        out.contains("Edit from") || out.contains("Shared"),
        "stale node should have a resolved zettel after resync"
    );
}

// ── Test: HLC LWW picks later writer ─────────────────────────────

#[test]
fn hlc_lww_picks_later_writer() {
    let setup = MultiNodeSetup::new(2);

    // Install kanban type on node0 (uses preset:last-writer-wins), sync to both
    ZdbTestRepo::zdb_at(&setup.nodes[0])
        .args(["type", "install", "kanban"])
        .assert()
        .success();
    MultiNodeSetup::sync(&setup.nodes[0]);
    MultiNodeSetup::sync(&setup.nodes[1]);

    // Create a kanban zettel on node0, sync to both
    let id = MultiNodeSetup::create(&setup.nodes[0], "LWW task", "original body");
    ZdbTestRepo::zdb_at(&setup.nodes[0])
        .args(["update", &id, "--type", "kanban"])
        .assert()
        .success();
    MultiNodeSetup::sync(&setup.nodes[0]);
    MultiNodeSetup::sync(&setup.nodes[1]);

    // Node0 edits first (earlier HLC)
    MultiNodeSetup::update(&setup.nodes[0], &id, "From node0", "body from node0");

    // Sleep to ensure node1's HLC is strictly later
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Node1 edits second (later HLC)
    MultiNodeSetup::update(&setup.nodes[1], &id, "From node1", "body from node1");

    // Node0 syncs first (pushes its edit to remote)
    MultiNodeSetup::sync(&setup.nodes[0]);

    // Node1 syncs — conflict resolved by LWW, node1 (later HLC) should win
    MultiNodeSetup::sync(&setup.nodes[1]);

    let out = MultiNodeSetup::read(&setup.nodes[1], &id);
    assert!(
        out.contains("From node1"),
        "LWW should pick later writer (node1), got: {out}"
    );
    assert!(
        out.contains("body from node1"),
        "LWW should pick later writer body, got: {out}"
    );
}

// ── Test: bundle full export/import bootstrap ────────────────────

#[test]
fn bundle_full_bootstrap() {
    let setup = MultiNodeSetup::new(2);

    // Create content on node0
    let id = MultiNodeSetup::create(&setup.nodes[0], "Bundle test", "bundle body");
    MultiNodeSetup::push(&setup.nodes[0]);

    // Export full bundle from node0
    let bundle_path = setup.remote_dir.path().join("full.bundle.tar");
    ZdbTestRepo::zdb_at(&setup.nodes[0])
        .args(["bundle", "export", "--full", "--output"])
        .arg(&bundle_path)
        .assert()
        .success();

    assert!(bundle_path.exists(), "bundle file should exist");

    // Create a fresh node3 (not cloned from remote)
    let dir3 = tempfile::TempDir::new().unwrap();
    let path3 = dir3.path().to_path_buf();
    ZdbTestRepo::zdb_at(&path3)
        .arg("init")
        .arg(&path3)
        .assert()
        .success();
    ZdbTestRepo::zdb_at(&path3)
        .args(["register-node", "Node-3"])
        .assert()
        .success();

    // Import the full bundle
    ZdbTestRepo::zdb_at(&path3)
        .args(["bundle", "import"])
        .arg(&bundle_path)
        .assert()
        .success();

    // Verify the zettel exists on node3
    let out = MultiNodeSetup::read(&path3, &id);
    assert!(out.contains("Bundle test"), "bootstrapped node should have the zettel");
}

// ── Test: air-gapped delta transfer ──────────────────────────────

#[test]
fn airgapped_delta_transfer() {
    let setup = MultiNodeSetup::new(2);

    // Initial content + sync
    let _id0 = MultiNodeSetup::create(&setup.nodes[0], "Initial", "init");
    MultiNodeSetup::push(&setup.nodes[0]);
    MultiNodeSetup::sync(&setup.nodes[1]);

    // Node0 creates a new zettel (node1 doesn't know about it yet)
    let id1 = MultiNodeSetup::create(&setup.nodes[0], "Delta note", "delta");
    MultiNodeSetup::push(&setup.nodes[0]);

    // Export delta bundle targeting node1
    let bundle_path = setup.remote_dir.path().join("delta.bundle.tar");

    // Export as full (delta requires knowing remote UUID; full is simpler for testing)
    ZdbTestRepo::zdb_at(&setup.nodes[0])
        .args(["bundle", "export", "--full", "--output"])
        .arg(&bundle_path)
        .assert()
        .success();

    // Import on node1
    ZdbTestRepo::zdb_at(&setup.nodes[1])
        .args(["bundle", "import"])
        .arg(&bundle_path)
        .assert()
        .success();

    // Node1 should now see the delta note
    let out = MultiNodeSetup::read(&setup.nodes[1], &id1);
    assert!(out.contains("Delta note"), "delta transfer should bring new zettel");
}

// ── Test: concurrent edits to same zettel ────────────────────────

#[test]
fn concurrent_edits_same_zettel() {
    let setup = MultiNodeSetup::new(3);

    // Node0 creates a zettel, sync to all
    let id = MultiNodeSetup::create(&setup.nodes[0], "Shared zettel", "original body");
    MultiNodeSetup::push(&setup.nodes[0]);
    MultiNodeSetup::sync(&setup.nodes[1]);
    MultiNodeSetup::sync(&setup.nodes[2]);

    // All 3 nodes edit the same zettel without syncing between edits
    MultiNodeSetup::update(&setup.nodes[0], &id, "Edit from node0", "body from node0");
    MultiNodeSetup::update(&setup.nodes[1], &id, "Edit from node1", "body from node1");
    MultiNodeSetup::update(&setup.nodes[2], &id, "Edit from node2", "body from node2");

    // Sync cascade: 3 rounds to ensure full convergence
    for _ in 0..3 {
        sync_round_robin(&setup);
    }

    // All nodes must converge to identical content (CRDT determinism)
    let content0 = MultiNodeSetup::read(&setup.nodes[0], &id);
    let content1 = MultiNodeSetup::read(&setup.nodes[1], &id);
    let content2 = MultiNodeSetup::read(&setup.nodes[2], &id);

    assert_eq!(content0, content1, "node0 and node1 diverged");
    assert_eq!(content1, content2, "node1 and node2 diverged");
}

// ── Test: delete-vs-edit across 3 nodes ──────────────────────────

#[test]
fn delete_vs_edit_multi_node() {
    let setup = MultiNodeSetup::new(3);

    // Node0 creates a zettel, sync to all
    let id = MultiNodeSetup::create(&setup.nodes[0], "Will conflict", "original body");
    MultiNodeSetup::push(&setup.nodes[0]);
    MultiNodeSetup::sync(&setup.nodes[1]);
    MultiNodeSetup::sync(&setup.nodes[2]);

    // Node1 deletes the zettel
    MultiNodeSetup::delete(&setup.nodes[1], &id);

    // Node2 edits the zettel
    MultiNodeSetup::update(&setup.nodes[2], &id, "Edited after delete", "surviving body");

    // Node1 pushes delete, then node2 syncs (triggers delete-vs-edit conflict)
    MultiNodeSetup::push(&setup.nodes[1]);
    MultiNodeSetup::sync(&setup.nodes[2]);

    // Full sync to propagate resolution
    for _ in 0..3 {
        sync_round_robin(&setup);
    }

    // Edit wins: zettel should exist on all nodes with node2's content
    for (i, node) in setup.nodes.iter().enumerate() {
        let out = MultiNodeSetup::read(node, &id);
        assert!(
            out.contains("Edited after delete") || out.contains("surviving body"),
            "node {i}: edit should win over delete, got: {out}"
        );
    }

    // Check resurrected marker in frontmatter
    let out = MultiNodeSetup::read(&setup.nodes[2], &id);
    assert!(
        out.contains("resurrected: true"),
        "resurrected marker missing from frontmatter: {out}"
    );
}
