use crate::state::AppState;
use axum::{
    Json,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const COOKIE_NAME: &str = "codeprism_token";

#[derive(Deserialize)]
pub struct LoginRequest {
    pub token: String,
}

#[derive(Serialize)]
pub struct AuthStatus {
    pub authentication_required: bool,
    pub authenticated: bool,
}

pub fn api_token_from_env() -> anyhow::Result<Option<Arc<str>>> {
    let Ok(raw_token) = std::env::var("CODEPRISM_API_TOKEN") else {
        return Ok(None);
    };
    let token = raw_token.trim();
    if token.is_empty() {
        return Ok(None);
    }
    if token.len() < 32
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        anyhow::bail!(
            "CODEPRISM_API_TOKEN must contain at least 32 ASCII letters, digits, '-' or '_'"
        );
    }
    Ok(Some(Arc::from(token)))
}

pub fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}

fn token_matches(actual: &str, expected: &str) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    actual
        .bytes()
        .zip(expected.bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn request_token(request: &Request) -> Option<&str> {
    if let Some(value) = request.headers().get(header::AUTHORIZATION)
        && let Ok(value) = value.to_str()
        && let Some(token) = value.strip_prefix("Bearer ")
    {
        return Some(token);
    }
    if let Some(value) = request.headers().get("x-codeprism-token")
        && let Ok(value) = value.to_str()
    {
        return Some(value);
    }
    request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == COOKIE_NAME).then_some(value)
            })
        })
}

fn is_authenticated(request: &Request, expected: &str) -> bool {
    request_token(request).is_some_and(|actual| token_matches(actual, expected))
}

pub async fn require_api_token(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if !path.starts_with("/api/")
        || matches!(path, "/api/v1/auth/status" | "/api/v1/auth/login")
        || state
            .api_token
            .as_deref()
            .is_none_or(|token| is_authenticated(&request, token))
    {
        return next.run(request).await;
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({"error": "Authentication required"})),
    )
        .into_response()
}

pub async fn auth_status(State(state): State<AppState>, request: Request) -> Json<AuthStatus> {
    let token = state.api_token.as_deref();
    Json(AuthStatus {
        authentication_required: token.is_some(),
        authenticated: token.is_none_or(|token| is_authenticated(&request, token)),
    })
}

pub async fn login(State(state): State<AppState>, Json(request): Json<LoginRequest>) -> Response {
    let Some(expected) = state.api_token.as_deref() else {
        return StatusCode::NO_CONTENT.into_response();
    };
    if !token_matches(&request.token, expected) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid token"})),
        )
            .into_response();
    }

    let cookie =
        format!("{COOKIE_NAME}={expected}; Path=/; HttpOnly; SameSite=Strict; Max-Age=28800");
    (StatusCode::NO_CONTENT, [(header::SET_COOKIE, cookie)]).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_hosts_and_tokens() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("localhost"));
        assert!(!is_loopback_host("0.0.0.0"));
        assert!(token_matches("a", "a"));
        assert!(!token_matches("a", "b"));
        assert!(!token_matches("short", "longer"));
    }

    #[test]
    fn accepts_bearer_and_same_site_cookie_tokens() {
        let bearer = Request::builder()
            .header(header::AUTHORIZATION, "Bearer test-token")
            .body(axum::body::Body::empty())
            .unwrap();
        let cookie = Request::builder()
            .header(header::COOKIE, "theme=dark; codeprism_token=test-token")
            .body(axum::body::Body::empty())
            .unwrap();

        assert!(is_authenticated(&bearer, "test-token"));
        assert!(is_authenticated(&cookie, "test-token"));
    }
}
