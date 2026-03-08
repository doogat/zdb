use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response, Extension};
use std::path::Path;

/// Load existing token or generate and persist a new one.
pub fn load_or_create_token(token_file: &Path) -> std::io::Result<String> {
    if let Ok(token) = std::fs::read_to_string(token_file) {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }

    let token = uuid::Uuid::new_v4().to_string();
    if let Some(parent) = token_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(token_file, &token)?;

    // chmod 0600 on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(token_file, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(token)
}

/// Axum middleware: reject requests without valid Bearer token.
pub async fn require_auth(
    Extension(auth): Extension<AuthToken>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let provided = auth_header.strip_prefix("Bearer ").unwrap_or("");

    if provided != auth.0 {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

/// Extension type to carry the expected token in request state.
#[derive(Clone)]
pub struct AuthToken(pub String);
