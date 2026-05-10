//! Intonation bracket notation の forward compat (0.1.0 から dict 受け入れ、 lib は strip / 無視)。
//!
//! 詳細仕様: `docs/PROPOSALS/scoring-engine.md` §3.7 / `docs/PROPOSALS/intonation.md` §0
//!
//! ## 0.1.0 stable lib 側挙動
//!
//! reading 内の bracket `[` / `]` / phrase 区切り `/` を **strip** し、 reading 部分のみ使用。
//! accent 情報は捨てる (= 0.1.0 では activate しない)、 0.2.0 で parse して `accent_phrases` field に保持。
//!
//! ## 例
//!
//! - `"ジョウズ"` → `"ジョウズ"` (no markers)
//! - `"ジョ]ウズ"` → `"ジョウズ"` (1型 accent marker stripped)
//! - `"ハ[クレイ/レ[イム"` → `"ハクレイレイム"` (multi-phrase markers stripped)

/// reading 文字列から intonation bracket marker (`[`, `]`, `/`) を除去。
///
/// 0.1.0 lib が dict から読み込んだ reading に対して、 出力前に必ず通す前処理。
/// 0.2.0 で intonation 機能 投入時、 この関数は 「強制 strip」 ではなく
/// 「accent 情報抽出 + reading 取得」 に置き換わる予定。
///
/// # 例
///
/// ```
/// use furigana::scoring::bracket::strip_intonation_markers;
///
/// assert_eq!(strip_intonation_markers("ジョウズ"), "ジョウズ");
/// assert_eq!(strip_intonation_markers("ジョ]ウズ"), "ジョウズ");
/// assert_eq!(strip_intonation_markers("ハ[クレイ/レ[イム"), "ハクレイレイム");
/// ```
#[must_use]
pub fn strip_intonation_markers(reading: &str) -> String {
    reading
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | '/'))
        .collect()
}

/// reading に intonation bracket marker (`[`, `]`, `/`) が含まれているか判定。
///
/// debug / 解析用途、 dict ロード時の reading 内 marker 検出に。
///
/// # 例
///
/// ```
/// use furigana::scoring::bracket::has_intonation_markers;
///
/// assert!(!has_intonation_markers("ジョウズ"));
/// assert!(has_intonation_markers("ジョ]ウズ"));
/// assert!(has_intonation_markers("ハ[クレイ/レ[イム"));
/// ```
#[must_use]
pub fn has_intonation_markers(reading: &str) -> bool {
    reading.chars().any(|c| matches!(c, '[' | ']' | '/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── strip_intonation_markers ────────────────────────────────────────────

    #[test]
    fn strip_no_markers_returns_unchanged() {
        assert_eq!(strip_intonation_markers("ジョウズ"), "ジョウズ");
        assert_eq!(strip_intonation_markers("マリサ"), "マリサ");
    }

    #[test]
    fn strip_single_close_bracket_for_initial_high() {
        assert_eq!(strip_intonation_markers("ア]メ"), "アメ"); // 1型 (頭高)
        assert_eq!(strip_intonation_markers("ジョ]ウズ"), "ジョウズ");
    }

    #[test]
    fn strip_single_open_bracket_for_flat() {
        assert_eq!(strip_intonation_markers("キ[リサメ"), "キリサメ"); // 0型 (平板)
        assert_eq!(strip_intonation_markers("ア[メ"), "アメ");
    }

    #[test]
    fn strip_open_and_close_for_middle_high() {
        assert_eq!(strip_intonation_markers("サ[ク]ラ"), "サクラ"); // 中高
        assert_eq!(strip_intonation_markers("コ[コロ]"), "ココロ"); // 尾高
    }

    #[test]
    fn strip_phrase_separator() {
        assert_eq!(
            strip_intonation_markers("ハ[クレイ/レ[イム"),
            "ハクレイレイム"
        ); // 2 phrase
    }

    #[test]
    fn strip_multiple_phrases() {
        assert_eq!(
            strip_intonation_markers("イ[イズナマル/メ[グム"),
            "イイズナマルメグム"
        );
    }

    #[test]
    fn strip_empty_string() {
        assert_eq!(strip_intonation_markers(""), "");
    }

    #[test]
    fn strip_only_markers_yields_empty() {
        assert_eq!(strip_intonation_markers("[]/"), "");
        assert_eq!(strip_intonation_markers("[/]/"), "");
    }

    #[test]
    fn strip_preserves_non_marker_punctuation() {
        // bracket と関係ない記号 (・ / 全角) は strip しない
        assert_eq!(
            strip_intonation_markers("オン・ザ・ロック"),
            "オン・ザ・ロック"
        );
        assert_eq!(strip_intonation_markers("ハ／シ"), "ハ／シ"); // 全角スラッシュ ／ は U+FF0F、 ASCII / と別
    }

    // ─── has_intonation_markers ──────────────────────────────────────────────

    #[test]
    fn has_markers_false_for_clean_reading() {
        assert!(!has_intonation_markers("ジョウズ"));
        assert!(!has_intonation_markers(""));
    }

    #[test]
    fn has_markers_true_for_any_marker() {
        assert!(has_intonation_markers("ア]メ"));
        assert!(has_intonation_markers("キ[リサメ"));
        assert!(has_intonation_markers("ハ[クレイ/レ[イム"));
        assert!(has_intonation_markers("/"));
        assert!(has_intonation_markers("["));
        assert!(has_intonation_markers("]"));
    }

    // ─── round-trip property ──────────────────────────────────────────────────

    #[test]
    fn strip_then_has_markers_yields_false() {
        let inputs = [
            "ジョウズ",
            "ジョ]ウズ",
            "ハ[クレイ/レ[イム",
            "サ[ク]ラ",
            "",
            "[/]",
        ];
        for input in inputs {
            let stripped = strip_intonation_markers(input);
            assert!(
                !has_intonation_markers(&stripped),
                "strip 後に marker 残ってる: input={input:?}, stripped={stripped:?}"
            );
        }
    }
}
