//! 数詞慣用語句 (numeric_phrases.tsv)
//!
//! 数字を含む慣用語句 (二十歳→ハタチ、明後日→アサッテ等) を、形態素解析や
//! 助数詞ルールより先に確定させるための表。
//!
//! ## 例 (TSV: 表層\t読み)
//! ```text
//! 二十歳	ハタチ
//! 二十日	ハツカ
//! 一昨日	オトトイ
//! 明後日	アサッテ
//! 五重塔	ゴジュウノトウ
//! 三日月	ミカヅキ
//! 一人	ヒトリ
//! 二人	フタリ
//! ```

/// numeric_phrases.tsv 1 行
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericPhrase {
    /// 表層 (例: "二十歳")
    pub surface: String,
    /// カタカナ読み
    pub kana: String,
}

/// numeric_phrases.tsv 全体
#[derive(Debug, Default, Clone)]
pub struct NumericPhrasesData {
    /// エントリ列
    pub entries: Vec<NumericPhrase>,
}

impl NumericPhrasesData {
    /// 表層に対応する読みを返す
    #[must_use]
    pub fn lookup(&self, surface: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.surface == surface)
            .map(|e| e.kana.as_str())
    }

    /// 全表層を一覧 (regex builder 用)
    pub fn surfaces(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.surface.as_str())
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
        NumericPhrasesData {
            entries: vec![
                NumericPhrase {
                    surface: "二十歳".into(),
                    kana: "ハタチ".into(),
                },
                NumericPhrase {
                    surface: "明後日".into(),
                    kana: "アサッテ".into(),
                },
                NumericPhrase {
                    surface: "三日月".into(),
                    kana: "ミカヅキ".into(),
                },
            ],
        }
    }

    #[test]
    fn lookup_works() {
        let d = sample();
        assert_eq!(d.lookup("二十歳"), Some("ハタチ"));
        assert_eq!(d.lookup("明後日"), Some("アサッテ"));
        assert_eq!(d.lookup("夕日"), None);
    }

    #[test]
    fn surfaces_iter() {
        let d = sample();
        let surfaces: Vec<&str> = d.surfaces().collect();
        assert_eq!(surfaces, vec!["二十歳", "明後日", "三日月"]);
    }
}
