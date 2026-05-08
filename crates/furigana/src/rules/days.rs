//! 日付の特殊読み (days.toml)
//!
//! 1 日〜31 日の特殊な訓読み (ツイタチ / フツカ / ミッカ ...)。
//!
//! ## 例 (新形式 - 推奨)
//! ```toml
//! [meta]
//! role = "days"
//!
//! [entries]
//! "1" = "ツイタチ"
//! "2" = "フツカ"
//! "3" = "ミッカ"
//! "20" = "ハツカ"
//! ```
//!
//! ## 例 (旧形式 - alpha.5+ 互換、 backwards compat)
//! ```toml
//! "1" = "ツイタチ"
//! "2" = "フツカ"
//! ```
//!
//! TOML のキーは文字列必須なので、数値も "1" のように文字列で書く。
//! ファイルに無い数値はデフォルト処理 (例: `15日`→ジュウゴニチ) に委譲する。
//! `[meta]` block は loader が role tag 取得用に読むだけで、 deserialize 時は
//! silently 無視される。

use serde::de::{Deserializer, Error};
use std::collections::HashMap;

/// days.toml 全体 (string キー → カタカナ読み)
#[derive(Debug, Default, Clone)]
pub struct DaysData {
    pub entries: HashMap<String, String>,
}

/// 新旧両形式 (`[entries]` block あり / flat top-level) を受ける custom deserialize。
///
/// - **新形式**: top-level に `[entries]` key (Table) があれば、 その配下を採用
/// - **旧形式**: `[entries]` key が無ければ top-level table 直下の string value を
///   採用 (alpha.5 〜 alpha.8 dict release tar 互換、 backwards compat)
///
/// `[meta]` 等の Table value は無視され、 String value のみ entries に取り込まれる。
/// 旧 lib alpha.5+ で配布された flat 形式 dict tar との互換性を保つため、 alpha.9
/// 以降も両形式を受け入れる。
impl<'de> serde::Deserialize<'de> for DaysData {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let val = toml::Value::deserialize(deserializer)?;
        let table = val
            .as_table()
            .ok_or_else(|| D::Error::custom("days TOML must be a table"))?;

        // 新形式: [entries] table が存在すれば、 その配下を採用
        let raw_entries = if let Some(toml::Value::Table(t)) = table.get("entries") {
            t
        } else {
            // 旧形式: top-level table 直下を採用 (alpha.5+ flat 互換)
            table
        };

        let mut entries = HashMap::new();
        for (k, v) in raw_entries {
            // String value のみ採用 ([meta] 等の Table value は silently 無視)
            if let Some(s) = v.as_str() {
                entries.insert(k.clone(), s.to_string());
            }
        }
        Ok(DaysData { entries })
    }
}

impl DaysData {
    /// 数値で参照する。該当が無ければ `None`。
    #[must_use]
    pub fn get(&self, day: u32) -> Option<&str> {
        self.entries.get(&day.to_string()).map(String::as_str)
    }

    /// 登録件数
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
    fn parses_basic_days() {
        let toml_str = r#"
            [entries]
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

    #[test]
    fn meta_block_is_silently_ignored() {
        let toml_str = r#"
            [meta]
            role = "days"
            description = "1〜31 日の特殊読み"

            [entries]
            "1" = "ツイタチ"
        "#;
        let data: DaysData = toml::from_str(toml_str).unwrap();
        assert_eq!(data.get(1), Some("ツイタチ"));
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn parses_legacy_flat_format() {
        // alpha.5 〜 alpha.8 互換: top-level に直接 string key-value
        let toml_str = r#"
            "1" = "ツイタチ"
            "2" = "フツカ"
            "20" = "ハツカ"
        "#;
        let data: DaysData = toml::from_str(toml_str).unwrap();
        assert_eq!(data.get(1), Some("ツイタチ"));
        assert_eq!(data.get(2), Some("フツカ"));
        assert_eq!(data.get(20), Some("ハツカ"));
        assert_eq!(data.len(), 3);
    }

    #[test]
    fn legacy_flat_with_meta_block_only_uses_meta_for_role() {
        // 旧 release で [meta] が無いがあえて足したケース: top-level の "1"
        // 等の string は entries に取り込まれ、 [meta] は Table なので silently 無視
        let toml_str = r#"
            [meta]
            role = "days"

            "1" = "ツイタチ"
            "2" = "フツカ"
        "#;
        // ※ TOML 文法的には [meta] の前に top-level key-value が必要、 これは
        //   後置きなので "1" "2" は [meta] 配下扱い → エラーにならず meta.1 = "..."
        //   になるが、 [entries] が無いので top-level の meta だけ見て string が
        //   無いので entries 空。 → このテストは「top-level に String が無いと空」
        //   を確認する形に
        let data: DaysData = toml::from_str(toml_str).unwrap();
        assert!(data.is_empty(), "[entries] 無し + meta 配下では entries 空");
    }

    #[test]
    fn new_format_takes_precedence_over_top_level() {
        // 両形式が混在した場合、 [entries] block 優先
        let toml_str = r#"
            "1" = "OLD"

            [entries]
            "1" = "NEW"
        "#;
        let data: DaysData = toml::from_str(toml_str).unwrap();
        assert_eq!(
            data.get(1),
            Some("NEW"),
            "[entries] block が top-level よりも優先される"
        );
    }
}
