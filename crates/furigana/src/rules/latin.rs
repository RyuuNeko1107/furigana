//! ラテン文字読み (latin.tsv)
//!
//! 英字 1 文字 → カタカナ (A→エー, B→ビー, ...)
//!
//! ## 例 (TSV: 文字\t読み)
//! ```text
//! A	エー
//! B	ビー
//! C	シー
//! ```
//!
//! lookup は **大文字小文字を区別せず** 行う (大文字キーで保存推奨)。

/// latin.tsv 1 行
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatinEntry {
    /// 文字 (大文字推奨、1 文字)
    pub letter: String,
    /// カタカナ読み
    pub kana: String,
}

/// latin.tsv 全体
#[derive(Debug, Default, Clone)]
pub struct LatinData {
    /// エントリ列
    pub entries: Vec<LatinEntry>,
}

impl LatinData {
    /// 1 文字を読みに変換 (case-insensitive)
    #[must_use]
    pub fn lookup(&self, ch: char) -> Option<&str> {
        let upper = ch.to_ascii_uppercase().to_string();
        self.entries
            .iter()
            .find(|e| e.letter.to_ascii_uppercase() == upper)
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

    fn sample() -> LatinData {
        LatinData {
            entries: vec![
                LatinEntry {
                    letter: "A".into(),
                    kana: "エー".into(),
                },
                LatinEntry {
                    letter: "B".into(),
                    kana: "ビー".into(),
                },
                LatinEntry {
                    letter: "Z".into(),
                    kana: "ズィー".into(),
                },
            ],
        }
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
