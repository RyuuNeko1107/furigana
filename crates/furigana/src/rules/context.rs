//! 文脈依存読みルール (context.toml)
//!
//! 前後トークンの surface / 品詞を見て読みを切り替える。
//!
//! ## 例
//! ```toml
//! [[rule]]
//! surface = "一日"
//!
//! [[rule.match]]
//! prev_month = true
//! reading = "ツイタチ"
//!
//! [[rule.match]]
//! next_starts_any = ["中", "間", "分"]
//! reading = "イチニチ"
//!
//! [[rule]]
//! surface = "一人"
//! default = "ヒトリ"
//!
//! [[rule]]
//! surface = "大人気"
//! default = "ダイニンキ"
//!
//! [[rule.match]]
//! next_starts = "な"
//! reading = "オトナゲ"
//! ```

use serde::Deserialize;

/// context.toml 全体
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ContextData {
    /// 1 surface に対する文脈ルール (上から評価)
    #[serde(default, rename = "rule")]
    pub rules: Vec<ContextRule>,
}

impl ContextData {
    /// 別の `ContextData` をマージする (rules を末尾に追記)
    ///
    /// context/ サブディレクトリ配下の複数 TOML を 1 つの構造体に
    /// 統合する用途で [`crate::loader::load_rules_dir`] 内部から呼ばれる。
    /// rules は順序依存のため、ファイル名ソート順で末尾追記される。
    pub fn merge(&mut self, other: Self) {
        self.rules.extend(other.rules);
    }
}

/// 1 surface に対する文脈ルール
#[derive(Debug, Clone, Deserialize)]
pub struct ContextRule {
    /// 対象 surface (例: "一日")
    pub surface: String,

    /// どの match にもヒットしない場合のデフォルト読み。
    /// `None` の場合は形態素解析側に委譲する。
    #[serde(default)]
    pub default: Option<String>,

    /// 個別 match パターン (上から順に評価、最初にヒットしたもの採用)
    #[serde(default, rename = "match")]
    pub matches: Vec<ContextMatch>,
}

/// 文脈マッチ条件 + その時の読み
///
/// 全ての条件は AND で結合される (1 つでも該当しなければマッチ失敗)。
/// 条件が 1 つも指定されていない場合は無条件ヒット (`default` 相当)。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContextMatch {
    /// マッチ時の読み (カタカナ)
    pub reading: String,

    // ─── 前トークン条件 ───
    /// 前トークンの surface 完全一致
    #[serde(default)]
    pub prev_eq: Option<String>,
    /// 前トークンの surface が末尾でいずれかに一致 (旧 TOML key `prev_ends_with_any` も受ける)
    #[serde(default, alias = "prev_ends_with_any")]
    pub prev_ends: Vec<String>,
    /// 前トークンが月名 (一月〜十二月 / 1月〜12月) で終わるか (旧 key `prev_ends_with_month`)
    #[serde(default, alias = "prev_ends_with_month")]
    pub prev_month: bool,

    // ─── 次トークン条件 ───
    /// 次トークンの surface 完全一致
    #[serde(default)]
    pub next_eq: Option<String>,
    /// 次トークンの surface が先頭で指定文字列に一致 (旧 key `next_starts_with`)
    #[serde(default, alias = "next_starts_with")]
    pub next_starts: Option<String>,
    /// 次トークンの surface が先頭でいずれかに一致 (旧 key `next_starts_with_any`)
    #[serde(default, alias = "next_starts_with_any")]
    pub next_starts_any: Vec<String>,
    /// 次トークンの surface が数字 (半角/全角) で始まる (旧 key `next_starts_with_digit`)
    #[serde(default, alias = "next_starts_with_digit")]
    pub next_digit: bool,

    // ─── 次の次トークン条件 ───
    /// 「大人気の無い」のように 1 つ飛ばし参照する用途 (旧 key `next_next_starts_with_any`)
    #[serde(default, alias = "next_next_starts_with_any")]
    pub next2_starts: Vec<String>,

    // ─── 品詞条件 ───
    /// 当該トークンの品詞 (例: "名詞", "形容詞") — 旧 key `pos_eq`
    #[serde(default, alias = "pos_eq")]
    pub pos: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_default_only() {
        let toml_str = r#"
            [[rule]]
            surface = "一人"
            default = "ヒトリ"
        "#;
        let data: ContextData = toml::from_str(toml_str).unwrap();
        assert_eq!(data.rules.len(), 1);
        assert_eq!(data.rules[0].surface, "一人");
        assert_eq!(data.rules[0].default.as_deref(), Some("ヒトリ"));
        assert!(data.rules[0].matches.is_empty());
    }

    #[test]
    fn parses_rule_with_multiple_matches() {
        let toml_str = r#"
            [[rule]]
            surface = "一日"

            [[rule.match]]
            prev_month = true
            reading = "ツイタチ"

            [[rule.match]]
            next_starts_any = ["中", "間"]
            reading = "イチニチ"
        "#;
        let data: ContextData = toml::from_str(toml_str).unwrap();
        let r = &data.rules[0];
        assert_eq!(r.matches.len(), 2);
        assert!(r.matches[0].prev_month);
        assert_eq!(r.matches[0].reading, "ツイタチ");
        assert_eq!(r.matches[1].next_starts_any, vec!["中", "間"]);
        assert_eq!(r.matches[1].reading, "イチニチ");
    }

    #[test]
    fn parses_pos_condition() {
        let toml_str = r#"
            [[rule]]
            surface = "上手"

            [[rule.match]]
            pos = "名詞"
            reading = "ジョウズ"
        "#;
        let data: ContextData = toml::from_str(toml_str).unwrap();
        assert_eq!(data.rules[0].matches[0].pos.as_deref(), Some("名詞"));
        assert_eq!(data.rules[0].matches[0].reading, "ジョウズ");
    }

    #[test]
    fn parses_two_step_lookahead() {
        let toml_str = r#"
            [[rule]]
            surface = "大人気"
            default = "ダイニンキ"

            [[rule.match]]
            next_eq = "の"
            next2_starts = ["な", "無"]
            reading = "オトナゲ"
        "#;
        let data: ContextData = toml::from_str(toml_str).unwrap();
        let r = &data.rules[0];
        assert_eq!(r.matches[0].next_eq.as_deref(), Some("の"));
        assert_eq!(r.matches[0].next2_starts, vec!["な", "無"]);
    }

    /// 旧 TOML key (alias 互換) でも deserialize できることを確認
    #[test]
    fn parses_legacy_aliases() {
        let toml_str = r#"
            [[rule]]
            surface = "一日"

            [[rule.match]]
            prev_ends_with_month = true
            reading = "ツイタチ"

            [[rule.match]]
            next_starts_with_any = ["中", "間"]
            pos_eq = "名詞"
            reading = "イチニチ"
        "#;
        let data: ContextData = toml::from_str(toml_str).unwrap();
        let r = &data.rules[0];
        assert!(r.matches[0].prev_month);
        assert_eq!(r.matches[1].next_starts_any, vec!["中", "間"]);
        assert_eq!(r.matches[1].pos.as_deref(), Some("名詞"));
    }

    #[test]
    fn empty_input_yields_default() {
        let data: ContextData = toml::from_str("").unwrap();
        assert!(data.rules.is_empty());
    }
}
