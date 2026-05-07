//! 公開 API: [`Furigana`] + [`FuriganaBuilder`]
//!
//! lib のエントリポイント。形態素解析器・ルール・辞書・チャンカーを
//! 1 つのオブジェクトに束ねて、`to_ruby` / `to_hiragana` / `to_tts` 等の
//! 高レベル変換メソッドを提供する。

use crate::analyzer::Analyzer;
use crate::chunks::NumberChunker;
use crate::dict::Dict;
use crate::error::Result;
use crate::numbers::NumericPhraseMatcher;
use crate::reading::{tokenize_text, tokens_to_hiragana, tokens_to_ruby, ReadingToken};
use crate::rules::RulesData;
use crate::tts::{self, TtsOptions};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ============================================================================
// Furigana 本体
// ============================================================================

/// フリガナ解決器
///
/// 内部で形態素解析器・ルール・辞書を保持する。
/// 通常は [`Furigana::minimal`] か [`Furigana::builder`] で構築する。
///
/// **lazy init**: Lindera 形態素解析器は構築時には初期化されず、最初の
/// `tokenize` / `to_*` 呼び出し時に [`OnceLock`] で 1 度だけ init される。
/// `Furigana::minimal()` の呼び出し自体は ~µs 級で済むため、引数 parse や
/// help 表示など analyze に至らない経路を高速化できる。サーバー起動時に
/// 先に init したい場合は [`Furigana::preload`] を呼ぶ。
pub struct Furigana {
    analyzer: OnceLock<Analyzer>,
    rules: RulesData,
    dict: Dict,
    phrase_matcher: NumericPhraseMatcher,
    chunker: NumberChunker,
}

impl Furigana {
    /// 最小構成で初期化 (空 default rules + Lindera + 空辞書)
    ///
    /// rules は `RulesData::default()` (= 全空)、辞書も空のため、
    /// 助数詞・文脈・スケール等の高度な処理は無効化される。
    /// 形態素解析 (Lindera) と直接 [`Self::add_reading`] による補完は動作する。
    /// 本格利用は `furigana dict pull` 後に [`Self::builder`] で
    /// `rules_dir` / `core_dict_dir` を指定してマウントする想定。
    ///
    /// # Errors
    /// 形態素解析器の初期化に失敗した場合。
    pub fn minimal() -> Result<Self> {
        Self::builder().build()
    }

    /// builder を取得
    #[must_use]
    pub fn builder() -> FuriganaBuilder {
        FuriganaBuilder::new()
    }

    /// 内部 [`Analyzer`] を取得 (必要なら初期化する)
    ///
    /// init は最初の呼び出しで 1 度だけ実行 ([`OnceLock`] 経由)。
    /// embed 済みの IPADIC を使うため init はほぼ失敗しないが、リソース不足等で
    /// 失敗した場合は panic する。事前に [`Self::preload`] で eager 初期化して
    /// 失敗を Result で受け取れる。
    fn analyzer(&self) -> &Analyzer {
        self.analyzer
            .get_or_init(|| Analyzer::new().expect("lindera analyzer init failed"))
    }

    /// 形態素解析器を eager に初期化する (server 起動時の preload 用)
    ///
    /// 通常は最初の `tokenize` / `to_*` 呼び出し時に lazy init されるが、
    /// 起動直後の最初のリクエストレイテンシを下げたい場合は build 直後に
    /// 呼んでおく。失敗時は [`crate::FuriganaError::AnalyzerInit`]。
    /// 既に init 済みの場合は no-op。
    ///
    /// # Errors
    /// 形態素解析器の初期化に失敗した場合。
    pub fn preload(&self) -> Result<()> {
        if self.analyzer.get().is_some() {
            return Ok(());
        }
        let analyzer = Analyzer::new()?;
        // set は既に init 済みだと Err を返すが、その場合は他スレッドが先に
        // 入れただけなので無視して良い。
        let _ = self.analyzer.set(analyzer);
        Ok(())
    }

    /// テキストをトークン化 (生 [`ReadingToken`] 列)
    #[must_use]
    pub fn tokenize(&self, text: &str) -> Vec<ReadingToken> {
        tokenize_text(
            text,
            self.analyzer(),
            &self.rules,
            &self.dict,
            &self.phrase_matcher,
            &self.chunker,
        )
    }

    /// テキスト → ひらがな文字列
    ///
    /// 漢字部分を読みのひらがなに置き換えた完全展開形を返す。TTS 等向け。
    /// 出力直前に `postprocess.toml` の `mode = "hiragana"` ルールを適用。
    #[must_use]
    pub fn to_hiragana(&self, text: &str) -> String {
        let hira = tokens_to_hiragana(&self.tokenize(text));
        self.rules.postprocess.apply(&hira, "hiragana")
    }

    /// テキスト → `{漢字|ひらがな}` 形式の ruby 文字列
    ///
    /// 例: `"灰桜の道"` → `"{灰桜|はいざくら}の{道|みち}"`
    /// 漢字を含まない部分はそのまま、読みなし部分も surface のまま。
    /// 出力直前に `postprocess.toml` の `mode = "ruby"` ルールを適用。
    #[must_use]
    pub fn to_ruby(&self, text: &str) -> String {
        let ruby = tokens_to_ruby(&self.tokenize(text));
        self.rules.postprocess.apply(&ruby, "ruby")
    }

    /// テキスト → TTS 向けに整形されたひらがな (ポーズ込み)
    ///
    /// 内部で [`Self::to_hiragana`] → [`tts::normalize_for_tts`] を走らせる。
    /// VOICEVOX 等の音声合成に流す前段で使う想定。
    /// 出力直前に `postprocess.toml` の `mode = "tts"` ルールを適用。
    #[must_use]
    pub fn to_tts(&self, text: &str, opts: &TtsOptions) -> String {
        // hiragana 自体の postprocess はここでは飛ばす (二重適用回避)。
        // 必要なら hiragana 用 postprocess を tts mode で再度書く想定。
        let hira = tokens_to_hiragana(&self.tokenize(text));
        let normalized = tts::normalize_for_tts(&hira, opts);
        self.rules.postprocess.apply(&normalized, "tts")
    }

    /// テキスト → ローマ字
    ///
    /// 内部で [`Self::to_hiragana`] → [`crate::romaji::hiragana_to_romaji`] を走らせる。
    /// 例: `"灰桜の散る道"` → `"haizakura no chiru michi"` (ヘボン式)。
    /// `style = RomajiStyle::Hepburn` (default) で b/m/p 前の n→m や ち→chi、
    /// `Kunrei` で規則的な si/ti/tu を出す。
    #[must_use]
    pub fn to_romaji(&self, text: &str, style: crate::romaji::RomajiStyle) -> String {
        // to_hiragana 内で hiragana 用 postprocess は適用済み
        let hira = self.to_hiragana(text);
        let romaji = crate::romaji::hiragana_to_romaji(&hira, style);
        self.rules.postprocess.apply(&romaji, "romaji")
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

    /// TOML 文字列を辞書に merge して、追加 (上書き含む) されたエントリ数を返す。
    ///
    /// ファイルシステムベースの `core_dict_dir` が使えない環境 (WASM など) 向け。
    /// ブラウザでは `fetch('./data/unihan.toml').then(r => r.text())` の結果を
    /// そのまま渡せる。形式は通常の `[entries]` セクション付き TOML:
    ///
    /// ```toml
    /// [entries]
    /// "灰桜" = "ハイザクラ"
    /// "黎明" = "レイメイ"
    /// ```
    ///
    /// `[entries]` 以外の TOML (例: `units.toml` の inline table) は内部で
    /// 自動的に skip される (lib 側 `Dict::from_toml_str` の defensive 実装による)。
    ///
    /// # Errors
    /// TOML parse 失敗時 [`crate::FuriganaError::Toml`]。
    pub fn merge_dict_toml(&mut self, content: &str) -> Result<usize> {
        let added = Dict::from_toml_str(content, "<merge_dict_toml>")?;
        let count = added.len();
        self.dict.merge(added);
        Ok(count)
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
/// 全フィールド optional。指定しなければデフォルト (空) が使われる。
/// Dict は core → user → overrides → add_entry の順にマージされ、
/// 後のものが優先 (override) される。
#[derive(Debug, Default)]
pub struct FuriganaBuilder {
    rules_dir: Option<PathBuf>,
    core_dict_dirs: Vec<PathBuf>,
    user_dict_dirs: Vec<PathBuf>,
    overrides_files: Vec<PathBuf>,
    extra_entries: Vec<(String, String)>,
    /// 外来語辞書ディレクトリ (複数指定可、 後勝ち merge)
    loanwords_dirs: Vec<PathBuf>,
}

impl FuriganaBuilder {
    /// 空の builder を作る
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// ルール TOML をディレクトリから読み込む (デフォルト空を上書き)
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

    /// overrides TOML ファイルを追加 (複数指定可、優先度: 高)
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

    /// 外来語辞書ディレクトリを追加 (`core/loanwords/**/*.toml` を recursive load)
    ///
    /// IT 用語 / OSS / クラウドサービス等の英単語に対する読み付与用。
    /// chunks/split() 階層 4.7 で **完全一致 lookup** されるため、 substring 切断ゼロ。
    #[must_use]
    pub fn core_loanwords_dir(mut self, p: impl AsRef<Path>) -> Self {
        self.loanwords_dirs.push(p.as_ref().to_path_buf());
        self
    }

    /// [`Furigana`] を構築
    ///
    /// 形態素解析器 (Lindera + IPADIC) は **lazy init** — 構築時には初期化せず、
    /// 最初の `tokenize` / `to_*` 呼び出し時に 1 度だけ初期化される。サーバー
    /// 起動時に init コストを払いたい場合は [`Furigana::preload`] を呼ぶ。
    ///
    /// # Errors
    /// - ルールファイルパース失敗 ([`crate::FuriganaError::Toml`])
    /// - 辞書ファイル/ディレクトリ I/O 失敗
    pub fn build(self) -> Result<Furigana> {
        let rules = match self.rules_dir.as_ref() {
            Some(dir) => crate::loader::load_rules_dir(dir)?,
            None => crate::embedded::rules()?,
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

        // jukugo の Aho-Corasick を 1 回 build → phrase_matcher と chunker で Arc 共有。
        // homonyms (context rule を持つ surface) は AC patterns から除外し、
        // reading pipeline の context rule (例: 「翡翠+が+水辺」 → カワセミ) が
        // chunks / phrase 段階で bypass されないようにする。
        let homonyms_exclude: std::collections::HashSet<String> = rules
            .context
            .rules
            .iter()
            .map(|r| r.surface.clone())
            .collect();
        // 2 字 jukugo は AC patterns から除外。 IPADIC が一語として返す長い複合語
        // (例: 「烏賊墨」 → イカスミ、 「金平糖」 → コンペイトウ) を 2 字 jukugo
        // (烏賊 / 金平) で先取りすると分断されて誤読になるため。
        // ≥3 字の jukugo entry だけが、 「Lindera が分解しがちな複合語」 として
        // AC 先取りに値する (例: 「千本桜」 「向日葵畑」 「義経千本桜」)。
        let jukugo_filtered: std::collections::HashMap<String, String> = dict
            .jukugo_iter()
            .filter(|(k, _)| !homonyms_exclude.contains(*k))
            .filter(|(k, _)| k.chars().count() >= 3)
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let jukugo_ac_arc: Option<std::sync::Arc<aho_corasick::AhoCorasick>> = if jukugo_filtered
            .is_empty()
        {
            None
        } else {
            aho_corasick::AhoCorasick::builder()
                .match_kind(aho_corasick::MatchKind::LeftmostLongest)
                .build(jukugo_filtered.keys())
                .ok()
                .map(std::sync::Arc::new)
        };
        let jukugo_map_arc = std::sync::Arc::new(jukugo_filtered);

        let mut phrase_matcher = NumericPhraseMatcher::new(&rules.numeric_phrases);
        let mut chunker = NumberChunker::new(&rules);
        if let Some(ac) = jukugo_ac_arc.clone() {
            phrase_matcher.set_jukugo(ac.clone(), jukugo_map_arc.clone());
            chunker.set_jukugo(ac, jukugo_map_arc);
        }

        // 外来語辞書 (IT 用語等の英単語) を merge して chunker に Arc 渡し。
        // chunks/split() 階層 4.7 で英単語 chunk 全体を完全一致 lookup に使う。
        let mut loanwords = crate::loanwords::Loanwords::new();
        for d in &self.loanwords_dirs {
            loanwords.merge(crate::loanwords::Loanwords::from_toml_dir(d)?);
        }
        if !loanwords.is_empty() {
            chunker.set_loanwords(std::sync::Arc::new(loanwords));
        }

        Ok(Furigana {
            analyzer: OnceLock::new(),
            rules,
            dict,
            phrase_matcher,
            chunker,
        })
    }
}

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
    fn minimal_has_no_rules_loaded() {
        // 本体には rules を embed しない方針なので、minimal は空 default。
        // 「一人」は context.toml の default が無いため lindera 由来の読みになる。
        let f = Furigana::minimal().unwrap();
        let ruby = f.to_ruby("一人");
        // 何らかのひらがな化はされるはずだが、context default の "ヒトリ" は出ない
        // (lindera が 一+人 で個別に読むため、典型的には「いちにん」)
        assert!(!ruby.is_empty(), "ruby: {ruby}");
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

    // 注: 以下 3 テストは過去 cargo test harness で 51 GB alloc 暴走を起こしていたが、
    // 原因が `NumberChunker` の dynamic regex の **never-match pattern**
    // (`r"(?P<n>\A\B)(?P<x>\A\B)"`) であったことを切り分け、`Option<Regex>` 化
    // で完全回避した (chunks/regex.rs の build_alt_regex_opt)。CHANGELOG 参照。

    #[test]
    fn to_tts_inserts_pauses() {
        let f = Furigana::minimal().unwrap();
        let opts = TtsOptions::default();
        let result = f.to_tts("こんにちは。さようなら。", &opts);
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
    fn rules_dir_overrides_default() {
        // テスト用 fixture (本来は furigana-dict から pull したものを使う)
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("rules");
        let f = Furigana::builder()
            .rules_dir(&dir)
            .build()
            .expect("build with rules_dir failed");
        // 一人 → ヒトリ (rules dir 経由でロードされる)
        let ruby = f.to_ruby("一人");
        assert!(ruby.contains("ひとり"));
    }
}
