//! `furigana serve` サブコマンド
//!
//! ローカル HTTP サーバー。default bind は `127.0.0.1:8000`。
//! API は ryuuneko.com の公開 Furigana API と互換 (drop-in 差し替え可能)。
//!
//! ## エンドポイント
//! - `GET  /furigana?text=...&mode=tts|hiragana|ruby|kanji`
//! - `POST /furigana`  body: `{ "text": "...", "mode": "...", ... }`
//! - `GET  /healthz`   ヘルスチェック
//!
//! ## パラメータ (公開 API 互換)
//! | name | default | 用途 |
//! |---|---|---|
//! | `text` | (必須) | 変換対象 (最大 10,000 文字) |
//! | `text_b64` | - | text の Base64 URL-safe 版 |
//! | `mode` | `tts` | `tts` / `hiragana` / `ruby` / `kanji` |
//! | `short_pause` | `" "` | TTS: `、` 後挿入 |
//! | `long_pause` | `"   "` | TTS: `。!?` 後挿入 |
//! | `keep_period` | `true` | TTS: `。` を残す |
//! | `segmented` | `false` | `segments` 配列を返す |
//! | `max_segment_len` | `60` | segment の最大文字数 |
//! | `debug` | `false` | `timings_ms` を返す |
//!
//! ## 認証
//! `X-API-Key: <token>` (公開 API 互換) または `Authorization: Bearer <token>`。
//! `config.toml` の `[auth].tokens` または起動時 `--token` (env `FURIGANA_TOKEN`)
//! が空でない場合のみ必須化。`/healthz` は常に認証不要。

use crate::config::Config;
use crate::paths::Paths;
use anyhow::{anyhow, Result};
use axum::{
    extract::{Query, Request, State},
    http::{HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::get,
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use clap::Args as ClapArgs;
use furigana::{Furigana, TtsOptions};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

/// 1 リクエストあたりの最大入力文字数 (公開 API 仕様)
const MAX_TEXT_LEN: usize = 10_000;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// bind address (`config.toml [server].bind` を上書き)
    #[arg(long)]
    pub bind: Option<String>,

    /// 認証トークン (env `FURIGANA_TOKEN`)。
    /// 指定すると `/furigana` で `X-API-Key` または Bearer 必須化。
    #[arg(long, env = "FURIGANA_TOKEN")]
    pub token: Option<String>,
}

#[derive(Clone)]
struct AppState {
    furigana: Arc<Furigana>,
    tokens: Arc<Vec<String>>,
}

// ─── リクエスト型 ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct FuriganaParams {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    text_b64: Option<String>,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_short_pause")]
    short_pause: String,
    #[serde(default = "default_long_pause")]
    long_pause: String,
    #[serde(default = "default_true")]
    keep_period: bool,
    #[serde(default)]
    segmented: bool,
    #[serde(default = "default_max_seg")]
    max_segment_len: usize,
    #[serde(default)]
    debug: bool,
}

fn default_mode() -> String {
    "tts".to_string()
}
fn default_short_pause() -> String {
    " ".to_string()
}
fn default_long_pause() -> String {
    "   ".to_string()
}
fn default_true() -> bool {
    true
}
fn default_max_seg() -> usize {
    60
}

// ─── レスポンス型 ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct FuriganaResponse {
    result: String,
    mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    segments: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timings_ms: Option<Value>,
}

type ApiError = (StatusCode, Json<Value>);

fn error(code: StatusCode, msg: impl Into<String>) -> ApiError {
    (code, Json(json!({ "error": msg.into() })))
}

// ─── ハンドラ ────────────────────────────────────────────────────────────────

async fn healthz(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "dict_size": state.furigana.dict_size(),
    }))
}

async fn furigana_get(
    State(state): State<AppState>,
    Query(params): Query<FuriganaParams>,
) -> Result<Json<FuriganaResponse>, ApiError> {
    process_furigana(&state.furigana, params)
}

async fn furigana_post(
    State(state): State<AppState>,
    Json(params): Json<FuriganaParams>,
) -> Result<Json<FuriganaResponse>, ApiError> {
    process_furigana(&state.furigana, params)
}

fn process_furigana(
    f: &Furigana,
    params: FuriganaParams,
) -> Result<Json<FuriganaResponse>, ApiError> {
    // ─── text 解決 ────────────────────────────────────────────────────────
    let text = if let Some(b64) = params.text_b64.as_ref() {
        let decoded = URL_SAFE_NO_PAD
            .decode(b64.trim_end_matches('='))
            .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid base64 in text_b64"))?;
        String::from_utf8(decoded).map_err(|_| {
            error(
                StatusCode::BAD_REQUEST,
                "text_b64 decoded bytes are not valid UTF-8",
            )
        })?
    } else if let Some(t) = params.text.as_ref() {
        t.clone()
    } else {
        return Err(error(StatusCode::BAD_REQUEST, "no text provided"));
    };

    if text.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "no text provided"));
    }
    let nchars = text.chars().count();
    if nchars > MAX_TEXT_LEN {
        return Err(error(
            StatusCode::BAD_REQUEST,
            format!("text too long: {nchars} chars (max {MAX_TEXT_LEN})"),
        ));
    }

    // ─── mode 正規化 ──────────────────────────────────────────────────────
    let mode = match params.mode.as_str() {
        "tts" | "hiragana" | "ruby" | "kanji" => params.mode.clone(),
        _ => default_mode(),
    };

    // ─── 変換 ─────────────────────────────────────────────────────────────
    let t_start = Instant::now();
    let tokens_start = Instant::now();
    let tokens = f.tokenize(&text);
    let t_tokenize_ms = tokens_start.elapsed().as_secs_f64() * 1000.0;

    let convert_start = Instant::now();
    let result = match mode.as_str() {
        "kanji" => text.clone(),
        "ruby" => furigana::tokens_to_ruby(&tokens),
        "hiragana" => furigana::tokens_to_hiragana(&tokens),
        _ => {
            // tts (default)
            let opts = TtsOptions {
                short_pause: params.short_pause.clone(),
                long_pause: params.long_pause.clone(),
                keep_period: params.keep_period,
            };
            let hira = furigana::tokens_to_hiragana(&tokens);
            furigana::tts::normalize_for_tts(&hira, &opts)
        }
    };
    let t_convert_ms = convert_start.elapsed().as_secs_f64() * 1000.0;
    let t_total_ms = t_start.elapsed().as_secs_f64() * 1000.0;

    // ─── segments / timings ──────────────────────────────────────────────
    let segments = if params.segmented && (mode == "tts" || mode == "hiragana") {
        Some(furigana::tts::segment_for_tts(
            &result,
            params.max_segment_len,
        ))
    } else {
        None
    };

    let timings_ms = if params.debug {
        Some(json!({
            "total": round1(t_total_ms),
            "tokenize": round1(t_tokenize_ms),
            "convert": round1(t_convert_ms),
        }))
    } else {
        None
    };

    Ok(Json(FuriganaResponse {
        result,
        mode,
        segments,
        timings_ms,
    }))
}

fn round1(ms: f64) -> f64 {
    (ms * 10.0).round() / 10.0
}

// ─── 認証ミドルウェア ────────────────────────────────────────────────────────

async fn require_token(
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

// ─── CORS ────────────────────────────────────────────────────────────────────

fn build_cors(cfg: &Config) -> CorsLayer {
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

// ─── 起動 ────────────────────────────────────────────────────────────────────

pub fn run(args: Args, paths: &Paths, cfg: &Config) -> Result<()> {
    let furigana = Arc::new(super::build_furigana(paths)?);

    let bind = args.bind.unwrap_or_else(|| cfg.server.bind.clone());
    let mut tokens = cfg.auth.tokens.clone();
    if let Some(t) = args.token {
        tokens.push(t);
    }
    let auth_enabled = !tokens.is_empty();

    let state = AppState {
        furigana,
        tokens: Arc::new(tokens),
    };

    let cors = build_cors(cfg);

    let furigana_routes = Router::new()
        .route("/furigana", get(furigana_get).post(furigana_post))
        .layer(middleware::from_fn_with_state(state.clone(), require_token));

    let app = Router::new()
        .merge(furigana_routes)
        .route("/healthz", get(healthz))
        .layer(cors)
        .with_state(state);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async move {
        let listener = TcpListener::bind(&bind)
            .await
            .map_err(|e| anyhow!("bind {bind} に失敗: {e}"))?;
        let actual = listener
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| bind.clone());

        tracing::info!("furigana serving on http://{actual}");
        if auth_enabled {
            tracing::info!("認証: 有効 (X-API-Key または Bearer)");
        } else {
            tracing::info!("認証: 無効 (ローカル想定)");
        }

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| anyhow!("server error: {e}"))?;

        anyhow::Ok(())
    })?;

    tracing::info!("シャットダウン完了");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => tracing::info!("SIGINT を受信、シャットダウンします"),
            _ = sigterm.recv() => tracing::info!("SIGTERM を受信、シャットダウンします"),
        }
    }
    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
        tracing::info!("Ctrl+C を受信、シャットダウンします");
    }
}
