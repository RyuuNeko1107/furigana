//! 設定ファイル (TOML) の読み込み
//!
//! 設定ファイルは optional。存在しない場合は default 値を使う。
//! 各フィールドは `serve` サブコマンドから読まれる。

use crate::paths::Paths;
use anyhow::{Context, Result};
use serde::Deserialize;

/// CLI 設定 (config.toml 全体)
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Config {
    /// HTTP サーバー設定
    #[serde(default)]
    pub server: ServerConfig,

    /// bearer 認証設定
    #[serde(default)]
    pub auth: AuthConfig,

    /// 自動辞書更新 (`furigana serve --auto-pull` の挙動も含む)
    #[serde(default)]
    pub auto_update: AutoUpdateConfig,
}

/// `[server]` セクション
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// bind address (default: `127.0.0.1:8000`)
    #[serde(default = "default_bind")]
    pub bind: String,

    /// CORS 許可オリジン (空 = same-origin only)
    #[serde(default)]
    pub cors_origins: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            cors_origins: Vec::new(),
        }
    }
}

fn default_bind() -> String {
    "127.0.0.1:8000".to_string()
}

/// `[auth]` セクション
#[derive(Debug, Default, Clone, Deserialize)]
pub struct AuthConfig {
    /// `/furigana` 認証用 (空 = 認証無効、ローカル想定のデフォルト)
    #[serde(default)]
    pub tokens: Vec<String>,

    /// `/admin/*` (reload 等) 専用トークン (空 = `/admin/*` 無効化)
    #[serde(default)]
    pub admin_tokens: Vec<String>,
}

/// `[auto_update]` セクション
///
/// `furigana serve` 起動中の **定期 polling** で GitHub Releases から
/// 最新 `ja-furigana-dict` を取得 → 自動 reload する仕組み。
/// 公開 API のみで完結するため admin_tokens 設定不要 (内部呼び出し)。
///
/// 起動時 1 回だけ pull したい場合は `--auto-pull` フラグの方を使う。
#[derive(Debug, Clone, Deserialize)]
pub struct AutoUpdateConfig {
    /// 定期 polling を有効化するか (default: false で opt-in)
    #[serde(default)]
    pub enabled: bool,

    /// polling 間隔。`"30m" / "1h" / "6h" / "1d"` 等の表記。
    /// 短すぎると GitHub API rate limit (60 req/h/IP) に当たるので 1h 以上推奨。
    #[serde(default = "default_interval")]
    pub interval: String,

    /// ピン留めする tag (例: `"v0.1.2"`)。空 or 未指定で **最新追従**。
    /// `--auto-pull` 起動時 pull もこのピンを尊重する。
    #[serde(default)]
    pub pin: String,
}

impl Default for AutoUpdateConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval: default_interval(),
            pin: String::new(),
        }
    }
}

fn default_interval() -> String {
    "24h".to_string()
}

impl Config {
    /// 設定ファイルを読み込む (存在しなければ default)
    pub fn load(paths: &Paths) -> Result<Self> {
        if !paths.config_file.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&paths.config_file).with_context(|| {
            format!("設定ファイル読み込み失敗: {}", paths.config_file.display())
        })?;
        let cfg: Self = toml::from_str(&content)
            .with_context(|| format!("設定ファイルパース失敗: {}", paths.config_file.display()))?;
        Ok(cfg)
    }
}
