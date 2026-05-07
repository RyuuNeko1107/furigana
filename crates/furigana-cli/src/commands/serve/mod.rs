//! `furigana serve` サブコマンド
//!
//! ローカル HTTP サーバー。default bind は `127.0.0.1:8000`。
//! API は 公開 Furigana API と互換 (drop-in 差し替え可能)。
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
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use clap::Args as ClapArgs;
use furigana::Furigana;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use auth::{build_cors, require_admin_token, require_token};
#[cfg(unix)]
use handlers::do_reload;
use handlers::{admin_reload, furigana_get, furigana_post, healthz};
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

    /// 起動時に GitHub Releases から最新の `ja-furigana-dict` を自動取得してから
    /// listen を開始する。trip 失敗時は warn を出して既存辞書で起動を続行
    /// (network なし環境でも壊れない)。`config.toml [auto_update].pin` で版を固定可。
    #[arg(long)]
    pub auto_pull: bool,
}

pub fn run(args: Args, paths: &Paths, cfg: &Config) -> Result<()> {
    // ─── 起動時 auto-pull (admin token 不要、公開 GitHub Release から取得) ────
    if args.auto_pull {
        let pin = if cfg.auto_update.pin.is_empty() {
            None
        } else {
            Some(cfg.auto_update.pin.as_str())
        };
        tracing::info!(
            "--auto-pull: {} を取得します",
            pin.unwrap_or("最新 release")
        );
        if let Err(e) = super::dict_pull::run(paths, pin) {
            tracing::warn!("起動時 dict pull に失敗 ({e})。既存辞書で起動を続行します。");
        }
    }

    let furigana_inner = super::build_furigana(paths)?;
    // server は最初のリクエストレイテンシを下げるため、Lindera analyzer を eager init。
    // build_furigana 自体は lazy なので listen 前にここで明示的に init して
    // 起動失敗を listen 前に検知できるようにもする。
    furigana_inner
        .preload()
        .map_err(|e| anyhow!("形態素解析器の初期化に失敗: {e}"))?;
    let furigana: Arc<Furigana> = Arc::new(furigana_inner);

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

        // 定期 auto-update: 公開 GitHub Releases を polling して新 tag があれば自動 reload
        if cfg.auto_update.enabled {
            spawn_auto_update(state.clone(), cfg.auto_update.clone());
        } else {
            tracing::info!(
                "auto_update: 無効 ([auto_update].enabled = true で起動時に polling task を生やす)"
            );
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

/// `[auto_update]` の polling task を spawn する。
///
/// 動作:
/// 1. `interval` を待つ (起動直後の polling は省略、急いで叩かない設計)
/// 2. GitHub API で `ja-furigana-dict` の最新 tag を取得
///    (`pin` が空なら latest、空でなければそれを期待値として比較)
/// 3. 反映済 tag (memory 上に保持) と違えば `dict_pull` + 再 build + state swap
/// 4. ループ
///
/// admin_tokens 設定不要 (内部呼び出しで HTTP 経由しない)。失敗時は warn を出し
/// 既存辞書で稼働を継続。
fn spawn_auto_update(state: AppState, cfg: crate::config::AutoUpdateConfig) {
    let interval = parse_duration(&cfg.interval).unwrap_or_else(|| {
        tracing::warn!(
            "auto_update.interval='{}' を解釈できませんでした (例: 1h / 30m / 1d)。 24h にフォールバック。",
            cfg.interval
        );
        std::time::Duration::from_secs(24 * 3600)
    });
    tracing::info!(
        "auto_update: 有効 (interval={:?}, pin={})",
        interval,
        if cfg.pin.is_empty() {
            "(latest)"
        } else {
            &cfg.pin
        }
    );

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        // 初回 tick は即時消費して、次の interval 経過後に最初の polling
        ticker.tick().await;

        // memory 上に「最後に反映した tag」を保持。process 再起動で初期化される
        // (pin 指定時はそのまま、未指定時は latest を 1 回引いてから比較開始)
        let mut last_applied: Option<String> = if cfg.pin.is_empty() {
            None
        } else {
            Some(cfg.pin.clone())
        };

        loop {
            ticker.tick().await;

            let target_tag = if cfg.pin.is_empty() {
                match crate::commands::dict_pull::resolve_latest_tag_async().await {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!("auto_update: latest tag 取得失敗: {e}");
                        continue;
                    }
                }
            } else {
                cfg.pin.clone()
            };

            if last_applied.as_deref() == Some(target_tag.as_str()) {
                tracing::debug!("auto_update: 既に {} 反映済、skip", target_tag);
                continue;
            }

            tracing::info!("auto_update: {} を取得 + reload します", target_tag);

            let paths = state.paths.clone();
            let target_owned = target_tag.clone();
            let result = tokio::task::spawn_blocking(move || {
                crate::commands::dict_pull::run(&paths, Some(&target_owned))
            })
            .await;

            match result {
                Ok(Ok(())) => {
                    // pull 成功 → state を swap
                    match handlers::do_reload(&state).await {
                        Ok(size) => {
                            tracing::info!(
                                "auto_update: {} 反映完了 (dict_size={})",
                                target_tag,
                                size
                            );
                            last_applied = Some(target_tag);
                        }
                        Err(e) => {
                            tracing::warn!("auto_update: reload 失敗: {e}");
                        }
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("auto_update: dict pull 失敗: {e}");
                }
                Err(e) => {
                    tracing::warn!("auto_update: spawn_blocking join error: {e}");
                }
            }
        }
    });
}

/// シンプルな期間 parser: `"30m" / "1h" / "6h" / "1d" / "3600"` 等を受ける
fn parse_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num_part, mult) = if let Some(n) = s.strip_suffix('h') {
        (n, 3600)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60)
    } else if let Some(n) = s.strip_suffix('d') {
        (n, 86400)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1)
    } else {
        (s, 1) // 純粋な数字なら秒
    };
    num_part
        .trim()
        .parse::<u64>()
        .ok()
        .map(|n| std::time::Duration::from_secs(n * mult))
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
