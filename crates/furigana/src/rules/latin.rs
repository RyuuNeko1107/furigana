//! ラテン文字読み (latin.toml)
//!
//! 英字 1 文字 → カタカナ (A→エー, B→ビー, ...)
//!
//! ## 例
//! ```toml
//! [entries]
//! "A" = "エー"
//! "B" = "ビー"
//! ```
//!
//! lookup は **大文字小文字を区別せず** 行う (大文字キーで保存推奨)。

use serde::Deserialize;
use std::collections::HashMap;

/// latin.toml 全体 (文字 → カタカナ)
#[derive(Debug, Default, Clone, Deserialize)]
pub struct LatinData {
    #[serde(default)]
    pub entries: HashMap<String, String>,
}

impl LatinData {
    /// 1 文字を読みに変換 (case-insensitive)
    #[must_use]
    pub fn lookup(&self, ch: char) -> Option<&str> {
        let upper = ch.to_ascii_uppercase().to_string();
        self.entries
            .iter()
            .find(|(k, _)| k.to_ascii_uppercase() == upper)
            .map(|(_, v)| v.as_str())
    }

    /// 件数
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 空判定
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LatinData {
        let toml_str = r#"
            [entries]
            "A" = "エー"
            "B" = "ビー"
            "Z" = "ズィー"
        "#;
        toml::from_str(toml_str).unwrap()
    }

    #[test]
    fn upper_match() {
        let d = sample();
        assert_eq!(d.lookup('A'), Some("エー"));
        assert_eq!(d.lookup('Z'), Some("ズィー"));
    }

    #[test]
    fn lower_also_matches() {
        let d = sample();
        assert_eq!(d.lookup('a'), Some("エー"));
        assert_eq!(d.lookup('b'), Some("ビー"));
    }

    #[test]
    fn miss_returns_none() {
        let d = sample();
        assert_eq!(d.lookup('Q'), None);
        assert_eq!(d.lookup('1'), None);
    }
}
