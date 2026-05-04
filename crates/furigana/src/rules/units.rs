//! SI 単位読み (units.toml)
//!
//! km / cm / mm / m / kg / mg / g / t / mL / L / TB / GB / MB / KB …
//!
//! ## 例
//! ```toml
//! [entries]
//! "km" = { kana = "キロメートル" }
//! "L"  = { kana = "リットル", ci = true }
//! "mL" = { kana = "ミリリットル", ci = true }
//! ```
//!
//! `ci = true` で case-insensitive lookup (大文字小文字を区別しない)。

use serde::Deserialize;
use std::collections::HashMap;

/// units.toml 1 件 (HashMap の value 側)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UnitEntry {
    /// カタカナ読み (例: `"キロメートル"`)
    pub kana: String,
    /// 大文字小文字を区別しないか (default false)
    #[serde(default)]
    pub ci: bool,
}

/// units.toml 全体
#[derive(Debug, Default, Clone, Deserialize)]
pub struct UnitsData {
    /// シンボル → エントリ
    #[serde(default)]
    pub entries: HashMap<String, UnitEntry>,
}

impl UnitsData {
    /// シンボルに対応する読みを返す。
    /// `ci = true` のエントリは大文字小文字を比較しない。
    #[must_use]
    pub fn lookup(&self, symbol: &str) -> Option<&str> {
        if let Some(e) = self.entries.get(symbol) {
            return Some(e.kana.as_str());
        }
        let symbol_lower = symbol.to_lowercase();
        self.entries
            .iter()
            .find(|(k, e)| e.ci && k.to_lowercase() == symbol_lower)
            .map(|(_, e)| e.kana.as_str())
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

    fn sample() -> UnitsData {
        let toml_str = r#"
            [entries]
            "km" = { kana = "キロメートル" }
            "L"  = { kana = "リットル", ci = true }
            "mL" = { kana = "ミリリットル", ci = true }
        "#;
        toml::from_str(toml_str).unwrap()
    }

    #[test]
    fn strict_match() {
        let d = sample();
        assert_eq!(d.lookup("km"), Some("キロメートル"));
    }

    #[test]
    fn ci_match_when_flagged() {
        let d = sample();
        assert_eq!(d.lookup("l"), Some("リットル"));
        assert_eq!(d.lookup("ml"), Some("ミリリットル"));
        assert_eq!(d.lookup("ML"), Some("ミリリットル"));
    }

    #[test]
    fn ci_does_not_apply_when_not_flagged() {
        let d = sample();
        assert_eq!(d.lookup("KM"), None); // km は ci なし
    }

    #[test]
    fn miss_returns_none() {
        let d = sample();
        assert_eq!(d.lookup("光年"), None);
    }
}
