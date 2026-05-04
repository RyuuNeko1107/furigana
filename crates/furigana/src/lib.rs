//! # furigana
//!
//! 日本語フリガナ (ルビ) 解決ライブラリ。
//!
//! 助数詞・連濁・文脈依存読み等の全ルールは `data/rules/` 配下の TOML/TSV から
//! 読み込まれる。本 crate はそれらルールを表現する型と、形態素解析 (Lindera) /
//! 辞書ルックアップ / kana 変換 のロジックを提供する。
//!
//! ## ステータス
//! Pre-alpha — 公開 API は変更される。

// TSV 例を doc comment に書く際タブを使うため、clippy の lint を抑制
#![allow(clippy::tabs_in_doc_comments)]

pub mod analyzer;
pub mod dict;
pub mod error;
pub mod kana;
pub mod loader;
pub mod numbers;
pub mod reading;
pub mod rules;

pub use error::{FuriganaError, Result};
