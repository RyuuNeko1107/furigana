//! データディレクトリ・設定パスの解決
//!
//! 優先順位: CLI `--data-dir` / `--config` > 環境変数 > **実行ファイルと同じディレクトリ** > XDG fallback
//!
//! 「`furigana.exe` をダブルクリックで起動 → 同じフォルダに `dict/` 等が展開される」
//! portable な配置を default にしてある。フォルダごとコピーすれば持ち運び可能。
//! `current_exe()` の解決に失敗した場合のみ XDG (`~/.local/share/furigana/` /
//! `%LOCALAPPDATA%\furigana\furigana\`) に fallback する。

use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use std::path::{Path, PathBuf};

/// 解決済みの各種パス
#[derive(Debug, Clone)]
pub struct Paths {
    /// 実行時データのルート (辞書 / rules 等)
    pub data_dir: PathBuf,
    /// 設定ファイル本体
    pub config_file: PathBuf,
}

impl Paths {
    /// デフォルト + override で解決
    pub fn resolve(override_data: Option<&Path>, override_config: Option<&Path>) -> Result<Self> {
        let data_dir = if let Some(d) = override_data {
            d.to_path_buf()
        } else {
            default_data_dir()?
        };
        let config_file = override_config
            .map(PathBuf::from)
            .unwrap_or_else(|| data_dir.join("config.toml"));

        Ok(Self {
            data_dir,
            config_file,
        })
    }

    /// 辞書ルート: `<data_dir>/dict/`
    #[must_use]
    pub fn dict_dir(&self) -> PathBuf {
        self.data_dir.join("dict")
    }

    /// core 辞書: `<data_dir>/dict/core/`
    #[must_use]
    pub fn dict_core_dir(&self) -> PathBuf {
        self.dict_dir().join("core")
    }

    /// user 辞書: `<data_dir>/dict/user/`
    #[must_use]
    pub fn dict_user_dir(&self) -> PathBuf {
        self.dict_dir().join("user")
    }

    /// overrides ファイル: `<data_dir>/dict/overrides.tsv`
    #[must_use]
    pub fn overrides_file(&self) -> PathBuf {
        self.dict_dir().join("overrides.tsv")
    }
}

/// `--data-dir` / env 未指定時の data_dir 解決:
/// 1. 実行ファイル (`furigana.exe`) と同じディレクトリ
/// 2. 失敗時のみ XDG / `%LOCALAPPDATA%` (滅多に到達しない)
fn default_data_dir() -> Result<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return Ok(parent.to_path_buf());
        }
    }
    let project = ProjectDirs::from("com", "furigana", "furigana")
        .ok_or_else(|| anyhow!("プロジェクトディレクトリの解決に失敗"))?;
    Ok(project.data_dir().to_path_buf())
}
