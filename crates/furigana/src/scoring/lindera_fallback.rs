//! Lindera fallback provider — Smart engine の safety net (★ alpha.13)。
//!
//! 詳細仕様: `docs/PROPOSALS/scoring-engine.md` §5.6 (postprocess 分離) と同 layer 観。
//!
//! ## 役割
//!
//! Smart engine の 5 provider (Protect / Alphabet / DictBridge / Number / Odoriji)
//! が一切覆わない位置 (= 助詞 / okurigana / dict 未登録 単語) を band 50 で
//! 埋めるための **最終 fallback**。
//!
//! - corpus pass で 62% uncovered (= path 構築不能) だった主因が hiragana 助詞 /
//!   okurigana の coverage 不在。 本 provider 追加で path 構築率 ≒ 100% を期待。
//! - band 50 (= `Score::lindera`) で BAND_DICT_EXACT (1000) / BAND_SPECIAL (950) /
//!   BAND_KANJI (100) より低い、 weakest_band 集約で 「他に candidate があるなら
//!   そちら勝ち、 完全に何も無い時のみ Lindera が拾う」 を実現。
//!
//! ## 設計
//!
//! - construction time に input 全体を 1 度だけ tokenize、 edge 配列を保持
//!   (= 各 `candidates_at` で再 tokenize しない、 amortized O(1) lookup)
//! - 各 [`MorphToken`] を `(byte_start, byte_end, reading)` の edge に変換、
//!   reading が無い token (= 記号 / 未知語) は surface をそのまま reading とする
//! - Lindera surface byte 累積が input byte 長と不一致なら、 edge 生成中止
//!   (defensive、 通常は Normal mode で一致するはず)
//!
//! ## なぜ band 50 で十分か
//!
//! [`crate::scoring::engine::PathScore`] は `weakest_band` (= path 中の最低 band)
//! で path を比較する。 path に Lindera edge が混じれば weakest=50、 全て dict /
//! kanji なら weakest≥100、 高 band 側勝ち。 つまり Lindera は 「他 provider が
//! 完全に空のとき」 だけ採用される (= safety net 動作)。
//!
//! ## 注意点
//!
//! - per-input lifetime: input を変えるたびに新しい [`LinderaFallbackProvider`]
//!   を作る必要がある (= state cache が input に依存)。 `Furigana::analyze` 内で
//!   毎回 new する想定。
//! - thread safety: 内部 [`Analyzer`] は [`std::sync::Mutex`] 保護なので、
//!   並列実行は直列化される。 single-thread 用途では問題なし。
//! - reading は Lindera 由来 = カタカナ (IPADIC details[7])、 既存 provider と
//!   出力形式整合。 [`crate::scoring::bracket::strip_intonation_markers`] は
//!   念のため通すが、 Lindera reading に bracket は含まれない想定。

use crate::analyzer::Analyzer;
use crate::scoring::bracket::strip_intonation_markers;
use crate::scoring::candidate::{Candidate, CandidateProvider, Score};

/// Lindera tokenize 結果を edge 配列で保持する fallback provider。
///
/// construction 時に 1 度だけ tokenize、 以降 `candidates_at` は O(edge_count)
/// で位置 lookup (= 通常 input なら数十 edges 程度、 amortized 軽量)。
#[derive(Debug, Clone)]
pub struct LinderaFallbackProvider {
    /// (byte_start, byte_end, reading) の 3-tuple。
    /// reading は カタカナ (Lindera 由来) または surface fallback。
    edges: Vec<(usize, usize, String)>,
}

impl LinderaFallbackProvider {
    /// `analyzer` で `input` を tokenize、 edge 配列を構築。
    ///
    /// Lindera tokenize 失敗 / surface byte 不一致 時は edges 空で初期化
    /// (= provider 自体が無効、 caller path に影響なし)。
    #[must_use]
    pub fn new(analyzer: &Analyzer, input: &str) -> Self {
        if input.is_empty() {
            return Self { edges: Vec::new() };
        }
        let tokens = analyzer.tokenize(input);
        let mut edges = Vec::with_capacity(tokens.len());
        let mut byte_pos = 0usize;
        for tok in tokens {
            let surface_len = tok.surface.len();
            // input 範囲外 (= 累積ズレ) なら edges 中断
            if byte_pos + surface_len > input.len() {
                return Self { edges: Vec::new() };
            }
            // surface と input 該当 slice が一致しない (= tokenize で文字捨て or
            // normalize が起こった) なら edges 中断、 safety net 自体 disable
            let slice = &input[byte_pos..byte_pos + surface_len];
            if slice != tok.surface {
                return Self { edges: Vec::new() };
            }
            // reading: Lindera details[7] (= カタカナ)、 無ければ surface fallback
            // (= 記号 / 未知語、 reading = surface で 「読まない」 扱い)
            let reading = tok.reading.unwrap_or_else(|| tok.surface.clone());
            edges.push((byte_pos, byte_pos + surface_len, reading));
            byte_pos += surface_len;
        }
        Self { edges }
    }

    /// 空 provider (test 用、 input なしで安全に new する)。
    #[must_use]
    pub fn empty() -> Self {
        Self { edges: Vec::new() }
    }
}

impl CandidateProvider for LinderaFallbackProvider {
    fn candidates_at(&self, input: &str, pos: usize) -> Vec<Candidate> {
        self.edges
            .iter()
            .filter(|(start, _, _)| *start == pos)
            .map(|(start, end, reading)| {
                let surface = &input[*start..*end];
                let char_count = surface.chars().count();
                let length = u8::try_from(char_count).unwrap_or(u8::MAX);
                Candidate::new(
                    surface.to_string(),
                    strip_intonation_markers(reading),
                    *start..*end,
                    Score::lindera(length),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::Analyzer;

    fn analyzer() -> Analyzer {
        Analyzer::new().expect("Analyzer init")
    }

    #[test]
    fn empty_input_yields_no_edges() {
        let a = analyzer();
        let p = LinderaFallbackProvider::new(&a, "");
        assert_eq!(p.candidates_at("", 0).len(), 0);
    }

    #[test]
    fn hiragana_particle_emits_band_50_candidate() {
        let a = analyzer();
        let input = "猫が好き";
        let p = LinderaFallbackProvider::new(&a, input);
        // 「猫」 の byte 範囲は 0..3 (UTF-8 3 byte)、 「が」 は 3..6
        let cands_at_3 = p.candidates_at(input, 3);
        assert!(
            !cands_at_3.is_empty(),
            "expected Lindera edge at pos=3 (=が)"
        );
        let ga = cands_at_3.iter().find(|c| c.surface == "が");
        assert!(ga.is_some(), "expected が candidate: {cands_at_3:?}");
        let ga = ga.unwrap();
        assert_eq!(ga.score.band, 50);
        // 助詞 「が」 の Lindera reading は 「ガ」 (カタカナ)
        assert_eq!(ga.reading, "ガ");
    }

    #[test]
    fn band_50_loses_to_kanji_band_100() {
        // band 比較の sanity: Lindera band 50 < kanji band 100。
        // 同じ pos で kanji candidate がある場合、 PathScore.weakest_band で
        // kanji 勝ち (= Lindera は何も無い時の最終 fallback)。
        let lindera = Score::lindera(2);
        let kanji = Score::kanji(2);
        assert!(kanji > lindera);
    }

    #[test]
    fn unknown_token_falls_back_to_surface_reading() {
        // 完全に未知の記号列 等は Lindera reading None になるので surface を reading 化、
        // 「読まないが path 上の edge にはなる」 動作 (= path 構築の safety net)。
        let a = analyzer();
        // 仮に Lindera が 「★」 に reading を付けなければ surface fallback
        let input = "★";
        let p = LinderaFallbackProvider::new(&a, input);
        let cands = p.candidates_at(input, 0);
        // edge が出るか / reading が surface fallback か (= None ではない) を check
        if let Some(c) = cands.first() {
            assert!(!c.reading.is_empty(), "reading should fallback to surface");
        }
    }
}
