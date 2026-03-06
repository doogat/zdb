use crate::common::{read_next, ServerGuard, ZdbTestRepo};
use tungstenite::http::Request;
use tungstenite::connect;

#[test]
fn subscribe_mutate_receive() {
    let repo = ZdbTestRepo::init();
    let server = ServerGuard::start(&repo);

    let mut ws = server.ws_subscribe("subscription { zettelChanged { action zettelId zettel { id title } } }");

    // Mutate: create a zettel
    let result = server.graphql_with_vars(
        r#"mutation($input: CreateZettelInput!) { createZettel(input: $input) { id title } }"#,
        serde_json::json!({ "input": { "title": "Sub Test", "content": "body" } }),
    );
    assert!(result.get("errors").is_none(), "create failed: {result}");
    let created_id = result["data"]["createZettel"]["id"].as_str().unwrap();

    // Read the subscription event
    let event = read_next(&mut ws);
    let data = &event["payload"]["data"]["zettelChanged"];
    assert_eq!(data["action"], "created");
    assert_eq!(data["zettelId"], created_id);
    assert_eq!(data["zettel"]["title"], "Sub Test");
}

#[test]
fn subscribe_delete_receive() {
    let repo = ZdbTestRepo::init();
    let server = ServerGuard::start(&repo);

    // Create first
    let result = server.graphql_with_vars(
        r#"mutation($input: CreateZettelInput!) { createZettel(input: $input) { id } }"#,
        serde_json::json!({ "input": { "title": "To Delete" } }),
    );
    let id = result["data"]["createZettel"]["id"].as_str().unwrap().to_string();

    // Subscribe to deletions
    let mut ws = server.ws_subscribe("subscription { zettelDeleted }");

    // Delete
    let result = server.graphql(&format!(r#"mutation {{ deleteZettel(id: "{id}") }}"#));
    assert!(result.get("errors").is_none(), "delete failed: {result}");

    // Read event
    let event = read_next(&mut ws);
    let deleted_id = &event["payload"]["data"]["zettelDeleted"];
    assert_eq!(deleted_id.as_str().unwrap(), id);
}

#[test]
fn ws_auth_missing_returns_401() {
    let repo = ZdbTestRepo::init();
    let server = ServerGuard::start(&repo);

    let request = Request::builder()
        .uri(format!("ws://127.0.0.1:{}/ws", server.port))
        .header("Sec-WebSocket-Protocol", "graphql-transport-ws")
        .header("Host", format!("127.0.0.1:{}", server.port))
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", tungstenite::handshake::client::generate_key())
        .body(())
        .unwrap();

    let result = connect(request);
    // Should fail — server returns 401 before upgrade
    assert!(result.is_err(), "WS without auth should fail");
}

#[test]
fn no_subscriber_mutations_work() {
    let repo = ZdbTestRepo::init();
    let server = ServerGuard::start(&repo);

    // No WS subscribers — just do mutations and verify they succeed
    for i in 0..3 {
        let result = server.graphql_with_vars(
            r#"mutation($input: CreateZettelInput!) { createZettel(input: $input) { id } }"#,
            serde_json::json!({ "input": { "title": format!("NoSub {i}") } }),
        );
        assert!(result.get("errors").is_none(), "create {i} failed: {result}");
    }

    // Verify all created
    let result = server.graphql(r#"{ zettels { id } }"#);
    let list = result["data"]["zettels"].as_array().unwrap();
    assert_eq!(list.len(), 3);
}

#[test]
fn subscribe_type_filter() {
    let repo = ZdbTestRepo::init();
    let server = ServerGuard::start(&repo);

    // Create two types
    for table in ["contact", "bookmark"] {
        let sql = format!("CREATE TABLE {table} (name TEXT NOT NULL)");
        let result = server.graphql_with_vars(
            r#"mutation($sql: String!) { executeSql(sql: $sql) { message } }"#,
            serde_json::json!({ "sql": sql }),
        );
        assert!(result.get("errors").is_none(), "CREATE TABLE {table} failed: {result}");
    }

    // Subscribe to contactChanged only
    let mut ws = server.ws_subscribe("subscription { contactChanged { action zettelId } }");

    // Create a bookmark — should NOT trigger contact subscription
    let result = server.graphql_with_vars(
        r#"mutation($input: CreateZettelInput!) { createZettel(input: $input) { id } }"#,
        serde_json::json!({ "input": { "title": "My Bookmark", "type": "bookmark" } }),
    );
    assert!(result.get("errors").is_none(), "create bookmark failed: {result}");

    // Create a contact — SHOULD trigger
    let result = server.graphql_with_vars(
        r#"mutation($input: CreateZettelInput!) { createZettel(input: $input) { id } }"#,
        serde_json::json!({ "input": { "title": "Alice", "type": "contact" } }),
    );
    assert!(result.get("errors").is_none(), "create contact failed: {result}");
    let contact_id = result["data"]["createZettel"]["id"].as_str().unwrap();

    // The first event we get should be the contact, not the bookmark
    let event = read_next(&mut ws);
    let data = &event["payload"]["data"]["contactChanged"];
    assert_eq!(data["action"], "created");
    assert_eq!(data["zettelId"], contact_id);
}

#[test]
fn subscribe_disconnect_reconnect() {
    let repo = ZdbTestRepo::init();
    let server = ServerGuard::start(&repo);

    // First connection + subscription
    {
        let mut ws = server.ws_subscribe("subscription { zettelChanged { action zettelId } }");
        let result = server.graphql_with_vars(
            r#"mutation($input: CreateZettelInput!) { createZettel(input: $input) { id } }"#,
            serde_json::json!({ "input": { "title": "First" } }),
        );
        assert!(result.get("errors").is_none());
        let event = read_next(&mut ws);
        assert_eq!(event["payload"]["data"]["zettelChanged"]["action"], "created");
        // ws drops here — disconnect
    }

    // Second connection — should work without issues
    let mut ws = server.ws_subscribe("subscription { zettelChanged { action zettelId } }");
    let result = server.graphql_with_vars(
        r#"mutation($input: CreateZettelInput!) { createZettel(input: $input) { id } }"#,
        serde_json::json!({ "input": { "title": "Second" } }),
    );
    assert!(result.get("errors").is_none());
    let event = read_next(&mut ws);
    assert_eq!(event["payload"]["data"]["zettelChanged"]["action"], "created");
}
