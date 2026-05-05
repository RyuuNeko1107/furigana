//! `furigana serve` サブコマンド
//!
//! ローカル HTTP サーバー。default bind は `127.0.0.1:8000`。
//! API は ryuuneko.com の公開 Furigana API と互換 (drop-in 差し替え可能)。
//!
//! ## 構成
//! - [`types`]    : リクエスト / レスポンス型 + AppState
//! - [`handlers`] : `/furigana` `/healthz` のハンドラ + 変換ロジック
//! - [`auth`]     : 認証ミドルウェア + CORS
//!
//! ## エンドポイント
//! - `GET  /furigana?text=...&mode=tts|hiragana|ruby|kanji`
//! - `POST /furigana`  body: `{ "text": "...", "mode": "...", ... }`
//! - `GET  /healthz`   ヘルスチェック
//!
//! ## 認証
//! `X-API-Key: <token>` (公開 API 互換、優先) または
//! `Authorization: Bearer <token>` (fallback)。
//! `config.toml` の `[auth].tokens` または起動時 `--token`
//! (env `FURIGANA_TOKEN`) が空でない場合のみ必須化。
//! `/healthz` は常に認証不要。

mod auth;
mod handlers;
mod types;

use crate::config::Config;
use crate::paths::Paths;
use anyhow::{anyhow, Result};
use axum::{middleware, routing::{get, post}, Router};
use clap::Args as ClapArgs;
use furigana::Furigana;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use auth::{build_cors, require_admin_token, require_token};
use handlers::{admin_reload, furigana_get, furigana_post, healthz};
#[cfg(unix)]
use handlers::do_reload;
use types::AppState;

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

pub fn run(args: Args, paths: &Paths, cfg: &Config) -> Result<()> {
    let furigana: Arc<Furigana> = Arc::new(super::build_furigana(paths)?);

    let bind = args.bind.unwrap_or_else(|| cfg.server.bind.clone());
    let mut tokens = cfg.auth.tokens.clone();
    if let Some(t) = args.token {
        tokens.push(t);
    }
    let auth_enabled = !tokens.is_empty();
    let admin_tokens = cfg.auth.admin_tokens.clone();
    let admin_enabled = !admin_tokens.is_empty();

    let state = AppState {
        furigana: Arc::new(RwLock::new(furigana)),
        tokens: Arc::new(tokens),
        admin_tokens: Arc::new(admin_tokens),
        paths: Arc::new(paths.clone()),
    };

    let cors = build_cors(cfg);

    let furigana_routes = Router::new()
        .route("/furigana", get(furigana_get).post(furigana_post))
        .layer(middleware::from_fn_with_state(state.clone(), require_token));

    let admin_routes = Router::new()
        .route("/admin/reload", post(admin_reload))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_admin_token,
        ));

    let app = Router::new()
        .merge(furigana_routes)
        .merge(admin_routes)
        .route("/healthz", get(healthz))
        .layer(cors)
        .with_state(state.clone());

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
            tracing::info!("認証 (/furigana): 有効 (X-API-Key または Bearer)");
        } else {
            tracing::info!("認証 (/furigana): 無効 (ローカル想定)");
        }
        if admin_enabled {
            tracing::info!("admin (/admin/reload): 有効 (admin_tokens で認証)");
        } else {
            tracing::info!("admin (/admin/reload): 無効 ([auth].admin_tokens を設定すると有効化)");
        }

        // Unix: SIGHUP で reload (本番運用で `kill -HUP <pid>` で辞書再読込できるように)
        #[cfg(unix)]
        spawn_sighup_reload(state.clone());

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| anyhow!("server error: {e}"))?;

        anyhow::Ok(())
    })?;

    tracing::info!("シャットダウン完了");
    Ok(())
}

/// SIGHUP を listen して `do_reload` を呼ぶバックグラウンドタスクを起動。
/// 本番運用 (systemd 等) で `systemctl reload furigana` 相当の挙動。
/// Windows ではビルドから除外される。
#[cfg(unix)]
fn spawn_sighup_reload(state: AppState) {
    use tokio::signal::unix::{signal, SignalKind};
    tokio::spawn(async move {
        let mut sighup = match signal(SignalKind::hangup()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("SIGHUP handler 初期化失敗: {e}");
                return;
            }
        };
        while sighup.recv().await.is_some() {
            tracing::info!("SIGHUP を受信、辞書を reload します");
            match do_reload(&state).await {
                Ok(size) => tracing::info!("reload 成功 (dict_size={size})"),
                Err(e) => tracing::error!("reload 失敗: {e}"),
            }
        }
    });
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
