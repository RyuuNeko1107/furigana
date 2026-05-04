//! 異体字マップ (compat_map.toml)
//!
//! 異体字 → 標準字 の正規化テーブル。
//! 例: 髙→高, 﨑→崎, 德→徳。
//! 形態素解析・辞書ルックアップに先立って入力テキストを正規化することで、
//! 同義表記揺れを 1 つの読みエントリにまとめられる。
//!
//! ## 例
//! ```toml
//! [map]
//! "髙" = "高"
//! "﨑" = "崎"
//! "德" = "徳"
//! ```

use serde::Deserialize;
use std::collections::HashMap;

/// compat_map.toml 全体 (variant → canonical)
#[derive(Debug, Default, Clone, Deserialize)]
pub struct CompatData {
    #[serde(default)]
    pub map: HashMap<String, String>,
}

impl CompatData {
    /// 異体字を標準字に変換 (該当無しなら None)
    #[must_use]
    pub fn lookup(&self, variant: &str) -> Option<&str> {
        self.map.get(variant).map(String::as_str)
    }

    /// 件数
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// 空判定
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_works() {
        let toml_str = r#"
            [map]
            "髙" = "高"
            "﨑" = "崎"
        "#;
        let d: CompatData = toml::from_str(toml_str).unwrap();
        assert_eq!(d.lookup("髙"), Some("高"));
        assert_eq!(d.lookup("﨑"), Some("崎"));
        assert_eq!(d.lookup("高"), None); // 逆引きはしない
    }

    #[test]
    fn default_is_empty() {
        let d = CompatData::default();
        assert!(d.is_empty());
    }
}
