//! SI 単位読み (units.tsv)
//!
//! km / cm / mm / m / kg / mg / g / t / mL / L / TB / GB / MB / KB …
//!
//! ## 例 (TSV: シンボル\t読み[\tフラグ])
//! ```text
//! km	キロメートル
//! mL	ミリリットル	ci
//! L	リットル	ci
//! ```
//!
//! 第 3 列 (フラグ) はオプション。`ci` = case-insensitive (大文字小文字を区別しない)。

/// units.tsv 1 行
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitEntry {
    /// 単位記号 (例: "km", "mL")
    pub symbol: String,
    /// カタカナ読み (例: "キロメートル", "ミリリットル")
    pub kana: String,
    /// 大文字小文字を区別しないか (例: "L"/"l" 両方を「リットル」)
    pub case_insensitive: bool,
}

/// units.tsv 全体
#[derive(Debug, Default, Clone)]
pub struct UnitsData {
    /// エントリ列 (記載順)
    pub entries: Vec<UnitEntry>,
}

impl UnitsData {
    /// シンボルに対応する読みを返す。
    /// `case_insensitive` フラグ付きエントリは大文字小文字を比較しない。
    #[must_use]
    pub fn lookup(&self, symbol: &str) -> Option<&str> {
        // strict 一致を先に
        if let Some(e) = self.entries.iter().find(|e| e.symbol == symbol) {
            return Some(e.kana.as_str());
        }
        // CI 一致
        let symbol_lower = symbol.to_lowercase();
        self.entries
            .iter()
            .find(|e| e.case_insensitive && e.symbol.to_lowercase() == symbol_lower)
            .map(|e| e.kana.as_str())
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
        UnitsData {
            entries: vec![
                UnitEntry {
                    symbol: "km".into(),
                    kana: "キロメートル".into(),
                    case_insensitive: false,
                },
                UnitEntry {
                    symbol: "L".into(),
                    kana: "リットル".into(),
                    case_insensitive: true,
                },
                UnitEntry {
                    symbol: "mL".into(),
                    kana: "ミリリットル".into(),
                    case_insensitive: true,
                },
            ],
        }
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
