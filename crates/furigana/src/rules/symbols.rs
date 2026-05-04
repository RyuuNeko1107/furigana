//! 記号読み (symbols.tsv)
//!
//! +, −, ±, %, ‰, /, etc.
//!
//! ## 例 (TSV: 記号\t読み)
//! ```text
//! +	プラス
//! -	マイナス
//! ±	プラスマイナス
//! %	パーセント
//! ‰	パーミル
//! /	スラッシュ
//! ```

/// symbols.tsv 1 行
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolEntry {
    /// 記号 (1〜数文字)
    pub symbol: String,
    /// カタカナ読み
    pub kana: String,
}

/// symbols.tsv 全体
#[derive(Debug, Default, Clone)]
pub struct SymbolsData {
    /// エントリ列
    pub entries: Vec<SymbolEntry>,
}

impl SymbolsData {
    /// 記号で読みを引く
    #[must_use]
    pub fn lookup(&self, symbol: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.symbol == symbol)
            .map(|e| e.kana.as_str())
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
        SymbolsData {
            entries: vec![
                SymbolEntry {
                    symbol: "+".into(),
                    kana: "プラス".into(),
                },
                SymbolEntry {
                    symbol: "-".into(),
                    kana: "マイナス".into(),
                },
                SymbolEntry {
                    symbol: "%".into(),
                    kana: "パーセント".into(),
                },
                SymbolEntry {
                    symbol: "‰".into(),
                    kana: "パーミル".into(),
                },
            ],
        }
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
