//! # furigana
//!
//! 日本語フリガナ (ルビ) 解決ライブラリ。
//!
//! 助数詞・連濁・文脈依存読み等の全ルールは TOML/TSV データファイルから
//! 読み込まれる。本 crate にはビルド時に default ルールが埋め込まれる
//! ため、`Furigana::minimal()` で zero-config 起動可能。
//!
//! ## クイックスタート
//!
//! ```no_run
//! use furigana::Furigana;
//!
//! let mut f = Furigana::minimal().expect("init failed");
//! f.add_reading("灰桜", "ハイザクラ");
//! let ruby = f.to_ruby("灰桜の散る道");
//! // → "{灰桜|はいざくら}の{散|ち}る{道|みち}"
//! ```
//!
//! ## ステータス
//! Pre-alpha — 公開 API は変更される。

// TSV 例を doc comment に書く際タブを使うため、clippy の lint を抑制
#![allow(clippy::tabs_in_doc_comments)]

pub mod analyzer;
pub mod chunks;
pub mod dict;
pub mod error;
pub mod kana;
pub mod loader;
pub mod numbers;
pub mod reading;
pub mod rules;
pub mod tts;

mod embedded;

pub use crate::dict::Dict;
pub use crate::error::{FuriganaError, Result};
pub use crate::reading::{tokens_to_hiragana, tokens_to_ruby, ReadingToken};
pub use crate::tts::TtsOptions;

use crate::analyzer::Analyzer;
use crate::chunks::NumberChunker;
use crate::numbers::NumericPhraseMatcher;
use crate::reading::tokenize_text;
use crate::rules::RulesData;
use std::path::{Path, PathBuf};

// ============================================================================
// Furigana 本体
// ============================================================================

/// フリガナ解決器
///
/// 内部で形態素解析器・ルール・辞書を保持する。
/// 通常は [`Furigana::minimal`] か [`Furigana::builder`] で構築する。
pub struct Furigana {
    analyzer: Analyzer,
    rules: RulesData,
    dict: Dict,
    phrase_matcher: NumericPhraseMatcher,
    chunker: NumberChunker,
}

impl Furigana {
    /// 最小構成で初期化 (埋め込みルール + Lindera + 空辞書)
    ///
    /// ファイル I/O 一切無し。組み込み用途向けの zero-config 起動。
    ///
    /// # Errors
    /// 形態素解析器の初期化に失敗した場合 / 埋め込みルールのパース失敗時
    /// (後者は CI 通過済みなので通常起きない)。
    pub fn minimal() -> Result<Self> {
        Self::builder().build()
    }

    /// builder を取得
    #[must_use]
    pub fn builder() -> FuriganaBuilder {
        FuriganaBuilder::new()
    }

    /// テキストをトークン化 (生 [`ReadingToken`] 列)
    #[must_use]
    pub fn tokenize(&self, text: &str) -> Vec<ReadingToken> {
        tokenize_text(
            text,
            &self.analyzer,
            &self.rules,
            &self.dict,
            &self.phrase_matcher,
            &self.chunker,
        )
    }

    /// テキスト → ひらがな文字列
    ///
    /// 漢字部分を読みのひらがなに置き換えた完全展開形を返す。TTS 等向け。
    #[must_use]
    pub fn to_hiragana(&self, text: &str) -> String {
        tokens_to_hiragana(&self.tokenize(text))
    }

    /// テキスト → `{漢字|ひらがな}` 形式の ruby 文字列
    ///
    /// 例: `"灰桜の道"` → `"{灰桜|はいざくら}の{道|みち}"`
    /// 漢字を含まない部分はそのまま、読みなし部分も surface のまま。
    #[must_use]
    pub fn to_ruby(&self, text: &str) -> String {
        tokens_to_ruby(&self.tokenize(text))
    }

    /// テキスト → TTS 向けに整形されたひらがな (ポーズ込み)
    ///
    /// 内部で [`Self::to_hiragana`] → [`tts::normalize_for_tts`] を走らせる。
    /// VOICEVOX 等の音声合成に流す前段で使う想定。
    #[must_use]
    pub fn to_tts(&self, text: &str, opts: &TtsOptions) -> String {
        let hira = self.to_hiragana(text);
        tts::normalize_for_tts(&hira, opts)
    }

    /// TTS 出力を文末・読点で分割
    ///
    /// `max_segment_len` 以内のチャンクに分割した配列を返す。
    /// VOICEVOX 等の文字数制限対策。
    #[must_use]
    pub fn segment_tts(
        &self,
        text: &str,
        opts: &TtsOptions,
        max_segment_len: usize,
    ) -> Vec<String> {
        let normalized = self.to_tts(text, opts);
        tts::segment_for_tts(&normalized, max_segment_len)
    }

    /// 動的に辞書エントリを追加 (override 用途)
    pub fn add_reading(&mut self, surface: impl Into<String>, reading: impl Into<String>) {
        self.dict.insert(surface, reading);
    }

    /// 内部辞書のサイズ (デバッグ用)
    #[must_use]
    pub fn dict_size(&self) -> usize {
        self.dict.len()
    }
}

impl std::fmt::Debug for Furigana {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Furigana")
            .field("dict_size", &self.dict.len())
            .field("context_rules", &self.rules.context.rules.len())
            .finish_non_exhaustive()
    }
}

// ============================================================================
// FuriganaBuilder
// ============================================================================

/// [`Furigana`] を段階的に構築する builder
///
/// 全フィールド optional。指定しなければ embed/空が使われる。
/// Dict は core → user → overrides の順にマージされ、後から追加した
/// ものが優先 (override) される。
#[derive(Debug, Default)]
pub struct FuriganaBuilder {
    rules_dir: Option<PathBuf>,
    core_dict_dirs: Vec<PathBuf>,
    user_dict_dirs: Vec<PathBuf>,
    overrides_files: Vec<PathBuf>,
    extra_entries: Vec<(String, String)>,
}

impl FuriganaBuilder {
    /// 空の builder を作る
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// ルール TOML/TSV を別ディレクトリから読む (embed を上書き)
    #[must_use]
    pub fn rules_dir(mut self, p: impl AsRef<Path>) -> Self {
        self.rules_dir = Some(p.as_ref().to_path_buf());
        self
    }

    /// core 辞書ディレクトリを追加 (複数指定可、優先度: 低)
    #[must_use]
    pub fn core_dict_dir(mut self, p: impl AsRef<Path>) -> Self {
        self.core_dict_dirs.push(p.as_ref().to_path_buf());
        self
    }

    /// user 辞書ディレクトリを追加 (複数指定可、優先度: 中)
    #[must_use]
    pub fn user_dict_dir(mut self, p: impl AsRef<Path>) -> Self {
        self.user_dict_dirs.push(p.as_ref().to_path_buf());
        self
    }

    /// overrides TSV ファイルを追加 (複数指定可、優先度: 高)
    #[must_use]
    pub fn overrides_file(mut self, p: impl AsRef<Path>) -> Self {
        self.overrides_files.push(p.as_ref().to_path_buf());
        self
    }

    /// 個別エントリをコード上で追加 (優先度: 最高)
    #[must_use]
    pub fn add_entry(mut self, surface: impl Into<String>, reading: impl Into<String>) -> Self {
        self.extra_entries.push((surface.into(), reading.into()));
        self
    }

    /// [`Furigana`] を構築
    ///
    /// # Errors
    /// - ルールファイルパース失敗 ([`FuriganaError::Toml`] / [`FuriganaError::Tsv`])
    /// - 辞書ファイル/ディレクトリ I/O 失敗
    /// - 形態素解析器初期化失敗 ([`FuriganaError::AnalyzerInit`])
    pub fn build(self) -> Result<Furigana> {
        let analyzer = Analyzer::new()?;

        let rules = match self.rules_dir.as_ref() {
            Some(dir) => crate::loader::load_rules_dir(dir)?,
            None => embedded::rules()?,
        };

        let mut dict = Dict::new();
        for d in &self.core_dict_dirs {
            dict.merge(Dict::from_toml_dir(d)?);
        }
        for d in &self.user_dict_dirs {
            dict.merge(Dict::from_toml_dir(d)?);
        }
        for f in &self.overrides_files {
            dict.merge(Dict::from_toml_file(f)?);
        }
        for (s, r) in self.extra_entries {
            dict.insert(s, r);
        }

        let phrase_matcher = NumericPhraseMatcher::new(&rules.numeric_phrases);
        let chunker = NumberChunker::new(&rules);

        Ok(Furigana {
            analyzer,
            rules,
            dict,
            phrase_matcher,
            chunker,
        })
    }
}

// ============================================================================
// テスト
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_init_works() {
        let f = Furigana::minimal().expect("minimal init failed");
        // 漢字無しの入力は素通し
        assert_eq!(f.to_ruby("こんにちは"), "こんにちは");
    }

    #[test]
    fn add_reading_then_to_ruby() {
        let mut f = Furigana::minimal().unwrap();
        f.add_reading("灰桜", "ハイザクラ");
        let ruby = f.to_ruby("灰桜");
        assert!(ruby.contains("はいざくら"), "ruby: {ruby}");
    }

    #[test]
    fn builder_with_extra_entries() {
        let f = Furigana::builder()
            .add_entry("灰桜", "ハイザクラ")
            .add_entry("黎明", "レイメイ")
            .build()
            .unwrap();
        assert_eq!(f.dict_size(), 2);

        let ruby = f.to_ruby("灰桜と黎明");
        assert!(ruby.contains("はいざくら"));
        assert!(ruby.contains("れいめい"));
    }

    #[test]
    fn to_hiragana_basic() {
        let mut f = Furigana::minimal().unwrap();
        f.add_reading("灰桜", "ハイザクラ");
        let h = f.to_hiragana("灰桜の道");
        assert!(h.starts_with("はいざくら"), "h: {h}");
    }

    #[test]
    fn embed_rules_loaded_by_default() {
        let f = Furigana::minimal().unwrap();
        // 一人 → ヒトリ (context.toml の default)
        let ruby = f.to_ruby("一人");
        assert!(ruby.contains("ひとり"), "ruby: {ruby}");
    }

    #[test]
    fn empty_input_yields_empty() {
        let f = Furigana::minimal().unwrap();
        assert_eq!(f.to_ruby(""), "");
        assert_eq!(f.to_hiragana(""), "");
        assert!(f.tokenize("").is_empty());
    }

    #[test]
    fn debug_format_shows_summary() {
        let f = Furigana::minimal().unwrap();
        let s = format!("{f:?}");
        assert!(s.contains("Furigana"));
        assert!(s.contains("dict_size"));
    }

    #[test]
    fn to_tts_inserts_pauses() {
        let f = Furigana::minimal().unwrap();
        let opts = TtsOptions::default();
        let result = f.to_tts("こんにちは。さようなら。", &opts);
        // ひらがな化後に TTS 整形 → 句点後に pause (default は最終的に 1 スペースに圧縮)
        assert!(result.contains("こんにちは。 "), "result: {result}");
    }

    #[test]
    fn to_tts_with_non_space_marker_preserves_long_pause() {
        let f = Furigana::minimal().unwrap();
        let opts = TtsOptions {
            short_pause: "<s>".to_string(),
            long_pause: "<l>".to_string(),
            keep_period: true,
        };
        let result = f.to_tts("こんにちは。さよなら。", &opts);
        assert!(result.contains("こんにちは。<l>"), "result: {result}");
    }

    #[test]
    fn segment_tts_returns_vec() {
        let f = Furigana::minimal().unwrap();
        let opts = TtsOptions::default();
        let segs = f.segment_tts("ぶん1。ぶん2。ぶん3。", &opts, 60);
        assert_eq!(segs.len(), 3);
    }

    #[test]
    fn rules_dir_overrides_embedded() {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = manifest
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data")
            .join("rules");
        let f = Furigana::builder()
            .rules_dir(&dir)
            .build()
            .expect("build with rules_dir failed");
        // 一人 → ヒトリ (rules dir 経由でも同じ動作)
        let ruby = f.to_ruby("一人");
        assert!(ruby.contains("ひとり"));
    }
}
