//! 大数スケール読み (scales.toml)
//!
//! 万 / 億 / 兆 / 京 / 垓 / 秭 / 穣 / 溝 / 澗 / 正 / 載 / 極 / 恒河沙 …
//!
//! ## 例
//! ```toml
//! [[entry]]
//! kanji = "無量大数"
//! kana = "ムリョウタイスウ"
//!
//! [[entry]]
//! kanji = "万"
//! kana = "マン"
//! ```
//!
//! 順序は大→小推奨 (エンジン側でこの順で評価する)。

use serde::Deserialize;

/// scales.toml 1 件
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ScaleEntry {
    /// 漢字 1〜数文字 (例: `"万"`, `"恒河沙"`)
    pub kanji: String,
    /// カタカナ読み (例: `"マン"`, `"ゴウガシャ"`)
    pub kana: String,
}

/// scales.toml 全体 (順序保持)
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ScalesData {
    /// エントリ列 (記載順)
    #[serde(default, rename = "entry")]
    pub entries: Vec<ScaleEntry>,
}

impl ScalesData {
    /// 漢字に対応する読みを線形検索 (件数が少ないため許容)
    #[must_use]
    pub fn lookup(&self, kanji: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.kanji == kanji)
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

    #[test]
    fn parses_basic() {
        let toml_str = r#"
            [[entry]]
            kanji = "万"
            kana = "マン"

            [[entry]]
            kanji = "億"
            kana = "オク"
        "#;
        let data: ScalesData = toml::from_str(toml_str).unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data.lookup("万"), Some("マン"));
        assert_eq!(data.lookup("億"), Some("オク"));
        assert_eq!(data.lookup("光年"), None);
    }

    #[test]
    fn empty_input_yields_default() {
        let data: ScalesData = toml::from_str("").unwrap();
        assert!(data.is_empty());
    }
}
