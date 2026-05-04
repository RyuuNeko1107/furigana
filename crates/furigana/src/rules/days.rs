//! 日付の特殊読み (days.toml)
//!
//! 1 日〜31 日の特殊な訓読み (ツイタチ / フツカ / ミッカ ...)。
//!
//! ## 例
//! ```toml
//! "1" = "ツイタチ"
//! "2" = "フツカ"
//! "3" = "ミッカ"
//! "20" = "ハツカ"
//! ```
//!
//! TOML のキーは文字列必須なので、数値も "1" のように文字列で書く。
//! ファイルに無い数値はデフォルト処理 (例: `15日`→ジュウゴニチ) に委譲する。

use serde::Deserialize;
use std::collections::HashMap;

/// days.toml 全体 (string キー → カタカナ読み)
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(transparent)]
pub struct DaysData(pub HashMap<String, String>);

impl DaysData {
    /// 数値で参照する。該当が無ければ `None`。
    #[must_use]
    pub fn get(&self, day: u32) -> Option<&str> {
        self.0.get(&day.to_string()).map(String::as_str)
    }

    /// 登録件数
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 空判定
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_days() {
        let toml_str = r#"
            "1" = "ツイタチ"
            "2" = "フツカ"
            "20" = "ハツカ"
        "#;
        let data: DaysData = toml::from_str(toml_str).unwrap();
        assert_eq!(data.get(1), Some("ツイタチ"));
        assert_eq!(data.get(2), Some("フツカ"));
        assert_eq!(data.get(20), Some("ハツカ"));
        assert_eq!(data.get(15), None);
        assert_eq!(data.len(), 3);
    }

    #[test]
    fn empty_input_yields_default() {
        let data: DaysData = toml::from_str("").unwrap();
        assert!(data.is_empty());
    }
}
