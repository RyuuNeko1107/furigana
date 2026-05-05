//! 認証ミドルウェア + CORS 設定

use super::types::AppState;
use crate::config::Config;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use tower_http::cors::{Any, CorsLayer};

/// `state.tokens` が空でない時、`/furigana` へのリクエストに認証を要求する
///
/// `X-API-Key` を優先、無ければ `Authorization: Bearer` を読む。
pub(super) async fn require_token(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if state.tokens.is_empty() {
        return Ok(next.run(req).await);
    }
    let presented = extract_token(&req).ok_or(StatusCode::UNAUTHORIZED)?;
    if state.tokens.iter().any(|t| t == &presented) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// `/admin/*` 用の認証ミドルウェア
///
/// `state.admin_tokens` が空なら 503 (admin 機能 disabled)。
/// 空でなければ `X-API-Key` または `Authorization: Bearer` を厳密に照合。
/// 認証は常に必須 (一般 tokens では通らない)。
pub(super) async fn require_admin_token(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if state.admin_tokens.is_empty() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let presented = extract_token(&req).ok_or(StatusCode::UNAUTHORIZED)?;
    if state.admin_tokens.iter().any(|t| t == &presented) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// `X-API-Key` 優先、無ければ `Authorization: Bearer <token>` を読む
fn extract_token(req: &Request) -> Option<String> {
    if let Some(v) = req.headers().get("x-api-key").and_then(|v| v.to_str().ok()) {
        let s = v.trim();
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    let auth = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())?;
    let stripped = auth.strip_prefix("Bearer ")?;
    Some(stripped.to_string())
}

/// `config.server.cors_origins` から `CorsLayer` を組み立てる。空なら Any 許可。
pub(super) fn build_cors(cfg: &Config) -> CorsLayer {
    let methods = [Method::GET, Method::POST, Method::OPTIONS];
    if cfg.server.cors_origins.is_empty() {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(methods)
            .allow_headers(Any)
    } else {
        let origins: Vec<HeaderValue> = cfg
            .server
            .cors_origins
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(methods)
            .allow_headers(Any)
    }
}
