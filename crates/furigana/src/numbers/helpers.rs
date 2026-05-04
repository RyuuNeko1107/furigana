//! 数値処理 module 内専用の小ヘルパ
//!
//! 公開 API ではない (`pub(crate)`)。`number_to_katakana` 等から呼ばれる。

/// 全角英数字・記号 → 半角 (本 module 内専用、[`crate::kana::zen_to_han`] の縮小版)
pub(crate) fn zen2han(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
            '．' => '.',
            '，' => ',',
            '％' => '%',
            '＋' => '+',
            '－' | '\u{2212}' | '\u{2013}' | '\u{2014}' => '-',
            '〜' | '～' => '~',
            '／' => '/',
            _ => c,
        })
        .collect()
}

/// `zen2han` した上でカンマを除去
pub(crate) fn norm_num(s: &str) -> String {
    zen2han(s).replace(',', "")
}

/// 数値文字列 → i64 (全角・カンマ対応、不正なら `None`)
pub(crate) fn to_int(s: &str) -> Option<i64> {
    norm_num(s).parse::<i64>().ok()
}

/// 文字列末尾の 1 桁を返す。数字が無ければ `0`。
pub(crate) fn last_digit(s: &str) -> u32 {
    let norm = norm_num(s);
    for ch in norm.chars().rev() {
        if ch.is_ascii_digit() {
            return ch.to_digit(10).unwrap_or(0);
        }
    }
    0
}

/// カタカナ末尾を促音化 (イチ→イッ、ロク→ロッ、ハチ→ハッ、ジュウ→ジュッ)。
/// 該当しなければそのまま返す。
pub(crate) fn sokuonize_last(num_kata: &str) -> String {
    for (src, dst) in &[
        ("イチ", "イッ"),
        ("ロク", "ロッ"),
        ("ハチ", "ハッ"),
        ("ジュウ", "ジュッ"),
    ] {
        if let Some(stripped) = num_kata.strip_suffix(src) {
            return format!("{stripped}{dst}");
        }
    }
    num_kata.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zen2han_works() {
        assert_eq!(zen2han("１２３"), "123");
        assert_eq!(zen2han("１，２３４"), "1,234");
        assert_eq!(zen2han("－５"), "-5");
    }

    #[test]
    fn norm_num_strips_commas() {
        assert_eq!(norm_num("1,234,567"), "1234567");
        assert_eq!(norm_num("１，２３４"), "1234");
    }

    #[test]
    fn to_int_handles_zenkaku() {
        assert_eq!(to_int("１２３"), Some(123));
        assert_eq!(to_int("-５"), Some(-5));
        assert_eq!(to_int("abc"), None);
    }

    #[test]
    fn last_digit_works() {
        assert_eq!(last_digit("123"), 3);
        assert_eq!(last_digit("100"), 0);
        assert_eq!(last_digit("1,234"), 4);
        assert_eq!(last_digit("abc"), 0);
    }

    #[test]
    fn sokuonize_last_works() {
        assert_eq!(sokuonize_last("イチ"), "イッ");
        assert_eq!(sokuonize_last("ロク"), "ロッ");
        assert_eq!(sokuonize_last("ハチ"), "ハッ");
        assert_eq!(sokuonize_last("ジュウ"), "ジュッ");
        assert_eq!(sokuonize_last("ニ"), "ニ");
    }
}
