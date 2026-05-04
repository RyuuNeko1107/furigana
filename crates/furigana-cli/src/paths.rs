//! データディレクトリ・設定パスの解決
//!
//! 優先順位: CLI 引数 > 環境変数 > XDG / `%LOCALAPPDATA%`
//!
//! - **Linux / macOS**: `~/.local/share/furigana/` (XDG_DATA_HOME 準拠)
//! - **Windows**: `%LOCALAPPDATA%\furigana\furigana\`
//!
//! 設定は `data_dir` とは別の `config_dir` に置く慣例 (XDG_CONFIG_HOME)。

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
        let project = ProjectDirs::from("com", "furigana", "furigana")
            .ok_or_else(|| anyhow!("プロジェクトディレクトリの解決に失敗"))?;

        let data_dir = override_data
            .map(PathBuf::from)
            .unwrap_or_else(|| project.data_dir().to_path_buf());
        let config_file = override_config
            .map(PathBuf::from)
            .unwrap_or_else(|| project.config_dir().join("config.toml"));

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
