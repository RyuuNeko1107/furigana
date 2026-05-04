//! 記号読み (symbols.toml)
//!
//! +, −, ±, %, ‰, /, etc.
//!
//! ## 例
//! ```toml
//! [entries]
//! "+" = "プラス"
//! "-" = "マイナス"
//! "±" = "プラスマイナス"
//! "%" = "パーセント"
//! ```

use serde::Deserialize;
use std::collections::HashMap;

/// symbols.toml 全体 (記号 → カタカナ)
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SymbolsData {
    #[serde(default)]
    pub entries: HashMap<String, String>,
}

impl SymbolsData {
    /// 記号で読みを引く
    #[must_use]
    pub fn lookup(&self, symbol: &str) -> Option<&str> {
        self.entries.get(symbol).map(String::as_str)
    }

    /// 1 文字での lookup ヘルパ
    #[must_use]
    pub fn lookup_char(&self, ch: char) -> Option<&str> {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        self.lookup(s)
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

    fn sample() -> SymbolsData {
        let toml_str = r#"
            [entries]
            "+" = "プラス"
            "-" = "マイナス"
            "%" = "パーセント"
            "‰" = "パーミル"
        "#;
        toml::from_str(toml_str).unwrap()
    }

    #[test]
    fn lookup_str() {
        let d = sample();
        assert_eq!(d.lookup("+"), Some("プラス"));
        assert_eq!(d.lookup("‰"), Some("パーミル"));
        assert_eq!(d.lookup("&"), None);
    }

    #[test]
    fn lookup_char_helper() {
        let d = sample();
        assert_eq!(d.lookup_char('%'), Some("パーセント"));
        assert_eq!(d.lookup_char('‰'), Some("パーミル"));
    }
}
