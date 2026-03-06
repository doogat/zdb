use async_graphql::ServerError;
use zdb_core::error::ZettelError;

/// Convert ZettelError to ServerError for use in dynamic schema resolvers.
pub fn to_server_error(e: ZettelError) -> ServerError {
    let (code, msg) = match &e {
        ZettelError::NotFound(m) => ("NOT_FOUND", m.as_str()),
        ZettelError::Validation(m) => ("VALIDATION_ERROR", m.as_str()),
        ZettelError::InvalidPath(m) => ("INVALID_PATH", m.as_str()),
        ZettelError::SqlEngine(m) => ("SQL_ERROR", m.as_str()),
        _ => ("INTERNAL_ERROR", ""),
    };
    let message = if msg.is_empty() {
        e.to_string()
    } else {
        msg.to_string()
    };
    let mut err = ServerError::new(message, None);
    err.extensions = Some({
        let mut map = async_graphql::ErrorExtensionValues::default();
        map.set("code", code);
        map
    });
    err
}
