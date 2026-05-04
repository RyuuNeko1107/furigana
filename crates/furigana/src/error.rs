//! エラー型

use thiserror::Error;

/// crate 共通エラー
#[derive(Debug, Error)]
pub enum FuriganaError {
    /// I/O 失敗 (辞書 / ルールファイル読み込み等)
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML パース失敗
    #[error("TOML parse error in {file}: {source}")]
    Toml {
        /// 対象ファイル名
        file: String,
        /// 元のエラー
        #[source]
        source: toml::de::Error,
    },

    /// TSV パース失敗
    #[error("TSV parse error in {file} at line {line}: {message}")]
    Tsv {
        /// 対象ファイル名
        file: String,
        /// エラー行番号 (1 始まり)
        line: usize,
        /// 詳細メッセージ
        message: String,
    },

    /// バリデーション失敗 (重複キー / スキーマ不整合等)
    #[error("Validation error: {0}")]
    Validation(String),

    /// 形態素解析器初期化失敗
    #[error("Analyzer init failed: {0}")]
    AnalyzerInit(String),
}

/// crate 共通 Result
pub type Result<T> = std::result::Result<T, FuriganaError>;
