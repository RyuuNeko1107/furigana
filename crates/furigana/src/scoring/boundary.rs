//! 漢字連続 region の検出 + boundary penalty 計算 ((b)(c))。
//!
//! 詳細仕様: `docs/PROPOSALS/scoring-engine.md` §5.2 / §5.3
//!
//! ## 役割
//!
//! 長文未知語が短い完全一致 entry に切り刻まれる問題への mitigation:
//!
//! - **(b) 漢字連続 boundary penalty**: 漢字 N 文字連続 region 内部を割る edge に base penalty −300
//! - **(c) 未知語 chunk 強化 penalty**: N >= 3 かつ region 内に完全一致 surface 皆無 → penalty −600 に強化
//! - **(a) longest match**: [`crate::scoring::engine::PathScore`] の `edge_count` 軸で実現済み
//!
//! ## scope 外
//!
//! 漢字以外の文字種 (カタカナ / ひらがな / 英数 / 記号) 連続は本 module の対象外
//! (= dict 登録すれば band 1000 で動く、 自動 chunk preservation はしない方針)。

use crate::kana;
use std::ops::Range;

/// 入力 1 つの漢字連続 region (= contiguous 漢字 char sequence) と、 そこに適用すべき penalty。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanjiRegion {
    /// input text 上の byte range
    pub range: Range<usize>,
    /// region 内の文字数 (= 漢字 char 数)
    pub char_count: usize,
    /// 内部分割 edge に加える penalty (= boundary_penalty)。 0 = penalty なし。
    pub interior_penalty: i16,
}

/// 入力全体の boundary 分析結果。
///
/// [`Self::analyze`] で input を walk して全 漢字連続 region を検出、
/// 各 region に dict の完全一致状況に応じた penalty を割り当てる。
#[derive(Debug, Clone, Default)]
pub struct BoundaryAnalysis {
    /// 検出された 漢字連続 region (順序保証、 byte range 昇順)
    pub regions: Vec<KanjiRegion>,
}

impl BoundaryAnalysis {
    /// 空の分析結果 (= regions なし、 全位置 penalty 0)。
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// `input` を walk して 漢字連続 region を検出、 各 region に penalty を割り当てる。
    ///
    /// `region_has_exact_match` callback で 「この region surface が dict 完全一致 entry に存在するか」 を問い合わせる。
    /// callback `true` を返す region は (b) base penalty −300、 `false` を返す region のうち
    /// 漢字 3 文字以上のものは (c) 強化 penalty −600、 残り (= 漢字 2 文字未満で完全一致なし) は base −300。
    ///
    /// ## penalty 値割り当てルール (proposal §5.2 / §5.3)
    ///
    /// | 条件 | penalty |
    /// |---|---|
    /// | region に完全一致 entry あり | -300 (b base) |
    /// | region に完全一致なし + 漢字 3 文字以上 | -600 (c 強化) |
    /// | region に完全一致なし + 漢字 1〜2 文字 | -300 (b base) |
    pub fn analyze<F>(input: &str, region_has_exact_match: F) -> Self
    where
        F: Fn(&str) -> bool,
    {
        let raw_regions = find_kanji_regions(input);
        let mut regions = Vec::new();
        for r in raw_regions {
            let surface = &input[r.clone()];
            let char_count = surface.chars().count();
            let has_exact = region_has_exact_match(surface);
            let penalty = if !has_exact && char_count >= 3 {
                -600 // (c) 強化
            } else {
                -300 // (b) base
            };
            regions.push(KanjiRegion {
                range: r,
                char_count,
                interior_penalty: penalty,
            });
        }
        Self { regions }
    }

    /// edge の start 位置 `pos` が 漢字連続 region の内部 (= start 以外の位置) なら penalty を返す。
    ///
    /// region 境界 (= region.start) や region 外なら 0。
    /// 複数 region に跨る場合は最初に hit した region の penalty を採用 (region は重ならない設計)。
    #[must_use]
    pub fn penalty_at(&self, pos: usize) -> i16 {
        for region in &self.regions {
            // 「内部」 = pos > region.start && pos < region.end
            if pos > region.range.start && pos < region.range.end {
                return region.interior_penalty;
            }
        }
        0
    }

    /// `pos` を含む region の reference (なければ None)。 debug / 解析用途。
    #[must_use]
    pub fn region_containing(&self, pos: usize) -> Option<&KanjiRegion> {
        self.regions
            .iter()
            .find(|r| pos >= r.range.start && pos < r.range.end)
    }
}

/// 入力中の 「漢字 1 文字以上連続する byte range」 を全列挙して返す。
///
/// 漢字判定は [`crate::kana::is_kanji_char`] (= CJK 統合漢字 + 拡張 A + 互換 + 々/〆/ヶ)。
/// 漢字 1 文字だけの region も含む (proposal §5.2 では漢字連続を抽出、 1 文字も region になる)。
#[must_use]
pub fn find_kanji_regions(input: &str) -> Vec<Range<usize>> {
    let mut regions = Vec::new();
    let mut current_start: Option<usize> = None;
    let mut current_end: usize = 0;

    for (idx, c) in input.char_indices() {
        let char_end = idx + c.len_utf8();
        if kana::is_kanji_char(c) {
            if current_start.is_none() {
                current_start = Some(idx);
            }
            current_end = char_end;
        } else if let Some(start) = current_start.take() {
            regions.push(start..current_end);
        }
    }
    if let Some(start) = current_start {
        regions.push(start..current_end);
    }
    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── find_kanji_regions ──────────────────────────────────────────────────

    #[test]
    fn find_regions_empty_input() {
        assert!(find_kanji_regions("").is_empty());
    }

    #[test]
    fn find_regions_no_kanji() {
        assert!(find_kanji_regions("ひらがな").is_empty());
        assert!(find_kanji_regions("カタカナ").is_empty());
        assert!(find_kanji_regions("ABC123").is_empty());
        assert!(find_kanji_regions("").is_empty());
    }

    #[test]
    fn find_regions_single_kanji() {
        let regions = find_kanji_regions("猫");
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], 0..3); // "猫" UTF-8 = 3 bytes
    }

    #[test]
    fn find_regions_consecutive_kanji() {
        let regions = find_kanji_regions("魔理沙");
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], 0..9); // 3 kanji × 3 bytes
    }

    #[test]
    fn find_regions_kanji_then_hiragana() {
        // "魔理沙が好き": 魔理沙 (kanji 3) + が (hira 1) + 好 (kanji 1) + き (hira 1)
        let regions = find_kanji_regions("魔理沙が好き");
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0], 0..9); // 魔理沙
        assert_eq!(regions[1], 12..15); // 好 (= 9 + 3 hira "が" → start 12)
    }

    #[test]
    fn find_regions_includes_odoriji() {
        // 「々」 は kanji_char に含まれる
        let regions = find_kanji_regions("人々");
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], 0..6);
    }

    // ─── BoundaryAnalysis::analyze ───────────────────────────────────────────

    #[test]
    fn analyze_empty_input_yields_no_regions() {
        let analysis = BoundaryAnalysis::analyze("", |_| false);
        assert!(analysis.regions.is_empty());
    }

    #[test]
    fn analyze_single_region_no_exact_match_short_uses_base_penalty() {
        // 漢字 2 文字 + 完全一致なし → -300 (b base)
        let analysis = BoundaryAnalysis::analyze("漢字", |_| false);
        assert_eq!(analysis.regions.len(), 1);
        assert_eq!(analysis.regions[0].char_count, 2);
        assert_eq!(analysis.regions[0].interior_penalty, -300);
    }

    #[test]
    fn analyze_single_region_no_exact_match_long_uses_enhanced_penalty() {
        // 漢字 3 文字 + 完全一致なし → -600 (c 強化)
        let analysis = BoundaryAnalysis::analyze("紅魔館", |_| false);
        assert_eq!(analysis.regions.len(), 1);
        assert_eq!(analysis.regions[0].char_count, 3);
        assert_eq!(analysis.regions[0].interior_penalty, -600);
    }

    #[test]
    fn analyze_single_region_with_exact_match_uses_base_penalty() {
        // 漢字 3 文字 + 完全一致あり → -300 (b base、 (c) 適用外)
        let analysis = BoundaryAnalysis::analyze("紅魔館", |s| s == "紅魔館");
        assert_eq!(analysis.regions.len(), 1);
        assert_eq!(analysis.regions[0].interior_penalty, -300);
    }

    #[test]
    fn analyze_two_separate_regions() {
        // "魔理沙が好き" → region 0 = 魔理沙 (3 chars)、 region 1 = 好 (1 char)
        let analysis = BoundaryAnalysis::analyze("魔理沙が好き", |_| false);
        assert_eq!(analysis.regions.len(), 2);
        assert_eq!(analysis.regions[0].char_count, 3);
        assert_eq!(analysis.regions[0].interior_penalty, -600); // 3+ + 完全一致なし
        assert_eq!(analysis.regions[1].char_count, 1);
        assert_eq!(analysis.regions[1].interior_penalty, -300); // 1 char、 c 適用外
    }

    // ─── penalty_at ──────────────────────────────────────────────────────────

    #[test]
    fn penalty_at_zero_outside_regions() {
        let analysis = BoundaryAnalysis::analyze("魔理沙", |_| false);
        // pos 9 (= region.end) は外
        assert_eq!(analysis.penalty_at(9), 0);
    }

    #[test]
    fn penalty_at_zero_at_region_start() {
        // region.start (= pos 0) は境界、 内部ではない
        let analysis = BoundaryAnalysis::analyze("魔理沙", |_| false);
        assert_eq!(analysis.penalty_at(0), 0);
    }

    #[test]
    fn penalty_at_returns_value_for_interior_position() {
        // "魔理沙" region [0..9]、 内部 pos = 3 (= 「理」 の start) または 6 (= 「沙」 の start)
        let analysis = BoundaryAnalysis::analyze("紅魔館", |_| false);
        assert_eq!(analysis.penalty_at(3), -600); // 内部、 強化 penalty
        assert_eq!(analysis.penalty_at(6), -600);
    }

    #[test]
    fn penalty_at_uses_base_when_exact_match_exists() {
        let analysis = BoundaryAnalysis::analyze("紅魔館", |s| s == "紅魔館");
        // 完全一致あり → base penalty -300
        assert_eq!(analysis.penalty_at(3), -300);
        assert_eq!(analysis.penalty_at(6), -300);
    }

    #[test]
    fn penalty_at_handles_multiple_regions() {
        // "魔理沙が好" → region 0 = [0..9] 漢字 3 / region 1 = [12..15] 漢字 1
        let analysis = BoundaryAnalysis::analyze("魔理沙が好", |_| false);
        assert_eq!(analysis.penalty_at(0), 0); // region 0 boundary
        assert_eq!(analysis.penalty_at(3), -600); // region 0 interior
        assert_eq!(analysis.penalty_at(9), 0); // region 0 end
        assert_eq!(analysis.penalty_at(12), 0); // region 1 boundary
        assert_eq!(analysis.penalty_at(15), 0); // region 1 end + outside
    }

    // ─── region_containing ───────────────────────────────────────────────────

    #[test]
    fn region_containing_returns_correct_region() {
        let analysis = BoundaryAnalysis::analyze("紅魔館", |_| false);
        let r = analysis.region_containing(3).unwrap();
        assert_eq!(r.range, 0..9);
    }

    #[test]
    fn region_containing_returns_none_outside() {
        let analysis = BoundaryAnalysis::analyze("紅魔館", |_| false);
        assert!(analysis.region_containing(9).is_none());
    }

    // ─── 漢字以外は対象外 ────────────────────────────────────────────────────

    #[test]
    fn analyze_ignores_non_kanji_runs() {
        // カタカナ連続は scope 外、 region に入らない
        let analysis = BoundaryAnalysis::analyze("ボイスボックス", |_| false);
        assert!(analysis.regions.is_empty());
    }

    #[test]
    fn analyze_ignores_hiragana_runs() {
        let analysis = BoundaryAnalysis::analyze("こんにちは", |_| false);
        assert!(analysis.regions.is_empty());
    }

    #[test]
    fn analyze_ignores_alphanumeric_runs() {
        let analysis = BoundaryAnalysis::analyze("API123", |_| false);
        assert!(analysis.regions.is_empty());
    }
}
