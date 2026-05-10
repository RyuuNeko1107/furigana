//! Matcher 評価 logic — `MatchCondition` が input context にマッチするかを判定。
//!
//! 詳細仕様: `docs/PROPOSALS/scoring-engine.md` §3.4
//!
//! ## semantics
//!
//! - 同 `MatchBlock` 内の condition は **AND** (全 hit で match 成立)
//! - 複数 `MatchBlock` は TOML 順で **第一 hit** 採用 (caller 側で iterate)
//! - condition が 1 つも指定されていない (= 全 None / 空 array) 場合は無条件 match
//!
//! ## char_type 判定
//!
//! `prev_char_type` / `next_char_type` は 「直前 token の最後の文字」 / 「直後 token の最初の文字」
//! を [`classify_char`] で分類して比較する。 token 不在 (= 文頭 / 文末) や
//! 分類不能文字の場合は no match。

use crate::kana;
use crate::scoring::format::{CharType, MatchCondition};

/// matcher 評価時の周辺 context。
///
/// caller は現在の token 位置で前後 token を参照可能な構造を渡す。
/// 文頭は `prev_token = None`、 文末は `next_token = None`。
#[derive(Debug, Clone, Copy, Default)]
pub struct MatchContext<'a> {
    /// 直前 token surface (文頭は None)
    pub prev_token: Option<&'a str>,
    /// 直後 token surface (文末は None)
    pub next_token: Option<&'a str>,
}

impl<'a> MatchContext<'a> {
    /// 全条件 None の空 context (= 文単独 token)
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// `prev_token` だけ指定
    #[must_use]
    pub fn with_prev(prev: &'a str) -> Self {
        Self {
            prev_token: Some(prev),
            next_token: None,
        }
    }

    /// `next_token` だけ指定
    #[must_use]
    pub fn with_next(next: &'a str) -> Self {
        Self {
            prev_token: None,
            next_token: Some(next),
        }
    }

    /// 前後両方指定
    #[must_use]
    pub fn with_both(prev: &'a str, next: &'a str) -> Self {
        Self {
            prev_token: Some(prev),
            next_token: Some(next),
        }
    }
}

impl MatchCondition {
    /// この condition が context にマッチするか判定 (AND semantics)。
    ///
    /// 全 condition が空 (= 全 None / 空 array) の場合は無条件 `true`。
    /// 1 つでも condition が指定されていて、 それが context に hit しない場合は `false`。
    #[must_use]
    pub fn matches_context(&self, ctx: &MatchContext<'_>) -> bool {
        // ─── prev_eq ────────────────────────────────────────────────────────
        if let Some(expected) = &self.prev_eq {
            match ctx.prev_token {
                Some(actual) if actual == expected => {}
                _ => return false,
            }
        }

        // ─── prev_eq_any ────────────────────────────────────────────────────
        if !self.prev_eq_any.is_empty() {
            let prev = match ctx.prev_token {
                Some(p) => p,
                None => return false, // prev 無いのに list 指定 → no match
            };
            if !self.prev_eq_any.iter().any(|s| s == prev) {
                return false;
            }
        }

        // ─── next_eq ────────────────────────────────────────────────────────
        if let Some(expected) = &self.next_eq {
            match ctx.next_token {
                Some(actual) if actual == expected => {}
                _ => return false,
            }
        }

        // ─── next_eq_any ────────────────────────────────────────────────────
        if !self.next_eq_any.is_empty() {
            let next = match ctx.next_token {
                Some(n) => n,
                None => return false,
            };
            if !self.next_eq_any.iter().any(|s| s == next) {
                return false;
            }
        }

        // ─── prev_char_type ─────────────────────────────────────────────────
        if let Some(expected_type) = self.prev_char_type {
            let last_char = ctx.prev_token.and_then(|s| s.chars().next_back());
            match last_char {
                Some(c) if classify_char(c) == Some(expected_type) => {}
                _ => return false,
            }
        }

        // ─── next_char_type ─────────────────────────────────────────────────
        if let Some(expected_type) = self.next_char_type {
            let first_char = ctx.next_token.and_then(|s| s.chars().next());
            match first_char {
                Some(c) if classify_char(c) == Some(expected_type) => {}
                _ => return false,
            }
        }

        true
    }
}

/// 文字を [`CharType`] に分類。
///
/// 既存 [`crate::kana`] の helper を再利用 + 英数 / 記号判定を追加。
/// 分類不能 (= 制御文字 / 空白 等) は `None`。
///
/// ## 分類順序 (mutually exclusive)
///
/// 1. 漢字 (CJK Unified Ideographs 等)
/// 2. ひらがな
/// 3. カタカナ (全角・半角)
/// 4. 英数 (ASCII alphanumeric / 全角英数)
/// 5. 記号 (上記以外の punctuation 等)
#[must_use]
pub fn classify_char(c: char) -> Option<CharType> {
    if kana::is_kanji_char(c) {
        Some(CharType::Kanji)
    } else if kana::is_hiragana_char(c) {
        Some(CharType::Hiragana)
    } else if kana::is_katakana_char(c) || is_extended_katakana_char(c) {
        Some(CharType::Katakana)
    } else if is_alphanumeric_char(c) {
        Some(CharType::Alphanumeric)
    } else if is_symbol_char(c) {
        Some(CharType::Symbol)
    } else {
        None
    }
}

/// カタカナ拡張判定 (kana::is_katakana_char に含まれない長音 / 半角カナ等)。
///
/// scoring 用途では実用的なカタカナ判定が要るので、 既存 strict 定義より広め:
/// - 長音記号 ー (U+30FC)
/// - 半角カタカナ (U+FF65〜U+FF9F)
/// - カタカナ拡張 (U+31F0〜U+31FF)
fn is_extended_katakana_char(c: char) -> bool {
    matches!(c,
        '\u{30FC}'                  // 長音記号 ー
        | '\u{FF65}'..='\u{FF9F}'   // 半角カタカナ
        | '\u{31F0}'..='\u{31FF}'   // カタカナ拡張
    )
}

/// 英数判定 (ASCII alphanumeric + 全角英数)。
fn is_alphanumeric_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c,
            '\u{FF10}'..='\u{FF19}'   // 全角数字 0-9
            | '\u{FF21}'..='\u{FF3A}' // 全角大文字 A-Z
            | '\u{FF41}'..='\u{FF5A}' // 全角小文字 a-z
        )
}

/// 記号判定 (kanji / kana / 英数 でなく、 punctuation 系の文字)。
///
/// 制御文字 / 空白は除外 (= None 扱い)、 句読点 / 括弧 / その他記号のみ Symbol 扱い。
fn is_symbol_char(c: char) -> bool {
    if c.is_control() || c.is_whitespace() {
        return false;
    }
    // 既知の punctuation / symbol range をざっくり include
    matches!(c,
        // ASCII punctuation
        '\u{0021}'..='\u{002F}'
        | '\u{003A}'..='\u{0040}'
        | '\u{005B}'..='\u{0060}'
        | '\u{007B}'..='\u{007E}'
        // 日本語句読点 / 括弧
        | '\u{3000}'..='\u{303F}'
        // 全角記号 (`！` 〜 `／` の前半部、 数字英字以外)
        | '\u{FF01}'..='\u{FF0F}'
        | '\u{FF1A}'..='\u{FF20}'
        | '\u{FF3B}'..='\u{FF40}'
        | '\u{FF5B}'..='\u{FF65}'
        // 一般 punctuation (U+2030..U+205E は U+2000..U+206F に含まれるので重複削除済)
        | '\u{2000}'..='\u{206F}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cond_default() -> MatchCondition {
        MatchCondition::default()
    }

    // ─── 無条件 match ────────────────────────────────────────────────────────

    #[test]
    fn empty_condition_matches_any_context() {
        let cond = cond_default();
        assert!(cond.matches_context(&MatchContext::empty()));
        assert!(cond.matches_context(&MatchContext::with_prev("前")));
        assert!(cond.matches_context(&MatchContext::with_next("後")));
        assert!(cond.matches_context(&MatchContext::with_both("前", "後")));
    }

    // ─── prev_eq ─────────────────────────────────────────────────────────────

    #[test]
    fn prev_eq_matches_when_equal() {
        let cond = MatchCondition {
            prev_eq: Some("階段".into()),
            ..Default::default()
        };
        assert!(cond.matches_context(&MatchContext::with_prev("階段")));
        assert!(!cond.matches_context(&MatchContext::with_prev("梯子")));
        assert!(!cond.matches_context(&MatchContext::empty()));
    }

    // ─── prev_eq_any ─────────────────────────────────────────────────────────

    #[test]
    fn prev_eq_any_matches_when_in_list() {
        let cond = MatchCondition {
            prev_eq_any: vec!["階段".into(), "段".into(), "梯子".into()],
            ..Default::default()
        };
        assert!(cond.matches_context(&MatchContext::with_prev("階段")));
        assert!(cond.matches_context(&MatchContext::with_prev("段")));
        assert!(cond.matches_context(&MatchContext::with_prev("梯子")));
        assert!(!cond.matches_context(&MatchContext::with_prev("丘")));
        assert!(!cond.matches_context(&MatchContext::empty()));
    }

    // ─── next_eq ─────────────────────────────────────────────────────────────

    #[test]
    fn next_eq_matches_when_equal() {
        let cond = MatchCondition {
            next_eq: Some("から".into()),
            ..Default::default()
        };
        assert!(cond.matches_context(&MatchContext::with_next("から")));
        assert!(!cond.matches_context(&MatchContext::with_next("まで")));
        assert!(!cond.matches_context(&MatchContext::empty()));
    }

    // ─── next_eq_any ─────────────────────────────────────────────────────────

    #[test]
    fn next_eq_any_matches_when_in_list() {
        let cond = MatchCondition {
            next_eq_any: vec!["まれ".into(), "まれる".into()],
            ..Default::default()
        };
        assert!(cond.matches_context(&MatchContext::with_next("まれ")));
        assert!(cond.matches_context(&MatchContext::with_next("まれる")));
        assert!(!cond.matches_context(&MatchContext::with_next("じる")));
    }

    // ─── prev_char_type ──────────────────────────────────────────────────────

    #[test]
    fn prev_char_type_matches_kanji() {
        let cond = MatchCondition {
            prev_char_type: Some(CharType::Kanji),
            ..Default::default()
        };
        assert!(cond.matches_context(&MatchContext::with_prev("校"))); // 「高校」 の最後
        assert!(cond.matches_context(&MatchContext::with_prev("高校"))); // 末尾文字 = 「校」
        assert!(!cond.matches_context(&MatchContext::with_prev("きの"))); // ひらがな末尾
    }

    #[test]
    fn next_char_type_matches_hiragana() {
        let cond = MatchCondition {
            next_char_type: Some(CharType::Hiragana),
            ..Default::default()
        };
        assert!(cond.matches_context(&MatchContext::with_next("じる"))); // 先頭 = 「じ」
        assert!(cond.matches_context(&MatchContext::with_next("から"))); // 先頭 = 「か」
        assert!(!cond.matches_context(&MatchContext::with_next("漢字"))); // 先頭 = 「漢」
        assert!(!cond.matches_context(&MatchContext::empty()));
    }

    // ─── AND 結合 ────────────────────────────────────────────────────────────

    #[test]
    fn multiple_conditions_combined_with_and() {
        let cond = MatchCondition {
            prev_eq: Some("生".into()),
            next_eq: Some("じる".into()),
            ..Default::default()
        };
        assert!(cond.matches_context(&MatchContext::with_both("生", "じる")));
        assert!(!cond.matches_context(&MatchContext::with_both("生", "まれる"))); // next_eq miss
        assert!(!cond.matches_context(&MatchContext::with_both("死", "じる"))); // prev_eq miss
    }

    #[test]
    fn prev_char_type_and_next_eq_combined() {
        let cond = MatchCondition {
            prev_char_type: Some(CharType::Hiragana),
            next_eq: Some("クリーム".into()),
            ..Default::default()
        };
        assert!(cond.matches_context(&MatchContext::with_both("きの", "クリーム")));
        assert!(!cond.matches_context(&MatchContext::with_both("漢字", "クリーム")));
        assert!(!cond.matches_context(&MatchContext::with_both("きの", "ジュース")));
    }

    // ─── classify_char ───────────────────────────────────────────────────────

    #[test]
    fn classify_char_kanji() {
        assert_eq!(classify_char('生'), Some(CharType::Kanji));
        assert_eq!(classify_char('漢'), Some(CharType::Kanji));
        assert_eq!(classify_char('魔'), Some(CharType::Kanji));
    }

    #[test]
    fn classify_char_hiragana() {
        assert_eq!(classify_char('あ'), Some(CharType::Hiragana));
        assert_eq!(classify_char('ん'), Some(CharType::Hiragana));
        assert_eq!(classify_char('ゃ'), Some(CharType::Hiragana));
    }

    #[test]
    fn classify_char_katakana() {
        assert_eq!(classify_char('ア'), Some(CharType::Katakana));
        assert_eq!(classify_char('ン'), Some(CharType::Katakana));
        assert_eq!(classify_char('ー'), Some(CharType::Katakana)); // 長音
    }

    #[test]
    fn classify_char_alphanumeric() {
        assert_eq!(classify_char('A'), Some(CharType::Alphanumeric));
        assert_eq!(classify_char('z'), Some(CharType::Alphanumeric));
        assert_eq!(classify_char('5'), Some(CharType::Alphanumeric));
        assert_eq!(classify_char('Ａ'), Some(CharType::Alphanumeric)); // 全角
        assert_eq!(classify_char('１'), Some(CharType::Alphanumeric)); // 全角数字
    }

    #[test]
    fn classify_char_symbol() {
        assert_eq!(classify_char('!'), Some(CharType::Symbol));
        assert_eq!(classify_char('、'), Some(CharType::Symbol));
        assert_eq!(classify_char('。'), Some(CharType::Symbol));
        assert_eq!(classify_char('「'), Some(CharType::Symbol));
        assert_eq!(classify_char('】'), Some(CharType::Symbol));
    }

    #[test]
    fn classify_char_unknown_returns_none() {
        // 制御文字 / 空白 / 未割当 は None
        assert_eq!(classify_char(' '), None);
        assert_eq!(classify_char('\t'), None);
        assert_eq!(classify_char('\n'), None);
    }

    // ─── multi-byte token char_type 判定 ─────────────────────────────────────

    #[test]
    fn prev_char_type_uses_last_char_of_multi_char_token() {
        let cond = MatchCondition {
            prev_char_type: Some(CharType::Kanji),
            ..Default::default()
        };
        // 「きの生」 の末尾は 「生」 = 漢字 → match
        assert!(cond.matches_context(&MatchContext::with_prev("きの生")));
        // 「漢字きの」 の末尾は 「の」 = ひらがな → no match
        assert!(!cond.matches_context(&MatchContext::with_prev("漢字きの")));
    }

    #[test]
    fn next_char_type_uses_first_char_of_multi_char_token() {
        let cond = MatchCondition {
            next_char_type: Some(CharType::Kanji),
            ..Default::default()
        };
        // 「生まれ」 の先頭は 「生」 = 漢字 → match
        assert!(cond.matches_context(&MatchContext::with_next("生まれ")));
        // 「まれ生」 の先頭は 「ま」 = ひらがな → no match
        assert!(!cond.matches_context(&MatchContext::with_next("まれ生")));
    }
}
