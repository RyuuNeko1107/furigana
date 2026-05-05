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
