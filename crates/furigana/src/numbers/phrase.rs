//! 慣用語句マッチャー (regex pre-compiled)
//!
//! 「二十歳→ハタチ」「明後日→アサッテ」のように形態素解析・助数詞ルール
//! より先に確定させる慣用語句を、長さ降順の正規表現でテキストから
//! 切り出す。

use crate::rules::NumericPhrasesData;
use aho_corasick::AhoCorasick;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

/// `apply` でテキストを (表層, Option<読み>) のチャンク列に分割する。
/// マッチした表層は読み確定 (`Some`)、間の文字列は読み未確定 (`None`)。
///
/// `jukugo_ac` / `jukugo_map` を [`Self::set_jukugo`] で注入すると、
/// phrase が match した範囲を真に含む jukugo entry がある場合に jukugo を
/// 優先採用する (例: numeric_phrases の「千本=センボン」 が match しても、
/// jukugo に「千本桜=センボンザクラ」 があれば後者を採用)。
#[derive(Debug, Clone)]
pub struct NumericPhraseMatcher {
    regex: Option<Regex>,
    table: HashMap<String, String>,
    jukugo_ac: Option<Arc<AhoCorasick>>,
    jukugo_map: Option<Arc<HashMap<String, String>>>,
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
            // size_limit で巨大 regex (compile 後の DFA / NFA size) を拒否。
            // 攻撃者制御の data (numeric_phrases.toml に大量 entry) で memory を食わせる
            // regex bomb 防御。 10 MB は通常使用 (~100 entries) を十分カバー。
            ::regex::RegexBuilder::new(&pattern)
                .size_limit(10 * 1024 * 1024)
                .build()
                .ok()
        };

        Self {
            regex,
            table,
            jukugo_ac: None,
            jukugo_map: None,
        }
    }

    /// 空マッチャー (テスト・default 用)
    #[must_use]
    pub fn empty() -> Self {
        Self {
            regex: None,
            table: HashMap::new(),
            jukugo_ac: None,
            jukugo_map: None,
        }
    }

    /// jukugo の Aho-Corasick automaton を注入する (起動時 1 回、 chunker と Arc 共有想定)
    ///
    /// phrase match を真に含む jukugo entry がある場合に jukugo を優先するため。
    /// homonyms (context rule を持つ surface) は予め exclude された AC を渡す前提
    /// (`Furigana::build()` 側で集約する)。
    pub fn set_jukugo(&mut self, ac: Arc<AhoCorasick>, map: Arc<HashMap<String, String>>) {
        self.jukugo_ac = Some(ac);
        self.jukugo_map = Some(map);
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
            // jukugo super-set check: phrase match を真に含む jukugo entry が
            // あれば jukugo を採用 (例: phrase「千本」 を jukugo「千本桜」 が override)
            let (surf_str, reading_opt, end_pos) = {
                let phrase_surf = m.as_str();
                let phrase_reading = self.table.get(phrase_surf).cloned();
                let mut chosen_surf = phrase_surf.to_string();
                let mut chosen_reading = phrase_reading;
                let mut chosen_end = m.end();
                if let (Some(ac), Some(map)) = (&self.jukugo_ac, &self.jukugo_map) {
                    let rest = &text[m.start()..];
                    if let Some(j_mat) = ac.find(rest) {
                        let phrase_len = m.end() - m.start();
                        if j_mat.start() == 0 && j_mat.end() > phrase_len {
                            let j_surface = &rest[..j_mat.end()];
                            if let Some(j_reading) = map.get(j_surface) {
                                chosen_surf = j_surface.to_string();
                                chosen_reading = Some(j_reading.clone());
                                chosen_end = m.start() + j_mat.end();
                            }
                        }
                    }
                }
                (chosen_surf, chosen_reading, chosen_end)
            };

            if m.start() > last_end {
                parts.push((text[last_end..m.start()].to_string(), None));
            }
            parts.push((surf_str, reading_opt));
            last_end = end_pos;
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

    /// jukugo super-set check: phrase「千本」 を jukugo「千本桜」 で override
    #[test]
    fn jukugo_super_overrides_phrase() {
        // テスト用の最小 phrases (千本 = センボン)
        let phrases_toml = r#"
[entries]
"千本" = "センボン"
"#;
        let p: NumericPhrasesData = crate::loader::parse_toml(phrases_toml, "test").unwrap();
        let mut m = NumericPhraseMatcher::new(&p);
        let map: HashMap<String, String> = [("千本桜".to_string(), "センボンザクラ".to_string())]
            .into_iter()
            .collect();
        let ac = AhoCorasick::builder()
            .match_kind(aho_corasick::MatchKind::LeftmostLongest)
            .build(map.keys())
            .unwrap();
        m.set_jukugo(Arc::new(ac), Arc::new(map));
        let result = m.apply("千本桜のテーマ");
        // 「千本桜」 全体が 1 chunk として読み確定される
        assert!(
            result
                .iter()
                .any(|(s, r)| s == "千本桜" && r.as_deref() == Some("センボンザクラ")),
            "expected 千本桜 chunk in {result:?}"
        );
    }

    /// jukugo super-set check が無ければ既存挙動 (phrase「千本」 を確定): 副作用ゼロ
    #[test]
    fn no_jukugo_keeps_phrase_behavior() {
        let phrases_toml = r#"
[entries]
"千本" = "センボン"
"#;
        let p: NumericPhrasesData = crate::loader::parse_toml(phrases_toml, "test").unwrap();
        let m = NumericPhraseMatcher::new(&p); // jukugo 未注入
        let result = m.apply("千本桜のテーマ");
        assert!(
            result
                .iter()
                .any(|(s, r)| s == "千本" && r.as_deref() == Some("センボン")),
            "expected 千本 chunk in {result:?}"
        );
    }
}
