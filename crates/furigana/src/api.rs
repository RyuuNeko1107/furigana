//! 公開 API: [`Furigana`] + [`FuriganaBuilder`]
//!
//! lib のエントリポイント。形態素解析器・ルール・辞書・チャンカーを
//! 1 つのオブジェクトに束ねて、`to_ruby` / `to_hiragana` / `to_tts` 等の
//! 高レベル変換メソッドを提供する。

use crate::analyzer::Analyzer;
use crate::dict::Dict;
use crate::error::Result;
use crate::reading::{tokens_to_hiragana, tokens_to_ruby, ReadingToken};
use crate::rules::RulesData;
use crate::scoring::analyze::{analyze as scoring_analyze, AnalyzeResult};
use crate::scoring::boundary::BoundaryAnalysis;
use crate::scoring::bracket::strip_intonation_markers;
use crate::scoring::candidate::{Candidate, CandidateProvider, Score};
use crate::scoring::lindera_fallback::LinderaFallbackProvider;
use crate::scoring::matcher::{
    next2_logical_token, next_logical_token, prev_logical_token, MatchContext,
};
use crate::scoring::numbers::NumberCandidateProvider;
use crate::scoring::odoriji::{apply_rendaku_to_result, OdorijiProvider};
use crate::scoring::special::{AlphabetPassthroughProvider, ProtectTokenProvider};
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
    /// Smart engine 用の数字系 candidate provider (★C3、 band 950)
    ///
    /// `analyze()` で provider として使う。 rules を pre-compile して保持、
    /// 各 `analyze()` 呼び出しごとに regex compile しない。
    number_provider: NumberCandidateProvider,
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
    ///
    /// 内部で [`Self::analyze`] を呼び (= Smart engine path + Lindera fallback)、
    /// [`AnalyzeToken`] を [`ReadingToken`] に変換して返す。
    ///
    /// `to_hiragana` / `to_ruby` / `to_tts` / `to_romaji` は内部で本 method を呼ぶので、
    /// production の reading 解決経路はすべて本 method 経由。
    ///
    /// analyze の reading は常に String (空ではあり得るが None ではない)、 一律
    /// `Some(reading)` で包む。 reading が surface と kana 等価 (= 「の」 + 「ノ」) の
    /// ケースは [`tokens_to_hiragana`] / [`tokens_to_ruby`] 側で 「surface そのまま」
    /// と判定される。
    #[must_use]
    pub fn tokenize(&self, text: &str) -> Vec<ReadingToken> {
        let result = self.analyze(text);
        result
            .tokens
            .into_iter()
            .map(|t| ReadingToken {
                surface: t.surface,
                reading: Some(t.reading),
            })
            .collect()
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

    /// Smart engine で input を analyze、 採択 path / 候補 / boundary region を返す (★F1)。
    ///
    /// `to_hiragana` 等の本流 method は alpha 期間中 Strict engine 経由だが、
    /// 本 method は **常に Smart engine** で動作する debug / inspection API。
    /// `engine()` setting に依らず Smart 結果を返す (= caller が明示的に
    /// Smart 解析を要求している前提)。
    ///
    /// ## 構成 provider (alpha.10 段階 + alpha.13 追加)
    ///
    /// - [`ProtectTokenProvider`] (URL / Email / 絵文字、 band 2000)
    /// - [`AlphabetPassthroughProvider`] (英字 passthrough、 hit は band 1000 / miss は band 100)
    /// - [`DictBridgeProvider`] (= self.dict 経由、 jukugo は band 1000 / unihan は band 100)
    /// - [`NumberCandidateProvider`] (数字 + 助数詞 / 大数スケール / SI 単位 / 日付 / 時刻 /
    ///   記号 / 素の数字、 band 950)
    /// - [`OdorijiProvider`] (々 placeholder edge、 band 100、 post-pass で連濁適用)
    /// - [`LinderaFallbackProvider`] (★alpha.13、 Lindera + IPADIC、 band 50): 他 provider が
    ///   一切覆わない位置 (= 助詞 / okurigana / dict 未登録 単語) を埋める safety net
    ///
    /// loanwords 検索は alpha.10 では未統合 (= AlphabetPassthrough は lookup 無しの passthrough_only)。
    /// `numeric_phrases` (二十歳=ハタチ 等) も別 provider 化が望ましいが C3 scope 外。
    ///
    /// ## 戻り値
    ///
    /// [`AnalyzeResult`] (= ★11 freeze、 0.1.0 stable で additive 追加のみ可)。
    ///
    /// 入力空 / 全 field 空、 path 構築不能 (= dict / 特殊処理で覆い切れない) →
    /// `tokens` / `path_indices` 空 で `candidates` / `boundary_regions` のみ返る。
    ///
    /// 「々」 token の reading は path 確定後に直前 token reading + 連濁判定で書き換え
    /// ([`crate::scoring::odoriji::apply_rendaku_to_result`])、 placeholder の 「々」 は残らない。
    #[must_use]
    pub fn analyze(&self, input: &str) -> AnalyzeResult {
        let protect = ProtectTokenProvider::new(input);
        let alphabet = AlphabetPassthroughProvider::passthrough_only(input);
        let dict_bridge = DictBridgeProvider::new(&self.dict);
        let odoriji = OdorijiProvider::new();
        // ★alpha.13: Lindera fallback (band 50) = 他 provider が一切覆わない位置の safety net。
        // construction で input 全体を tokenize、 各 candidates_at は edge 配列 lookup のみ。
        let lindera = LinderaFallbackProvider::new(self.analyzer(), input);
        let providers: [&dyn CandidateProvider; 6] = [
            &protect,
            &alphabet,
            &dict_bridge,
            &self.number_provider,
            &odoriji,
            &lindera,
        ];

        let boundary =
            BoundaryAnalysis::analyze(input, |surface| self.dict.lookup_jukugo(surface).is_some());

        let mut result = scoring_analyze(input, &providers, Some(&boundary));
        apply_rendaku_to_result(&mut result);
        result
    }
}

// ============================================================================
// DictBridgeProvider — Dict (jukugo + unihan) を CandidateProvider に橋渡し
// ============================================================================

/// 既存 [`Dict`] を [`CandidateProvider`] として scoring engine に流す bridge。
///
/// alpha.10 段階の transitional impl: Dict (= 旧 format、 simple HashMap) が
/// 0.1.0 stable で新 format (`scoring::format::Entry`) に置き換わるまでの繋ぎ。
///
/// ## band 割り当て
///
/// - jukugo (≥ 2 文字 surface) → [`Score::dict_exact`] (band 1000)
/// - unihan (= 1 文字 surface) → [`Score::kanji`] (band 100)
///
/// reading は [`strip_intonation_markers`] で bracket marker を除去
/// (forward compat for 0.2.0)。
///
/// ## 計算量
///
/// `candidates_at(pos)` は jukugo を全件 prefix match scan する naive 実装で
/// O(M)、 input 全体で O(N × M)。 alpha.10 wire-up 用、 必要なら 0.1.0-rc1 で
/// trie / Aho-Corasick 化。
struct DictBridgeProvider<'a> {
    dict: &'a Dict,
}

impl<'a> DictBridgeProvider<'a> {
    fn new(dict: &'a Dict) -> Self {
        Self { dict }
    }
}

impl<'a> CandidateProvider for DictBridgeProvider<'a> {
    fn candidates_at(&self, input: &str, pos: usize) -> Vec<Candidate> {
        let mut out = Vec::new();
        let tail = &input[pos..];

        // ★A2 alpha.12: rich_iter で 全 entry を walk、 prefix match した entry に
        // ついて MatchCondition を評価 (= 文脈分岐 reading を Smart engine に乗せる)
        let mut emitted_at_pos: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for (surface, entry) in self.dict.rich_iter() {
            if !tail.starts_with(surface) {
                continue;
            }
            let surface_byte_len = surface.len();
            let end_pos = pos + surface_byte_len;
            let char_count = surface.chars().count();
            let length = u8::try_from(char_count).unwrap_or(u8::MAX);

            // MatchContext build: char-class-based pseudo-token segmentation
            // (Lindera 不要、 Smart engine 経路で自前で context 取得)
            let prev = prev_logical_token(input, pos);
            let next = next_logical_token(input, end_pos);
            let next2 = next2_logical_token(input, end_pos);
            let ctx = MatchContext::with_all(
                if prev.is_empty() { None } else { Some(prev) },
                if next.is_empty() { None } else { Some(next) },
                if next2.is_empty() { None } else { Some(next2) },
            );

            // match block 列を順次評価、 第一 hit の reading を採用
            let reading = entry
                .matches()
                .iter()
                .find(|m| m.condition.matches_context(&ctx))
                .map(|m| m.reading.as_str())
                .unwrap_or_else(|| entry.default_reading());

            // band: 1 字 surface は band 100 (= kanji fallback)、 2+ 字 は band 1000 (= dict_exact)
            let score = if char_count == 1 {
                Score::kanji(length)
            } else {
                Score::dict_exact(length)
            };

            out.push(Candidate::new(
                surface.to_string(),
                strip_intonation_markers(reading),
                pos..end_pos,
                score,
            ));
            emitted_at_pos.insert(surface.to_string());
        }

        // ★A2 alpha.12: [[kanji]] block (= core/kanji/*.toml) を walk して
        // 1 文字 surface に文脈分岐 reading を提供。 rich_iter / unihan と異なり
        // ファイル毎の per-char default + match 配列を持つので、 rich で拾い損ねた
        // 1 字 surface はここで MatchCondition 評価ありで band 100 candidate 化。
        if let Some(c) = tail.chars().next() {
            let char_len = c.len_utf8();
            let surface = &tail[..char_len];
            let end_pos = pos + char_len;
            let prev = prev_logical_token(input, pos);
            let next = next_logical_token(input, end_pos);
            let next2 = next2_logical_token(input, end_pos);
            let ctx = MatchContext::with_all(
                if prev.is_empty() { None } else { Some(prev) },
                if next.is_empty() { None } else { Some(next) },
                if next2.is_empty() { None } else { Some(next2) },
            );
            for block in self.dict.kanji_iter() {
                if block.char != surface {
                    continue;
                }
                if emitted_at_pos.contains(surface) {
                    // rich 側で既に同 surface を band 100 で emit している
                    // (= [[kanji]] と entries の重複登録 ケース)、 skip して duplicate 回避
                    continue;
                }
                let reading = block
                    .matches
                    .iter()
                    .find(|m| m.condition.matches_context(&ctx))
                    .map(|m| m.reading.as_str())
                    .unwrap_or(block.default.as_str());
                out.push(Candidate::new(
                    surface.to_string(),
                    strip_intonation_markers(reading),
                    pos..end_pos,
                    Score::kanji(1),
                ));
                emitted_at_pos.insert(surface.to_string());
            }
        }

        // unihan fallback: rich / [[kanji]] block 両方に無い 1 文字 surface
        // (= unihan/*.toml で flat 登録された entry) を band 100 で出す。
        if let Some(c) = tail.chars().next() {
            let char_len = c.len_utf8();
            let surface = &tail[..char_len];
            if !emitted_at_pos.contains(surface) {
                if let Some(reading) = self.dict.lookup_unihan(surface) {
                    out.push(Candidate::new(
                        surface.to_string(),
                        strip_intonation_markers(reading),
                        pos..pos + char_len,
                        Score::kanji(1),
                    ));
                }
            }
        }

        out
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

        // Smart engine 用の数字系 provider (★C3): rules を pre-compile して保持。
        // analyze() 呼び出しごとに rebuild すると regex compile cost が乗るので一度作る。
        let number_provider = NumberCandidateProvider::new(&rules);

        Ok(Furigana {
            analyzer: OnceLock::new(),
            rules,
            dict,
            number_provider,
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
        // 3本 → サンボン (counters.toml 由来、 NumberCandidateProvider が hit)
        let hira = f.to_hiragana("3本");
        assert!(hira.contains("さんぼん"), "hiragana: {hira}");
    }

    // ─── Smart engine wire-up sanity tests ───────────────────────────────────

    #[test]
    fn to_ruby_uses_dict_then_lindera_fallback() {
        // 「灰桜の道」 → 灰桜 (dict band 1000、 ハイザクラ) + の (Lindera band 50、 ノ)
        // + 道 (Lindera band 50、 ミチ) → "{灰桜|はいざくら}の{道|みち}"
        let f = Furigana::builder()
            .add_entry("灰桜", "ハイザクラ")
            .build()
            .unwrap();
        let ruby = f.to_ruby("灰桜の道");
        assert!(ruby.contains("{灰桜|はいざくら}"), "expected ruby: {ruby}");
    }

    // ─── analyze() (F1) tests ────────────────────────────────────────────────

    #[test]
    fn analyze_empty_input_yields_empty_result() {
        let f = Furigana::minimal().unwrap();
        let r = f.analyze("");
        assert!(r.tokens.is_empty());
        assert!(r.candidates.is_empty());
        assert!(r.path_indices.is_empty());
        assert!(r.boundary_regions.is_empty());
    }

    #[test]
    fn analyze_single_jukugo_entry_yields_one_token() {
        let mut f = Furigana::minimal().unwrap();
        f.add_reading("灰桜", "ハイザクラ");
        let r = f.analyze("灰桜");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].surface, "灰桜");
        assert_eq!(r.tokens[0].reading, "ハイザクラ");
        assert_eq!(r.tokens[0].range, 0..6); // UTF-8 3 bytes × 2
        assert_eq!(r.path_indices, vec![0]);
    }

    #[test]
    fn analyze_jukugo_prefers_longer_match_over_unihan() {
        // 「灰桜」 jukugo (band 1000、 length 2) が
        // 「灰」 unihan + 「桜」 unihan (各 band 100) を path レベルで上回る
        let mut f = Furigana::minimal().unwrap();
        f.add_reading("灰桜", "ハイザクラ");
        f.add_reading("灰", "ハイ");
        f.add_reading("桜", "サクラ");
        let r = f.analyze("灰桜");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].reading, "ハイザクラ");
    }

    #[test]
    fn analyze_unihan_fallback_when_no_jukugo() {
        let mut f = Furigana::minimal().unwrap();
        f.add_reading("猫", "ネコ");
        let r = f.analyze("猫");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].surface, "猫");
        assert_eq!(r.tokens[0].reading, "ネコ");
    }

    #[test]
    fn analyze_url_protected_token_passthrough() {
        let f = Furigana::minimal().unwrap();
        let input = "https://example.com";
        let r = f.analyze(input);
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].surface, input);
        assert_eq!(r.tokens[0].reading, input); // passthrough
    }

    #[test]
    fn analyze_alphabet_passthrough_returns_surface() {
        let f = Furigana::minimal().unwrap();
        let r = f.analyze("API");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].surface, "API");
        assert_eq!(r.tokens[0].reading, "API"); // passthrough_only (lookup 無し)
    }

    #[test]
    fn analyze_falls_back_to_lindera_when_no_other_provider_covers() {
        // alpha.13 以前: 「猫が好き」 のような ひらがな混在 input は dict / 保護 /
        // 英字 のどれも cover せず path 構築不能だった。
        // alpha.13+ : Lindera fallback (band 50) が input 全体を tokenize、
        // 他 provider が空の位置を埋めるので path が必ず構築される (safety net)。
        let f = Furigana::minimal().unwrap();
        let r = f.analyze("猫が好き");
        assert!(
            !r.tokens.is_empty(),
            "Lindera fallback should cover input: {r:?}"
        );
        // path 全体を Lindera で覆ったので token 列が input を完全に span するはず
        let total_len: usize = r.tokens.iter().map(|t| t.range.end - t.range.start).sum();
        assert_eq!(total_len, "猫が好き".len());
    }

    #[test]
    fn analyze_emits_boundary_region_for_kanji_run() {
        let mut f = Furigana::minimal().unwrap();
        f.add_reading("灰桜", "ハイザクラ");
        let r = f.analyze("灰桜");
        // 漢字 2 字連続 region として検出される
        assert_eq!(r.boundary_regions.len(), 1);
        assert_eq!(r.boundary_regions[0], 0..6);
    }

    #[test]
    fn analyze_strips_intonation_brackets_from_reading() {
        let mut f = Furigana::minimal().unwrap();
        // 0.2.0 forward compat: bracket marker は lib 側で strip される
        f.add_reading("灰桜", "ハ[イザクラ");
        let r = f.analyze("灰桜");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].reading, "ハイザクラ");
    }

    #[test]
    fn analyze_expands_odoriji_with_rendaku() {
        // 神々 → カミ + ガミ (連濁あり)
        let mut f = Furigana::minimal().unwrap();
        f.add_reading("神", "カミ");
        let r = f.analyze("神々");
        assert_eq!(r.tokens.len(), 2);
        assert_eq!(r.tokens[0].surface, "神");
        assert_eq!(r.tokens[0].reading, "カミ");
        assert_eq!(r.tokens[1].surface, "々");
        assert_eq!(r.tokens[1].reading, "ガミ");
    }

    #[test]
    fn analyze_odoriji_falls_back_to_clone_for_non_voiceable() {
        // 我々 → ワレ + ワレ (ワ 行は連濁対象外、 そのまま複製)
        let mut f = Furigana::minimal().unwrap();
        f.add_reading("我", "ワレ");
        let r = f.analyze("我々");
        assert_eq!(r.tokens.len(), 2);
        assert_eq!(r.tokens[1].surface, "々");
        assert_eq!(r.tokens[1].reading, "ワレ");
    }

    #[test]
    fn analyze_odoriji_loses_to_jukugo_when_dict_has_explicit_entry() {
        // dict に 「神々」 = カミガミ を登録すると、 jukugo (band 1000) が
        // 「神」+「々」 (band 100 × 2) を上回り、 単一 token に
        let mut f = Furigana::minimal().unwrap();
        f.add_reading("神々", "カミガミ");
        f.add_reading("神", "カミ");
        let r = f.analyze("神々");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].surface, "神々");
        assert_eq!(r.tokens[0].reading, "カミガミ");
    }

    #[test]
    fn analyze_candidates_include_all_overlapping_entries() {
        // 同位置で jukugo + unihan が両方候補に上がる (path 採択は jukugo 勝ち)
        let mut f = Furigana::minimal().unwrap();
        f.add_reading("灰桜", "ハイザクラ");
        f.add_reading("灰", "ハイ");
        let r = f.analyze("灰桜");
        // 採択 path は 「灰桜」 1 token
        assert_eq!(r.tokens.len(), 1);
        // candidates[0] には dict 由来の 「灰桜」 + 「灰」 の両方が上がる
        let pos0_surfaces: Vec<&str> = r.candidates[0].iter().map(|c| c.surface.as_str()).collect();
        assert!(pos0_surfaces.contains(&"灰桜"));
        assert!(pos0_surfaces.contains(&"灰"));
    }

    // ─── analyze() (C3) tests: NumberCandidateProvider 統合 ────────────────────

    fn fixture_rules_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("rules")
    }

    #[test]
    fn analyze_number_counter_path_uses_band_950() {
        // fixture rules 経由で counter regex が effective、 「3本」 が NumberProvider 経由で path に乗る
        let f = Furigana::builder()
            .rules_dir(fixture_rules_dir())
            .build()
            .expect("build with rules_dir");
        let r = f.analyze("3本");
        assert_eq!(r.tokens.len(), 1, "expected single counter token: {r:?}");
        assert_eq!(r.tokens[0].surface, "3本");
        assert_eq!(r.tokens[0].reading, "サンボン");
    }

    #[test]
    fn analyze_number_si_unit_beats_alphabet_passthrough_for_mixed_surface() {
        // 「100km」: AlphabetPassthrough が pure passthrough だと band 100 (miss)、
        // NumberProvider の SI candidate (band 950) が勝つ
        let f = Furigana::builder()
            .rules_dir(fixture_rules_dir())
            .build()
            .expect("build with rules_dir");
        let r = f.analyze("100km");
        assert_eq!(r.tokens.len(), 1, "expected single SI token: {r:?}");
        assert_eq!(r.tokens[0].surface, "100km");
        assert!(
            r.tokens[0].reading.contains("ヒャク") && r.tokens[0].reading.contains("キロメートル"),
            "reading: {}",
            r.tokens[0].reading,
        );
    }

    #[test]
    fn analyze_pure_digit_uses_number_provider_not_alphabet() {
        // 「100」 のみ: AlphabetPassthrough miss は band 100、 NumberProvider digit は band 950 → 後者が勝つ
        let f = Furigana::builder()
            .rules_dir(fixture_rules_dir())
            .build()
            .expect("build with rules_dir");
        let r = f.analyze("100");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].surface, "100");
        assert_eq!(r.tokens[0].reading, "ヒャク");
    }

    #[test]
    fn analyze_dict_entry_overrides_number_provider_for_counter_surface() {
        // dict に 「3本」 = カスタム読み を入れると band 1000 で NumberProvider 950 を override
        let mut f = Furigana::builder()
            .rules_dir(fixture_rules_dir())
            .build()
            .expect("build with rules_dir");
        f.add_reading("3本", "ミホン");
        let r = f.analyze("3本");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(
            r.tokens[0].reading, "ミホン",
            "dict 1000 が special 950 に勝つ"
        );
    }

    #[test]
    fn analyze_date_full_pattern_emits_single_token() {
        let f = Furigana::builder()
            .rules_dir(fixture_rules_dir())
            .build()
            .expect("build with rules_dir");
        let r = f.analyze("2025年10月30日");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].surface, "2025年10月30日");
        assert!(
            r.tokens[0].reading.contains("ジュウガツ"),
            "reading: {}",
            r.tokens[0].reading,
        );
    }

    #[test]
    fn analyze_minimal_falls_back_to_lindera_for_counter_when_rules_empty() {
        // alpha.13 以前: minimal() = 空 RulesData → counter regex None で 「3本」 の
        // 「本」 を覆える provider 無し、 path 構築不能だった。
        // alpha.13+ : Lindera fallback が「3」「本」 を band 50 で edge 化、 path 構築成功。
        let f = Furigana::minimal().unwrap();
        let r = f.analyze("3本");
        assert!(
            !r.tokens.is_empty(),
            "Lindera fallback should provide edges: {r:?}"
        );
        let total_len: usize = r.tokens.iter().map(|t| t.range.end - t.range.start).sum();
        assert_eq!(total_len, "3本".len());
    }

    // ─── analyze() (★A2 alpha.12) DictBridge MatchCondition 評価 tests ─────

    /// Detailed entry を含む dict を temp file 経由で build する helper。
    /// Furigana の内部 dict 構築経路は file load なので、 unit test で
    /// rich field を inject するために temp TOML を書き出して dir 経由 load する。
    fn build_with_dict_toml(toml_body: &str) -> Furigana {
        let dir = std::env::temp_dir().join(format!(
            "furigana_dict_bridge_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let dict_file = dir.join("test.toml");
        std::fs::write(
            &dict_file,
            format!(
                "[meta]\nschema_version = \"2\"\nrole = \"jukugo\"\n\n{}",
                toml_body
            ),
        )
        .unwrap();
        Furigana::builder()
            .core_dict_dir(&dir)
            .build()
            .expect("build with detailed entry")
    }

    #[test]
    fn dict_bridge_evaluates_inline_match_default_reading() {
        // 「上手」 単独 (= 文脈 「から」 無し) → default 「ジョウズ」
        let f = build_with_dict_toml(
            r#"[entries."上手"]
reading = "ジョウズ"

[[entries."上手".match]]
next_eq = "から"
reading = "カミテ"
"#,
        );
        let r = f.analyze("上手");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].reading, "ジョウズ");
    }

    #[test]
    fn dict_bridge_evaluates_inline_match_with_next_eq() {
        // 「上手から」 → 文脈 「から」 match → reading 「カミテ」
        // path 全 cover のため 「から」 も dict に Simple で追加
        let f = build_with_dict_toml(
            r#"[entries]
"から" = "カラ"

[entries."上手"]
reading = "ジョウズ"

[[entries."上手".match]]
next_eq = "から"
reading = "カミテ"
"#,
        );
        let r = f.analyze("上手から");
        let kamite_token = r.tokens.iter().find(|t| t.surface == "上手");
        assert!(
            kamite_token.is_some(),
            "expected 「上手」 token in path: {r:?}"
        );
        assert_eq!(kamite_token.unwrap().reading, "カミテ");
    }

    #[test]
    fn dict_bridge_evaluates_match_with_prev_eq() {
        // 「下上手」 → 「下」 (= 単漢字 dict entry あり) + 「上手」 (= prev_eq "下" → シタテ)
        let f = build_with_dict_toml(
            r#"[entries]
"下" = "シタ"

[entries."上手"]
reading = "ジョウズ"

[[entries."上手".match]]
prev_eq = "下"
reading = "シタテ"
"#,
        );
        let r = f.analyze("下上手");
        let jouzu_token = r.tokens.iter().find(|t| t.surface == "上手");
        assert!(jouzu_token.is_some(), "expected 「上手」 token: {r:?}");
        assert_eq!(jouzu_token.unwrap().reading, "シタテ");
    }

    // ─── analyze() (★A2 alpha.12) [[kanji]] block 評価 tests ───────────────

    /// [[kanji]] block (role = "kanji") を含む dict を temp file 経由で build する helper。
    fn build_with_kanji_toml(toml_body: &str) -> Furigana {
        let dir = std::env::temp_dir().join(format!(
            "furigana_kanji_block_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let dict_file = dir.join("test.toml");
        std::fs::write(
            &dict_file,
            format!(
                "[meta]\nschema_version = \"2\"\nrole = \"kanji\"\n\n{}",
                toml_body
            ),
        )
        .unwrap();
        Furigana::builder()
            .core_dict_dir(&dir)
            .build()
            .expect("build with [[kanji]] block")
    }

    #[test]
    fn dict_bridge_evaluates_kanji_block_default_reading() {
        // 「生」 単独 → default 「セイ」 (= 文脈 「じる」 無し / prev 漢字ではない)
        let f = build_with_kanji_toml(
            r#"[[kanji]]
char = "生"
default = "セイ"

[[kanji.match]]
next_eq = "じる"
reading = "ショウ"
"#,
        );
        let r = f.analyze("生");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].reading, "セイ");
    }

    #[test]
    fn dict_bridge_evaluates_kanji_block_with_next_eq() {
        // 「生じる」 → 「生」 が next_eq "じる" match → 「ショウ」
        // path 全 cover のため 「じる」 を Simple entry で追加
        let f = build_with_kanji_toml(
            r#"[entries]
"じる" = "ジル"

[[kanji]]
char = "生"
default = "セイ"

[[kanji.match]]
next_eq = "じる"
reading = "ショウ"
"#,
        );
        let r = f.analyze("生じる");
        let sei_token = r.tokens.iter().find(|t| t.surface == "生");
        assert!(sei_token.is_some(), "expected 「生」 token in path: {r:?}");
        assert_eq!(sei_token.unwrap().reading, "ショウ");
    }
}
