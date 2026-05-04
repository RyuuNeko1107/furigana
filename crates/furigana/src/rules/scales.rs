//! 大数スケール読み (scales.tsv)
//!
//! 万 / 億 / 兆 / 京 / 垓 / 秭 / 穣 / 溝 / 澗 / 正 / 載 / 極 / 恒河沙 …
//!
//! ## 例 (TSV: 漢字\t読み)
//! ```text
//! 万	マン
//! 億	オク
//! 兆	チョウ
//! 京	ケイ
//! ```
//!
//! 1 行 = 1 エントリ。空行・`#` 始まりはコメント。

/// scales.tsv 1 行
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaleEntry {
    /// 漢字 1〜数文字 (例: "万", "恒河沙")
    pub kanji: String,
    /// カタカナ読み (例: "マン", "ゴウガシャ")
    pub kana: String,
}

/// scales.tsv 全体 (順序保持: ファイル記載順 = 大→小推奨)
#[derive(Debug, Default, Clone)]
pub struct ScalesData {
    /// エントリ列。ロード時に大→小ソートを期待する側で並べる。
    pub entries: Vec<ScaleEntry>,
}

impl ScalesData {
    /// 漢字に対応する読みを線形検索する (エントリ数が少ないため許容)
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
    fn lookup_works() {
        let data = ScalesData {
            entries: vec![
                ScaleEntry {
                    kanji: "万".into(),
                    kana: "マン".into(),
                },
                ScaleEntry {
                    kanji: "億".into(),
                    kana: "オク".into(),
                },
                ScaleEntry {
                    kanji: "兆".into(),
                    kana: "チョウ".into(),
                },
            ],
        };
        assert_eq!(data.lookup("万"), Some("マン"));
        assert_eq!(data.lookup("億"), Some("オク"));
        assert_eq!(data.lookup("兆"), Some("チョウ"));
        assert_eq!(data.lookup("光年"), None);
        assert_eq!(data.len(), 3);
    }

    #[test]
    fn default_is_empty() {
        let data = ScalesData::default();
        assert!(data.is_empty());
    }
}
