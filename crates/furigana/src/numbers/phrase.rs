//! 慣用語句マッチャー (regex pre-compiled)
//!
//! 「二十歳→ハタチ」「明後日→アサッテ」のように形態素解析・助数詞ルール
//! より先に確定させる慣用語句を、長さ降順の正規表現でテキストから
//! 切り出す。

use crate::rules::NumericPhrasesData;
use regex::Regex;
use std::collections::HashMap;

/// `apply` でテキストを (表層, Option<読み>) のチャンク列に分割する。
/// マッチした表層は読み確定 (`Some`)、間の文字列は読み未確定 (`None`)。
#[derive(Debug, Clone)]
pub struct NumericPhraseMatcher {
    regex: Option<Regex>,
    table: HashMap<String, String>,
}

impl NumericPhraseMatcher {
    /// `phrases` から正規表現を pre-compile する
    #[must_use]
    pub fn new(phrases: &NumericPhrasesData) -> Self {
        let table: HashMap<String, String> = phrases.entries.clone();

        let regex = if phrases.entries.is_empty() {
            None
        } else {
            // 長い表層を優先するため文字数降順ソート (例: "一人前" を "一人" より先にマッチ)
            let mut surfaces: Vec<&str> = phrases.surfaces().collect();
            surfaces.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
            let alts: Vec<String> = surfaces.iter().map(|s| regex::escape(s)).collect();
            let pattern = format!("(?:{})", alts.join("|"));
            Regex::new(&pattern).ok()
        };

        Self { regex, table }
    }

    /// 空マッチャー (テスト・default 用)
    #[must_use]
    pub fn empty() -> Self {
        Self {
            regex: None,
            table: HashMap::new(),
        }
    }

    /// テキストを慣用句で分割し、(表層, Option<読み>) 列を返す
    #[must_use]
    pub fn apply(&self, text: &str) -> Vec<(String, Option<String>)> {
        let Some(regex) = &self.regex else {
            return vec![(text.to_string(), None)];
        };

        let mut parts: Vec<(String, Option<String>)> = Vec::new();
        let mut last_end = 0;

        for m in regex.find_iter(text) {
            if m.start() > last_end {
                parts.push((text[last_end..m.start()].to_string(), None));
            }
            let surf = m.as_str();
            let reading = self.table.get(surf).cloned();
            parts.push((surf.to_string(), reading));
            last_end = m.end();
        }

        if last_end < text.len() {
            parts.push((text[last_end..].to_string(), None));
        }

        if parts.is_empty() {
            vec![(text.to_string(), None)]
        } else {
            parts
        }
    }
}

/// 互換 API: phrases を引数に取って 1 回限り適用するヘルパ
#[must_use]
pub fn apply_numeric_overrides(
    text: &str,
    phrases: &NumericPhrasesData,
) -> Vec<(String, Option<String>)> {
    NumericPhraseMatcher::new(phrases).apply(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::parse_toml;

    fn load_phrases() -> NumericPhrasesData {
        let raw = include_str!("../../tests/fixtures/rules/numeric_phrases.toml");
        parse_toml(raw, "numeric_phrases.toml").unwrap()
    }

    #[test]
    fn match_hatachi() {
        let p = load_phrases();
        let m = NumericPhraseMatcher::new(&p);
        let result = m.apply("二十歳になった");
        assert!(result
            .iter()
            .any(|(s, r)| s == "二十歳" && r.as_deref() == Some("ハタチ")));
    }

    #[test]
    fn match_multiple() {
        let p = load_phrases();
        let m = NumericPhraseMatcher::new(&p);
        let result = m.apply("明後日と一昨日");
        assert!(result
            .iter()
            .any(|(s, r)| s == "明後日" && r.as_deref() == Some("アサッテ")));
        assert!(result
            .iter()
            .any(|(s, r)| s == "一昨日" && r.as_deref() == Some("オトトイ")));
    }

    #[test]
    fn no_match_passthrough() {
        let p = load_phrases();
        let m = NumericPhraseMatcher::new(&p);
        let result = m.apply("こんにちは");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ("こんにちは".to_string(), None));
    }

    #[test]
    fn empty_matcher() {
        let m = NumericPhraseMatcher::empty();
        let result = m.apply("test");
        assert_eq!(result, vec![("test".to_string(), None)]);
    }

    #[test]
    fn longer_match_wins() {
        let p = load_phrases();
        let m = NumericPhraseMatcher::new(&p);
        // "一人前" が "一人" より先に確定する
        let result = m.apply("一人前");
        assert!(result
            .iter()
            .any(|(s, r)| s == "一人前" && r.as_deref() == Some("イチニンマエ")));
    }
}
