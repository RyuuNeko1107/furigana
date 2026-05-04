//! `furigana serve` サブコマンド
//!
//! ローカル HTTP サーバーを起動。default bind は `127.0.0.1:8000`。
//!
//! ## エンドポイント
//! - `GET  /furigana?text=...&format=ruby|hiragana` — クエリ経由で 1 件変換
//! - `POST /furigana`                                — JSON body `{ "text": "...", "format": "..." }`
//! - `GET  /healthz`                                 — `"ok"` テキスト
//!
//! ## 認証
//! `config.toml` の `[auth].tokens` または `--token` で渡された値が
//! 1 つ以上設定されていれば、`/furigana` に `Authorization: Bearer <token>`
//! が必須。空なら認証無し (デフォルト)。
//!
//! ## CORS
//! `config.server.cors_origins` が空なら全 origin 許可 (localhost 用途)。
//! 値ありなら厳密なオリジンチェック。

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
use clap::Args as ClapArgs;
use furigana::Furigana;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// bind address (config.toml `[server].bind` を上書き)
    #[arg(long)]
    pub bind: Option<String>,

    /// 認証トークン (env `FURIGANA_TOKEN`)。
    /// 指定すると `/furigana` で Bearer 必須化。
    #[arg(long, env = "FURIGANA_TOKEN")]
    pub token: Option<String>,
}

#[derive(Clone)]
struct AppState {
    furigana: Arc<Furigana>,
    tokens: Arc<Vec<String>>,
}

#[derive(Deserialize)]
struct FuriganaQuery {
    text: String,
    #[serde(default = "default_format")]
    format: String,
}

#[derive(Deserialize)]
struct FuriganaBody {
    text: String,
    #[serde(default = "default_format")]
    format: String,
}

#[derive(Serialize)]
struct FuriganaResponse {
    text: String,
    reading: String,
    format: String,
}

fn default_format() -> String {
    "ruby".to_string()
}

async fn healthz(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "dict_size": state.furigana.dict_size(),
    }))
}

async fn furigana_get(
    State(state): State<AppState>,
    Query(q): Query<FuriganaQuery>,
) -> Result<Json<FuriganaResponse>, (StatusCode, String)> {
    process(&state.furigana, &q.text, &q.format)
}

async fn furigana_post(
    State(state): State<AppState>,
    Json(body): Json<FuriganaBody>,
) -> Result<Json<FuriganaResponse>, (StatusCode, String)> {
    process(&state.furigana, &body.text, &body.format)
}

fn process(
    f: &Furigana,
    text: &str,
    format: &str,
) -> Result<Json<FuriganaResponse>, (StatusCode, String)> {
    let reading = match format {
        "ruby" => f.to_ruby(text),
        "hiragana" | "hira" => f.to_hiragana(text),
        other => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("unknown format '{other}' (use: ruby | hiragana)"),
            ));
        }
    };
    Ok(Json(FuriganaResponse {
        text: text.to_string(),
        reading,
        format: format.to_string(),
    }))
}

async fn require_bearer(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if state.tokens.is_empty() {
        return Ok(next.run(req).await);
    }
    let auth = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());
    let presented = match auth {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => return Err(StatusCode::UNAUTHORIZED),
    };
    if state.tokens.iter().any(|t| t == presented) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

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

/// 実行
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_bearer,
        ));

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
            tracing::info!("Bearer 認証: 有効");
        } else {
            tracing::info!("Bearer 認証: 無効 (ローカル想定)");
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
