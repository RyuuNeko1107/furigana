//! ひらがな⇄カタカナ変換、漢字判定、Unicode 正規化ユーティリティ
//!
//! データに依存しない純粋関数のみ。
//! `normalize_text` だけ [`CompatData`](crate::rules::CompatData) を引数に取り、
//! 異体字置換を行う。

use crate::rules::CompatData;
use once_cell::sync::Lazy;
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

// ─── 範囲定数 ────────────────────────────────────────────────────────────────

/// ひらがな範囲: ぁ(0x3041) 〜 ん(0x3093)
const HIRAGANA_START: u32 = 0x3041;
const HIRAGANA_END: u32 = 0x3093;

/// カタカナ範囲: ァ(0x30A1) 〜 ン(0x30F3)
const KATAKANA_START: u32 = 0x30A1;
const KATAKANA_END: u32 = 0x30F3;

/// ひら⇄カタ オフセット
const KATA_HIRA_OFFSET: u32 = 0x60;

// ─── 単文字判定 ──────────────────────────────────────────────────────────────

/// ひらがな 1 文字か (ぁ〜ん + ゔ)
#[must_use]
pub fn is_hiragana_char(c: char) -> bool {
    let cp = c as u32;
    (HIRAGANA_START..=HIRAGANA_END).contains(&cp) || c == 'ゔ'
}

/// カタカナ 1 文字か (ァ〜ン + ヴ)
#[must_use]
pub fn is_katakana_char(c: char) -> bool {
    let cp = c as u32;
    (KATAKANA_START..=KATAKANA_END).contains(&cp) || c == 'ヴ'
}

/// 漢字 1 文字か (CJK 統合漢字 + 拡張 A + 互換 + 々〆ヶ)
#[must_use]
pub fn is_kanji_char(c: char) -> bool {
    matches!(c,
        '\u{3400}'..='\u{4DBF}' |   // CJK 拡張 A
        '\u{4E00}'..='\u{9FFF}' |   // CJK 統合漢字
        '\u{F900}'..='\u{FAFF}' |   // CJK 互換
        '々' | '〆' | 'ヶ'
    )
}

// ─── 文字列単位 ──────────────────────────────────────────────────────────────

/// カタカナ→ひらがな
#[must_use]
pub fn kata_to_hira(s: &str) -> String {
    s.chars()
        .map(|c| {
            let cp = c as u32;
            if (KATAKANA_START..=KATAKANA_END).contains(&cp) {
                char::from_u32(cp - KATA_HIRA_OFFSET).unwrap_or(c)
            } else if c == 'ヴ' {
                'ゔ'
            } else {
                c
            }
        })
        .collect()
}

/// ひらがな→カタカナ
#[must_use]
pub fn hira_to_kata(s: &str) -> String {
    s.chars()
        .map(|c| {
            let cp = c as u32;
            if (HIRAGANA_START..=HIRAGANA_END).contains(&cp) {
                char::from_u32(cp + KATA_HIRA_OFFSET).unwrap_or(c)
            } else if c == 'ゔ' {
                'ヴ'
            } else {
                c
            }
        })
        .collect()
}

/// 漢字を 1 文字でも含むか
#[must_use]
pub fn has_kanji(s: &str) -> bool {
    s.chars().any(is_kanji_char)
}

/// カタカナを 1 文字でも含むか (長音 ー 含む)
#[must_use]
pub fn has_katakana(s: &str) -> bool {
    s.chars()
        .any(|c| is_katakana_char(c) || c == 'ー' || c == 'ヴ')
}

/// 純カタカナ文字列か (長音 ー / 中点 ・ も許容)
#[must_use]
pub fn is_pure_katakana(s: &str) -> bool {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[゠-ヿー・]+$").unwrap());
    !s.is_empty() && RE.is_match(s)
}

/// 純ひらがな文字列か (ゔ 含む、その他記号は不可)
#[must_use]
pub fn is_pure_hiragana(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_hiragana_char)
}

// ─── 全角→半角 ──────────────────────────────────────────────────────────────

/// 全角英数字・記号 → 半角
#[must_use]
pub fn zen_to_han(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
            'Ａ'..='Ｚ' => char::from_u32(c as u32 - 'Ａ' as u32 + 'A' as u32).unwrap_or(c),
            'ａ'..='ｚ' => char::from_u32(c as u32 - 'ａ' as u32 + 'a' as u32).unwrap_or(c),
            '－' | '\u{2212}' => '-',
            '＋' => '+',
            '～' | '〜' => '~',
            '％' => '%',
            '．' => '.',
            '，' => ',',
            '／' => '/',
            _ => c,
        })
        .collect()
}

// ─── 正規化 ──────────────────────────────────────────────────────────────────

/// テキスト正規化: NFKC → 異体字置換 → NFC
///
/// `compat_map` の variant → canonical 変換を、NFKC の後に適用する。
/// 入力が空なら空文字列を返す。
#[must_use]
pub fn normalize_text(s: &str, compat: &CompatData) -> String {
    if s.is_empty() {
        return String::new();
    }
    // NFKC で結合・互換正規化
    let nfkc: String = s.nfkc().collect();
    // 異体字置換 (1 文字単位)
    let replaced: String = nfkc
        .chars()
        .map(|c| {
            let cs = c.to_string();
            if let Some(canonical) = compat.lookup(&cs) {
                canonical.chars().next().unwrap_or(c)
            } else {
                c
            }
        })
        .collect();
    // 安全のため NFC で正規化結合
    replaced.nfc().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::CompatEntry;
    use std::collections::HashMap;

    // ─── kata_to_hira / hira_to_kata ──────────────────────────────

    #[test]
    fn kata_to_hira_basic() {
        assert_eq!(kata_to_hira("ヨム"), "よむ");
        assert_eq!(kata_to_hira("トウキョウ"), "とうきょう");
        assert_eq!(kata_to_hira("ヴァイオリン"), "ゔぁいおりん");
    }

    #[test]
    fn kata_to_hira_passthrough() {
        assert_eq!(kata_to_hira("漢字"), "漢字");
        assert_eq!(kata_to_hira("hello123"), "hello123");
        assert_eq!(kata_to_hira(""), "");
    }

    #[test]
    fn kata_to_hira_keeps_long_mark_and_punct() {
        assert_eq!(kata_to_hira("コーヒー・ラテ"), "こーひー・らて");
    }

    #[test]
    fn hira_to_kata_basic() {
        assert_eq!(hira_to_kata("よむ"), "ヨム");
        assert_eq!(hira_to_kata("とうきょう"), "トウキョウ");
        assert_eq!(hira_to_kata("ゔぁ"), "ヴァ");
    }

    #[test]
    fn round_trip_kata_hira() {
        let original = "アイウエオカキクケコ";
        assert_eq!(hira_to_kata(&kata_to_hira(original)), original);
    }

    // ─── 単文字判定 ───────────────────────────────────────────────

    #[test]
    fn is_hiragana_char_works() {
        assert!(is_hiragana_char('あ'));
        assert!(is_hiragana_char('ん'));
        assert!(is_hiragana_char('ゔ'));
        assert!(!is_hiragana_char('ア'));
        assert!(!is_hiragana_char('a'));
    }

    #[test]
    fn is_katakana_char_works() {
        assert!(is_katakana_char('ア'));
        assert!(is_katakana_char('ン'));
        assert!(is_katakana_char('ヴ'));
        assert!(!is_katakana_char('あ'));
        assert!(!is_katakana_char('a'));
    }

    #[test]
    fn is_kanji_char_works() {
        assert!(is_kanji_char('漢'));
        assert!(is_kanji_char('東'));
        assert!(is_kanji_char('々'));
        assert!(is_kanji_char('〆'));
        assert!(is_kanji_char('ヶ'));
        assert!(!is_kanji_char('あ'));
        assert!(!is_kanji_char('a'));
    }

    // ─── has_kanji / has_katakana ───────────────────────────────

    #[test]
    fn has_kanji_works() {
        assert!(has_kanji("読む"));
        assert!(has_kanji("東京タワー"));
        assert!(has_kanji("々"));
        assert!(!has_kanji("よむ"));
        assert!(!has_kanji("カタカナ"));
        assert!(!has_kanji(""));
    }

    #[test]
    fn has_katakana_works() {
        assert!(has_katakana("カタカナ"));
        assert!(has_katakana("漢字とカナ"));
        assert!(has_katakana("コーヒー"));
        assert!(!has_katakana("ひらがな"));
        assert!(!has_katakana("漢字"));
    }

    // ─── pure 判定 ────────────────────────────────────────────────

    #[test]
    fn is_pure_katakana_works() {
        assert!(is_pure_katakana("カタカナ"));
        assert!(is_pure_katakana("タワー"));
        assert!(is_pure_katakana("コーヒー・ラテ"));
        assert!(!is_pure_katakana("漢字"));
        assert!(!is_pure_katakana("ひらがな"));
        assert!(!is_pure_katakana(""));
        assert!(!is_pure_katakana("カナと漢字"));
    }

    #[test]
    fn is_pure_hiragana_works() {
        assert!(is_pure_hiragana("ひらがな"));
        assert!(is_pure_hiragana("ゔぁい"));
        assert!(!is_pure_hiragana("カタカナ"));
        assert!(!is_pure_hiragana(""));
        assert!(!is_pure_hiragana("ひらと漢字"));
    }

    // ─── zen_to_han ──────────────────────────────────────────────

    #[test]
    fn zen_to_han_digits_and_symbols() {
        assert_eq!(zen_to_han("１２３"), "123");
        assert_eq!(zen_to_han("５０％"), "50%");
        assert_eq!(zen_to_han("Ａ＋Ｂ"), "A+B");
        assert_eq!(zen_to_han("ｈｅｌｌｏ"), "hello");
        assert_eq!(zen_to_han("１．５"), "1.5");
    }

    #[test]
    fn zen_to_han_passthrough() {
        // 漢字・カナはそのまま
        assert_eq!(zen_to_han("漢字"), "漢字");
        assert_eq!(zen_to_han("カナ"), "カナ");
    }

    // ─── normalize_text ──────────────────────────────────────────

    fn make_compat(pairs: &[(&str, &str)]) -> CompatData {
        let entries: Vec<_> = pairs
            .iter()
            .map(|(v, c)| CompatEntry {
                variant: (*v).to_string(),
                canonical: (*c).to_string(),
            })
            .collect();
        let mut data = CompatData {
            entries,
            map: HashMap::new(),
        };
        data.rebuild_map();
        data
    }

    #[test]
    fn normalize_text_replaces_variants() {
        let compat = make_compat(&[("髙", "高"), ("﨑", "崎")]);
        assert_eq!(normalize_text("髙﨑", &compat), "高崎");
    }

    #[test]
    fn normalize_text_keeps_unmapped() {
        let compat = make_compat(&[]);
        assert_eq!(normalize_text("こんにちは", &compat), "こんにちは");
    }

    #[test]
    fn normalize_text_applies_nfkc() {
        let compat = make_compat(&[]);
        // 全角数字 NFKC → 半角
        assert_eq!(normalize_text("１２３", &compat), "123");
    }

    #[test]
    fn normalize_text_empty() {
        let compat = make_compat(&[]);
        assert_eq!(normalize_text("", &compat), "");
    }
}
