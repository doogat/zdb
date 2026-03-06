use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use async_graphql::dynamic::Schema;
use async_graphql::http::ALL_WEBSOCKET_PROTOCOLS;
use async_graphql_axum::{GraphQLProtocol, GraphQLWebSocket};
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::Extension;

/// WebSocket upgrade handler for GraphQL subscriptions.
///
/// Auth is handled by the existing `require_auth` middleware on the router.
/// Clients send `Authorization: Bearer <token>` on the HTTP upgrade request.
///
/// Keepalive: async-graphql sends periodic pings per the graphql-ws protocol.
/// `keepalive_timeout` closes the connection if a pong isn't received within 30s.
/// Idle connections survive indefinitely as long as the client responds to pings.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    protocol: GraphQLProtocol,
    Extension(shared_schema): Extension<Arc<ArcSwap<Schema>>>,
) -> impl IntoResponse {
    let schema = (*shared_schema.load_full()).clone();
    ws.protocols(ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |stream| {
            GraphQLWebSocket::new(stream, schema, protocol)
                .keepalive_timeout(Duration::from_secs(30))
                .serve()
        })
}
