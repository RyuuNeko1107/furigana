//! SI 単位読み (units.toml)
//!
//! km / cm / mm / m / kg / mg / g / t / mL / L / TB / GB / MB / KB …
//!
//! ## 例
//! ```toml
//! [entries]
//! "km" = { kana = "キロメートル" }
//! "L"  = { kana = "リットル" }
//! "mL" = { kana = "ミリリットル" }
//! ```
//!
//! 単位は本質的に case-insensitive (km/KM/Km は全部「キロメートル」と読む)
//! ため lookup はデフォルトで大文字小文字を区別しない。SI で厳密に区別したい
//! 場合のみ `ci = false` で opt-out する (例: `mg` (ミリグラム) と `Mg`
//! (メガグラム) の表記揺れを区別したい等)。

use serde::Deserialize;
use std::collections::HashMap;

/// units.toml 1 件 (HashMap の value 側)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UnitEntry {
    /// カタカナ読み (例: `"キロメートル"`)
    pub kana: String,
    /// 大文字小文字を区別しないか (default true。SI で厳密に区別したい場合のみ false)
    #[serde(default = "default_ci")]
    pub ci: bool,
}

fn default_ci() -> bool {
    true
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
    ///
    /// 順序:
    /// 1. 完全一致 (大小区別する `ci = false` エントリを優先するため)
    /// 2. `ci = true` のエントリで lowercase 比較
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
            "L"  = { kana = "リットル" }
            "mL" = { kana = "ミリリットル" }
            "Mg" = { kana = "メガグラム", ci = false }
            "mg" = { kana = "ミリグラム", ci = false }
        "#;
        toml::from_str(toml_str).unwrap()
    }

    #[test]
    fn ci_default_true_matches_any_case() {
        let d = sample();
        // ci default true なので km/KM/Km/kM 全部 hit
        assert_eq!(d.lookup("km"), Some("キロメートル"));
        assert_eq!(d.lookup("KM"), Some("キロメートル"));
        assert_eq!(d.lookup("Km"), Some("キロメートル"));
        assert_eq!(d.lookup("kM"), Some("キロメートル"));
        assert_eq!(d.lookup("l"), Some("リットル"));
        assert_eq!(d.lookup("ml"), Some("ミリリットル"));
        assert_eq!(d.lookup("ML"), Some("ミリリットル"));
    }

    #[test]
    fn ci_false_keeps_strict_match() {
        let d = sample();
        // SI 区別 (mg=ミリグラム / Mg=メガグラム) は完全一致で取れる
        assert_eq!(d.lookup("mg"), Some("ミリグラム"));
        assert_eq!(d.lookup("Mg"), Some("メガグラム"));
    }

    #[test]
    fn miss_returns_none() {
        let d = sample();
        assert_eq!(d.lookup("光年"), None);
    }
}
