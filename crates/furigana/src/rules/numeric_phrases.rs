//! 数詞慣用語句 (numeric_phrases.toml)
//!
//! 数字を含む慣用語句 (二十歳→ハタチ、明後日→アサッテ等) を、形態素解析や
//! 助数詞ルールより先に確定させるための表。
//!
//! ## 例
//! ```toml
//! [entries]
//! "二十歳" = "ハタチ"
//! "二十日" = "ハツカ"
//! "明後日" = "アサッテ"
//! ```

use serde::Deserialize;
use std::collections::HashMap;

/// numeric_phrases.toml 全体 (表層 → カタカナ)
#[derive(Debug, Default, Clone, Deserialize)]
pub struct NumericPhrasesData {
    #[serde(default)]
    pub entries: HashMap<String, String>,
}

impl NumericPhrasesData {
    /// 表層に対応する読みを返す
    #[must_use]
    pub fn lookup(&self, surface: &str) -> Option<&str> {
        self.entries.get(surface).map(String::as_str)
    }

    /// 全表層を一覧 (regex builder 用、HashMap なので順不同)
    pub fn surfaces(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
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

    fn sample() -> NumericPhrasesData {
        let toml_str = r#"
            [entries]
            "二十歳" = "ハタチ"
            "明後日" = "アサッテ"
            "三日月" = "ミカヅキ"
        "#;
        toml::from_str(toml_str).unwrap()
    }

    #[test]
    fn lookup_works() {
        let d = sample();
        assert_eq!(d.lookup("二十歳"), Some("ハタチ"));
        assert_eq!(d.lookup("明後日"), Some("アサッテ"));
        assert_eq!(d.lookup("夕日"), None);
    }

    #[test]
    fn surfaces_iter_returns_all_keys() {
        let d = sample();
        let mut surfaces: Vec<&str> = d.surfaces().collect();
        surfaces.sort_unstable();
        assert_eq!(surfaces, vec!["三日月", "二十歳", "明後日"]);
    }
}
