//! # furigana
//!
//! 日本語フリガナ (ルビ) 解決ライブラリ。
//!
//! ## クイックスタート
//!
//! ```no_run
//! use furigana::Furigana;
//!
//! let mut f = Furigana::minimal().expect("init failed");
//! f.add_reading("灰桜", "ハイザクラ");
//! let ruby = f.to_ruby("灰桜の散る道");
//! // → "{灰桜|はいざくら}の{散る|ちる}{道|みち}"
//! ```
//!
//! ## アーキテクチャ
//!
//! 公開 API は [`Furigana`] / [`FuriganaBuilder`] で、内部は以下の module に分かれている:
//!
//! - [`analyzer`] : 形態素解析 (Lindera + IPADIC)
//! - [`kana`]     : ひら⇄カタ + Unicode 正規化
//! - [`numbers`]  : 数値処理 (digit / counter / phrase / extras)
//! - [`chunks`]   : テキスト全体の数値オーケストレーション
//! - [`reading`]  : 読み解決パイプライン (pipeline / merge / context / output)
//! - [`tts`]      : TTS 整形 + segment
//! - [`dict`]     : surface→reading 辞書
//! - [`rules`]    : ルールデータ型 (counters / context / scales / etc.)
//! - [`loader`]   : TOML ローダー
//!
//! ## ステータス
//!
//! Pre-alpha — 公開 API は変更される。

#![allow(clippy::tabs_in_doc_comments)]

pub mod analyzer;
pub mod chunks;
pub mod dict;
pub mod error;
pub mod kana;
pub mod loader;
pub mod numbers;
pub mod reading;
pub mod romaji;
pub mod rules;
pub mod tts;

mod api;
mod embedded;

pub use crate::api::{Furigana, FuriganaBuilder};
pub use crate::dict::Dict;
pub use crate::error::{FuriganaError, Result};
pub use crate::reading::{tokens_to_hiragana, tokens_to_ruby, ReadingToken};
pub use crate::romaji::{hiragana_to_romaji, RomajiStyle};
pub use crate::tts::TtsOptions;
