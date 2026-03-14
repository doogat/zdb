use crate::common::{ServerGuard, ZdbTestRepo};
use std::sync::Arc;
use tokio_postgres::SimpleQueryMessage;

/// Concurrent reads succeed while a write is in flight.
#[test]
fn concurrent_reads_during_write() {
    let repo = ZdbTestRepo::init();

    // Create a zettel so reads have something to query
    repo.zdb()
        .args(["create", "--title", "Seed", "--body", "seed body"])
        .assert()
        .success();

    let server = Arc::new(ServerGuard::start(&repo));

    // Fire concurrent reads + one write simultaneously
    let mut handles: Vec<std::thread::JoinHandle<()>> = Vec::new();

    // 6 concurrent read queries (GraphQL + REST)
    for i in 0..3 {
        let srv = Arc::clone(&server);
        handles.push(std::thread::spawn(move || {
            let result = srv.graphql(r#"{ zettels { id title } }"#);
            assert!(
                result.get("errors").is_none(),
                "concurrent graphql read {i} failed: {result}"
            );
            let zettels = result.pointer("/data/zettels").unwrap();
            assert!(zettels.is_array(), "expected array, got {zettels}");
        }));
    }
    for i in 0..3 {
        let srv = Arc::clone(&server);
        handles.push(std::thread::spawn(move || {
            let resp = srv.rest_get("/zettels");
            assert!(
                resp.status().is_success(),
                "concurrent rest read {i} failed: {}",
                resp.status()
            );
        }));
    }

    // 1 write (mutation)
    let srv = Arc::clone(&server);
    handles.push(std::thread::spawn(move || {
        let result = srv.graphql_with_vars(
            r#"mutation($input: CreateZettelInput!) { createZettel(input: $input) { id } }"#,
            serde_json::json!({ "input": {
                "title": "Written during reads",
                "content": "body",
            } }),
        );
        assert!(
            result.get("errors").is_none(),
            "write during concurrent reads failed: {result}"
        );
    }));

    for h in handles {
        h.join().expect("thread panicked");
    }

    // Verify both seed and new zettel exist
    let result = server.graphql(r#"{ search(query: "Written during reads") { hits { id } } }"#);
    assert!(
        result.get("errors").is_none(),
        "post-write search failed: {result}"
    );
    let hits = result.pointer("/data/search/hits").unwrap();
    assert_eq!(hits.as_array().unwrap().len(), 1);
}

/// pgwire SELECT uses ReadPool (verified by running a query while a mutation is in flight)
#[test]
fn pgwire_select_during_graphql_write() {
    let repo = ZdbTestRepo::init();

    repo.zdb()
        .args(["create", "--title", "PgSeed", "--body", "pg seed"])
        .assert()
        .success();

    let server = Arc::new(ServerGuard::start(&repo));

    // Fire a pgwire SELECT and a GraphQL mutation concurrently
    let srv1 = Arc::clone(&server);
    let pg_handle = std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let (client, connection) = tokio_postgres::Config::new()
                .host("127.0.0.1")
                .port(srv1.pg_port)
                .user("zdb")
                .password(&srv1.token)
                .dbname("zdb")
                .connect(tokio_postgres::NoTls)
                .await
                .expect("pg connect failed");
            tokio::spawn(async move {
                connection.await.ok();
            });

            let messages = client
                .simple_query("SELECT 1 AS n")
                .await
                .expect("pg SELECT failed");
            let row = messages
                .iter()
                .find_map(|m| match m {
                    SimpleQueryMessage::Row(row) => Some(row),
                    _ => None,
                })
                .expect("missing row");
            assert_eq!(row.get(0), Some("1"));
        });
    });

    let srv2 = Arc::clone(&server);
    let write_handle = std::thread::spawn(move || {
        let result = srv2.graphql_with_vars(
            r#"mutation($input: CreateZettelInput!) { createZettel(input: $input) { id } }"#,
            serde_json::json!({ "input": {
                "title": "Written during pg read",
                "content": "body",
            } }),
        );
        assert!(
            result.get("errors").is_none(),
            "write during pg read failed: {result}"
        );
    });

    pg_handle.join().expect("pgwire thread panicked");
    write_handle.join().expect("write thread panicked");
}
