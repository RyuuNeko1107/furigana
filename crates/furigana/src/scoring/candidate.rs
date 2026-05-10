//! Candidate / Score 型と band 定数 (Smart engine 共通基盤)。
//!
//! 詳細仕様: `docs/PROPOSALS/scoring-engine.md` §4 score 設計
//!
//! ## 概要
//!
//! - [`Score`]: candidate edge の score tuple、 lexicographic 比較 (band → length → match_hits → boundary_penalty)
//! - [`Candidate`]: input text 上の 1 つの候補 edge (surface + reading + range + score)
//! - [`CandidateProvider`]: candidate を供給する trait (entry / kanji / Lindera / 特殊処理 各 layer 実装)
//! - [`Engine`]: engine 選択 enum (Strict / Smart)
//! - band 定数: 1000 = 単語辞書完全一致、 950 = 特殊処理、 100 = 漢字辞書、 50 = Lindera unihan injection

use serde::Serialize;
use std::cmp::Ordering;
use std::ops::Range;

// ─── band 値定数 ─────────────────────────────────────────────────────────────

/// 単語辞書 (`[entries]`) 完全一致の band 値。
pub const BAND_DICT_EXACT: u16 = 1000;

/// 特殊処理 (数字+助数詞 / 漢数字 / 数字読み / 踊り字 等の動的合成) の band 値。
///
/// dict 完全一致 (1000) には常に負け、 漢字辞書 (100) / Lindera (50) には常勝。
/// dict に意図的 entry を書けば 1000 で override 可能。
pub const BAND_SPECIAL: u16 = 950;

/// 漢字辞書 (`[[kanji]]`) の band 値。 dict 完全一致 / 特殊処理 がない時の前段 fallback。
pub const BAND_KANJI: u16 = 100;

/// Lindera unihan injection の band 値。 dict / 漢字辞書 にない時の最終 fallback。
pub const BAND_LINDERA_UNIHAN: u16 = 50;

/// 保護トークン (URL / Email / 絵文字 等) の band 値。 全 candidate を上回って必ず勝つ。
///
/// これらは reading 振り対象外、 surface = output で透過する。 path 選択時に必ず採用される
/// よう、 band 1000 (単語辞書完全一致) より高い値を設定。
pub const BAND_PROTECTED: u16 = 2000;

// ─── Score ───────────────────────────────────────────────────────────────────

/// candidate edge の score tuple。
///
/// **連続値ではない、 discrete tuple の lexicographic 比較** で順位決定:
///
/// 1. `band` 大 (band 値で勝負、 1000 vs 950 等)
/// 2. `length` 大 (longest match、 surface 長で勝負)
/// 3. `match_hits` 多 (inline match condition 評価で hit した数)
/// 4. `boundary_penalty` 大 (= less negative、 ペナルティが軽い path が勝つ)
///
/// 同点 (= 全 4 軸同値) の場合の tie-break は caller 側 (例: TOML 出現順) で。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Score {
    /// band 値 (1000 / 950 / 100 / 50 等)
    pub band: u16,
    /// surface 文字数 (longest match 効果の表現)
    pub length: u8,
    /// inline match condition hit 数 (default ≠ match block)
    pub match_hits: u8,
    /// (b)(c) 漢字連続 boundary penalty 累積。 negative = penalty あり。
    pub boundary_penalty: i16,
}

impl Score {
    /// 明示値で構築。
    #[must_use]
    pub const fn new(band: u16, length: u8, match_hits: u8, boundary_penalty: i16) -> Self {
        Self {
            band,
            length,
            match_hits,
            boundary_penalty,
        }
    }

    /// 単語辞書完全一致の score (band 1000、 length 指定)。
    #[must_use]
    pub const fn dict_exact(length: u8) -> Self {
        Self::new(BAND_DICT_EXACT, length, 0, 0)
    }

    /// 特殊処理 score (band 950、 length 指定)。
    #[must_use]
    pub const fn special(length: u8) -> Self {
        Self::new(BAND_SPECIAL, length, 0, 0)
    }

    /// 漢字辞書 score (band 100、 length 指定)。 length は通常 1。
    #[must_use]
    pub const fn kanji(length: u8) -> Self {
        Self::new(BAND_KANJI, length, 0, 0)
    }

    /// Lindera unihan injection score (band 50、 length 指定)。
    #[must_use]
    pub const fn lindera(length: u8) -> Self {
        Self::new(BAND_LINDERA_UNIHAN, length, 0, 0)
    }
}

impl Ord for Score {
    fn cmp(&self, other: &Self) -> Ordering {
        // lexicographic: 各軸で同点なら次の軸で勝負
        self.band
            .cmp(&other.band)
            .then(self.length.cmp(&other.length))
            .then(self.match_hits.cmp(&other.match_hits))
            // boundary_penalty: i16、 negative = ペナルティ済。
            // less negative (= 数値として大きい) が better、 通常 i16 Ord と一致。
            .then(self.boundary_penalty.cmp(&other.boundary_penalty))
    }
}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ─── Candidate ───────────────────────────────────────────────────────────────

/// Viterbi DP 上の 1 つの candidate edge。
///
/// `range` は input text 上の byte range。 `range.end - range.start` が surface byte 長。
#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    /// surface 文字列 (= input[range])
    pub surface: String,
    /// reading 文字列 (カタカナ等)
    pub reading: String,
    /// input text 上の byte range
    pub range: Range<usize>,
    /// score tuple
    pub score: Score,
}

impl Candidate {
    /// surface の byte 長 (= `range.end - range.start`)。
    #[must_use]
    pub fn surface_byte_len(&self) -> usize {
        self.range.end.saturating_sub(self.range.start)
    }

    /// 構築 helper (test / 実装中のみ便利)。
    #[must_use]
    pub fn new(
        surface: impl Into<String>,
        reading: impl Into<String>,
        range: Range<usize>,
        score: Score,
    ) -> Self {
        Self {
            surface: surface.into(),
            reading: reading.into(),
            range,
            score,
        }
    }
}

// ─── CandidateProvider trait ─────────────────────────────────────────────────

/// 各 layer (entry / kanji / Lindera / 特殊処理) が実装する candidate 供給 trait。
///
/// `candidates_at` は **byte 位置 `pos` から始まる** 候補を返す。 caller (Smart engine)
/// は input 上の各位置で全 provider を呼び出して候補を集約、 Viterbi-like DP で path を解く。
pub trait CandidateProvider {
    /// `input` の byte 位置 `pos` から始まる candidate を全列挙して返す。
    ///
    /// 候補が無い (= この位置から始まる surface が dict / kanji / etc に存在しない) 場合は空 Vec。
    /// 候補が複数あっても OK (= 同位置から異なる surface 長の candidate を返してよい)。
    fn candidates_at(&self, input: &str, pos: usize) -> Vec<Candidate>;
}

// ─── Engine 選択 ─────────────────────────────────────────────────────────────

/// Engine 種別。 0.1.0 alpha 期間中は Strict が default、 0.1.0-rc1 で Smart に default 切替。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Engine {
    /// 既存 priority chain (`reading::pipeline::resolve_reading`)。
    /// alpha.10: Strict が default、 Smart は env var or builder で opt-in。
    #[default]
    Strict,
    /// 新 candidate scoring engine (Viterbi-like path 選択)。 alpha.10 で experimental flag。
    Smart,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Score lexicographic 比較 ────────────────────────────────────────────

    #[test]
    fn score_higher_band_wins() {
        let dict = Score::dict_exact(2);
        let kanji = Score::kanji(5);
        assert!(
            dict > kanji,
            "band 1000 > band 100 even with shorter length"
        );
    }

    #[test]
    fn score_band_special_beats_kanji() {
        let special = Score::special(2);
        let kanji = Score::kanji(5);
        assert!(special > kanji, "band 950 > band 100");
    }

    #[test]
    fn score_band_special_loses_to_dict() {
        let special = Score::special(5);
        let dict = Score::dict_exact(2);
        assert!(
            dict > special,
            "band 1000 > band 950 (dict exact wins over special)"
        );
    }

    #[test]
    fn score_same_band_longer_wins() {
        let long = Score::dict_exact(4);
        let short = Score::dict_exact(2);
        assert!(long > short, "longest match within same band");
    }

    #[test]
    fn score_same_band_length_more_match_hits_wins() {
        let with_hits = Score::new(BAND_DICT_EXACT, 2, 1, 0);
        let without = Score::new(BAND_DICT_EXACT, 2, 0, 0);
        assert!(with_hits > without, "match_hits tie-break");
    }

    #[test]
    fn score_lighter_penalty_wins() {
        let no_penalty = Score::new(BAND_DICT_EXACT, 2, 0, 0);
        let with_penalty = Score::new(BAND_DICT_EXACT, 2, 0, -300);
        assert!(no_penalty > with_penalty, "no penalty > -300 penalty");
    }

    #[test]
    fn score_lesser_penalty_wins() {
        let small_penalty = Score::new(BAND_DICT_EXACT, 2, 0, -100);
        let large_penalty = Score::new(BAND_DICT_EXACT, 2, 0, -600);
        assert!(small_penalty > large_penalty, "-100 > -600");
    }

    #[test]
    fn score_equal() {
        let a = Score::dict_exact(3);
        let b = Score::dict_exact(3);
        assert_eq!(a.cmp(&b), Ordering::Equal);
    }

    // ─── band 値 sanity check ────────────────────────────────────────────────

    #[test]
    fn band_values_are_correctly_ordered() {
        assert!(BAND_DICT_EXACT > BAND_SPECIAL);
        assert!(BAND_SPECIAL > BAND_KANJI);
        assert!(BAND_KANJI > BAND_LINDERA_UNIHAN);
        assert_eq!(BAND_DICT_EXACT, 1000);
        assert_eq!(BAND_SPECIAL, 950);
        assert_eq!(BAND_KANJI, 100);
        assert_eq!(BAND_LINDERA_UNIHAN, 50);
    }

    // ─── Candidate ───────────────────────────────────────────────────────────

    #[test]
    fn candidate_surface_byte_len() {
        let c = Candidate::new("猫", "ネコ", 0..3, Score::dict_exact(1));
        assert_eq!(c.surface_byte_len(), 3); // "猫" in UTF-8 = 3 bytes
    }

    // ─── Engine default ──────────────────────────────────────────────────────

    #[test]
    fn engine_default_is_strict() {
        // alpha.10 期間中は Strict が default。 0.1.0-rc1 で Smart に切替予定。
        assert_eq!(Engine::default(), Engine::Strict);
    }
}
