//! 認証ミドルウェア + CORS 設定

use super::types::AppState;
use crate::config::Config;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use std::net::SocketAddr;
use subtle::ConstantTimeEq;
use tower_http::cors::{Any, CorsLayer};

/// log 用に token を識別可能だが秘匿性は保つ形 (先頭 4 文字 + length) で短縮
fn token_log_repr(token: &str) -> String {
    let prefix: String = token.chars().take(4).collect();
    format!("{}…(len={})", prefix, token.len())
}

/// 認証失敗時の共通 log + metrics
fn record_auth_failure(state: &AppState, req: &Request, presented: Option<&str>, scope: &str) {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.to_string())
        .unwrap_or_else(|| "?".to_string());
    let path = req.uri().path();
    let token_repr = presented
        .map(token_log_repr)
        .unwrap_or_else(|| "<none>".to_string());
    tracing::warn!(
        peer = %peer,
        path = %path,
        scope = %scope,
        token = %token_repr,
        "auth failure"
    );
    state.metrics.record_auth_failure();
}

/// **timing-safe** な token 比較。
///
/// 単純 `==` で比較すると secret length / 一致 prefix 長 が処理時間差に漏れて
/// 攻撃者が char-by-char で token を推測できる。 `subtle::ConstantTimeEq` で
/// 全 byte を見比べた結果に縮約して時間差を消す。 length 不一致は早期 false。
fn tokens_match(allowed: &[String], presented: &str) -> bool {
    let presented_bytes = presented.as_bytes();
    let mut found = subtle::Choice::from(0u8);
    for t in allowed {
        let t_bytes = t.as_bytes();
        if t_bytes.len() != presented_bytes.len() {
            continue;
        }
        found |= t_bytes.ct_eq(presented_bytes);
    }
    bool::from(found)
}

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
    let Some(presented) = extract_token(&req) else {
        record_auth_failure(&state, &req, None, "user");
        return Err(StatusCode::UNAUTHORIZED);
    };
    if tokens_match(&state.tokens, &presented) {
        Ok(next.run(req).await)
    } else {
        record_auth_failure(&state, &req, Some(&presented), "user");
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
    let Some(presented) = extract_token(&req) else {
        record_auth_failure(&state, &req, None, "admin");
        return Err(StatusCode::UNAUTHORIZED);
    };
    if tokens_match(&state.admin_tokens, &presented) {
        Ok(next.run(req).await)
    } else {
        record_auth_failure(&state, &req, Some(&presented), "admin");
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
