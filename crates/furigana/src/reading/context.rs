//! 文脈依存読みのルールエンジン
//!
//! [`ContextData`] (data-driven) を引数に取り、surface に対応する
//! [`ContextRule`] の `match` を順次評価して読みを決定する。

use crate::analyzer::MorphToken;
use crate::rules::{ContextData, ContextMatch};

/// 当該 token (idx) の surface に対応する [`crate::rules::ContextRule`] を引き、
/// match 条件を順に評価して読みを決定する。
///
/// - どの match にもヒット → その reading
/// - default のみあり → それ
/// - rule がそもそも無い OR どれにもヒットせず default も無い → `None`
#[must_use]
pub fn apply_context_rules(
    context: &ContextData,
    tokens: &[MorphToken],
    idx: usize,
) -> Option<String> {
    let token = tokens.get(idx)?;
    let surface = token.surface.as_str();

    let rule = context.rules.iter().find(|r| r.surface == surface)?;

    for m in &rule.matches {
        if context_match_eval(m, tokens, idx) {
            return Some(m.reading.clone());
        }
    }

    rule.default.clone()
}

fn context_match_eval(m: &ContextMatch, tokens: &[MorphToken], idx: usize) -> bool {
    let token = &tokens[idx];
    let prev = idx.checked_sub(1).and_then(|i| tokens.get(i));
    let next = tokens.get(idx + 1);
    let next_next = tokens.get(idx + 2);

    // ─── 前トークン条件 ────────────────────────────────────────────────────
    if let Some(eq) = &m.prev_eq {
        if prev.map(|t| t.surface.as_str()) != Some(eq.as_str()) {
            return false;
        }
    }
    if !m.prev_ends.is_empty() {
        let ok = prev.is_some_and(|t| m.prev_ends.iter().any(|s| t.surface.ends_with(s.as_str())));
        if !ok {
            return false;
        }
    }
    if m.prev_month {
        let ok = prev.is_some_and(|t| ends_with_month(&t.surface));
        if !ok {
            return false;
        }
    }

    // ─── 次トークン条件 ────────────────────────────────────────────────────
    if let Some(eq) = &m.next_eq {
        if next.map(|t| t.surface.as_str()) != Some(eq.as_str()) {
            return false;
        }
    }
    if let Some(prefix) = &m.next_starts {
        let ok = next.is_some_and(|t| t.surface.starts_with(prefix.as_str()));
        if !ok {
            return false;
        }
    }
    if !m.next_starts_any.is_empty() {
        let ok = next.is_some_and(|t| {
            m.next_starts_any
                .iter()
                .any(|s| t.surface.starts_with(s.as_str()))
        });
        if !ok {
            return false;
        }
    }
    if m.next_digit {
        let ok = next.is_some_and(|t| starts_with_digit(&t.surface));
        if !ok {
            return false;
        }
    }

    // ─── 次の次トークン条件 ────────────────────────────────────────────────
    if !m.next2_starts.is_empty() {
        let ok = next_next.is_some_and(|t| {
            m.next2_starts
                .iter()
                .any(|s| t.surface.starts_with(s.as_str()))
        });
        if !ok {
            return false;
        }
    }

    // ─── 品詞条件 ──────────────────────────────────────────────────────────
    if let Some(eq) = &m.pos {
        if token.pos.as_deref() != Some(eq.as_str()) {
            return false;
        }
    }

    true
}

/// 月名 (一月〜十二月、1月〜12月、全角含む) で終わるか判定
fn ends_with_month(s: &str) -> bool {
    const MONTHS: &[&str] = &[
        "一月",
        "二月",
        "三月",
        "四月",
        "五月",
        "六月",
        "七月",
        "八月",
        "九月",
        "十月",
        "十一月",
        "十二月",
        "1月",
        "2月",
        "3月",
        "4月",
        "5月",
        "6月",
        "7月",
        "8月",
        "9月",
        "10月",
        "11月",
        "12月",
        "１月",
        "２月",
        "３月",
        "４月",
        "５月",
        "６月",
        "７月",
        "８月",
        "９月",
    ];
    MONTHS.iter().any(|m| s.ends_with(m))
}

/// 半角・全角の数字で始まるか
fn starts_with_digit(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || ('０'..='９').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load_rules_dir;
    use crate::rules::RulesData;
    use std::path::PathBuf;

    fn rules() -> RulesData {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("rules");
        load_rules_dir(&dir).expect("load rules failed")
    }

    fn morph(surface: &str, pos: Option<&str>) -> MorphToken {
        MorphToken {
            surface: surface.to_string(),
            reading: None,
            pos: pos.map(ToString::to_string),
            pos_detail: None,
            conjugation_type: None,
            conjugation_form: None,
            base_form: None,
        }
    }

    #[test]
    fn hitori() {
        let rules = rules();
        let tokens = vec![morph("一人", Some("名詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 0),
            Some("ヒトリ".to_string())
        );
    }

    #[test]
    fn tsuitachi_after_month() {
        let rules = rules();
        let tokens = vec![morph("一月", Some("名詞")), morph("一日", Some("名詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 1),
            Some("ツイタチ".to_string())
        );
    }

    #[test]
    fn ichinichi_with_duration_suffix() {
        let rules = rules();
        let tokens = vec![morph("一日", Some("名詞")), morph("中", Some("名詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 0),
            Some("イチニチ".to_string())
        );
    }

    #[test]
    fn ichinichi_with_duration_prefix() {
        let rules = rules();
        let tokens = vec![morph("丸", Some("名詞")), morph("一日", Some("名詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 1),
            Some("イチニチ".to_string())
        );
    }

    #[test]
    fn jouzu_only_for_noun() {
        let rules = rules();
        let nominal = vec![morph("上手", Some("名詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &nominal, 0),
            Some("ジョウズ".to_string())
        );
        // 品詞条件にヒットしない → default なし → None
        let other = vec![morph("上手", Some("動詞"))];
        assert_eq!(apply_context_rules(&rules.context, &other, 0), None);
    }

    #[test]
    fn otonage_with_na() {
        let rules = rules();
        let tokens = vec![morph("大人気", Some("名詞")), morph("ない", Some("形容詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 0),
            Some("オトナゲ".to_string())
        );
    }

    #[test]
    fn otonage_with_no_nai() {
        let rules = rules();
        // 「大人気」「の」「ない」 → オトナゲ
        let tokens = vec![
            morph("大人気", Some("名詞")),
            morph("の", Some("助詞")),
            morph("ない", Some("形容詞")),
        ];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 0),
            Some("オトナゲ".to_string())
        );
    }

    #[test]
    fn dainkinki_default() {
        let rules = rules();
        // 「大人気」「の」「映画」 → デフォルト ダイニンキ
        let tokens = vec![
            morph("大人気", Some("名詞")),
            morph("の", Some("助詞")),
            morph("映画", Some("名詞")),
        ];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 0),
            Some("ダイニンキ".to_string())
        );
    }

    #[test]
    fn no_match_returns_none() {
        let rules = rules();
        let tokens = vec![morph("無関係な単語", None)];
        assert_eq!(apply_context_rules(&rules.context, &tokens, 0), None);
    }
}
