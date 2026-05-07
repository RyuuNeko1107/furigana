//! 単漢字 surface の default reading override
//!
//! `resolve_reading` の Step 4 (Lindera reading) より先に評価されるため、
//! Lindera が「土 → ド」 のような short / 音読み寄りの reading を返した場合も
//! ここで「土 → ツチ」 のように override できる。
//!
//! ## なぜ unihan に直接書かないのか
//!
//! `core/unihan.toml` は「単漢字 fallback」 で `resolve_reading` の **Step 5**
//! (Lindera より後) で参照される。 `unihan` を Lindera より先に評価すると
//! 「米国産 → コメコクサン」「私の意見 → ワタクシ」 等の副作用が出る (issue #15
//! の R20 試行で 6 件 corpus regression を確認済み) ため、 **明示的に override
//! したい単漢字だけを集めた専用ファイル** を別に持つ設計。
//!
//! ## ファイル構成
//!
//! `core/single_overrides.toml` (single file、 `[entries]` セクション)。
//! key は **必ず 1 字漢字 surface**、 value はカタカナ読み。
//! 2 字以上の surface は load 時に silent skip される (validate.py が PR 段階で
//! 検出するので通常 load 時には来ない)。

use crate::error::{FuriganaError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// `[entries]` セクションを受ける defensive 型
#[derive(Debug, Default, Deserialize)]
struct SingleOverridesFile {
    #[serde(default)]
    entries: HashMap<String, toml::Value>,
}

/// 単漢字 surface → カタカナ reading の override マップ
#[derive(Debug, Default, Clone)]
pub struct SingleOverrides {
    map: HashMap<String, String>,
}

impl SingleOverrides {
    /// 空 override
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// TOML 文字列から構築
    ///
    /// # Errors
    /// TOML パース失敗時 [`FuriganaError::Toml`]。
    pub fn from_toml_str(content: &str, file: &str) -> Result<Self> {
        let parsed: SingleOverridesFile =
            toml::from_str(content).map_err(|e| FuriganaError::Toml {
                file: file.to_string(),
                source: e,
            })?;
        let mut d = Self::default();
        for (k, v) in parsed.entries {
            // 1 字 surface 以外は silent skip (validate.py で fail させる役割)
            if k.chars().count() != 1 {
                continue;
            }
            if let toml::Value::String(reading) = v {
                // sanitize: 制御文字 / bidi override / zero-width / 過大長 reject
                crate::sanitize::sanitize_dict_value("single_override surface", &k)
                    .map_err(|e| FuriganaError::Validation(format!("{file}: {e}")))?;
                crate::sanitize::sanitize_dict_value("single_override reading", &reading)
                    .map_err(|e| FuriganaError::Validation(format!("{file}: {e}")))?;
                d.map.insert(k, reading);
            }
        }
        Ok(d)
    }

    /// TOML ファイルから構築
    ///
    /// # Errors
    /// I/O 失敗 / TOML パース失敗。
    pub fn from_toml_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        Self::from_toml_str(&content, &path.display().to_string())
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

    /// 単漢字 surface の override reading を返す (なければ None)
    #[must_use]
    pub fn lookup(&self, surface: &str) -> Option<&str> {
        if surface.chars().count() != 1 {
            return None;
        }
        self.map.get(surface).map(String::as_str)
    }

    /// (surface, reading) ペアを iter (api.rs での merge 用)
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.map.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// エントリ追加 (1 字 surface のみ採用)
    pub fn insert(&mut self, surface: String, reading: String) {
        if surface.chars().count() == 1 {
            self.map.insert(surface, reading);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_toml_str_basic() {
        let toml = r#"
[entries]
"土" = "ツチ"
"鋸" = "ノコギリ"
"#;
        let d = SingleOverrides::from_toml_str(toml, "test").unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d.lookup("土"), Some("ツチ"));
        assert_eq!(d.lookup("鋸"), Some("ノコギリ"));
    }

    #[test]
    fn lookup_rejects_multi_char_surface() {
        let mut d = SingleOverrides::default();
        d.map.insert("土地".to_string(), "トチ".to_string());
        // map に 2 字 surface が入っていても lookup は 1 字制約で reject
        assert_eq!(d.lookup("土地"), None);
    }

    #[test]
    fn from_toml_silently_skips_multi_char() {
        let toml = r#"
[entries]
"土" = "ツチ"
"土地" = "トチ"
"#;
        let d = SingleOverrides::from_toml_str(toml, "test").unwrap();
        // 1 字 surface のみ採用
        assert_eq!(d.len(), 1);
        assert_eq!(d.lookup("土"), Some("ツチ"));
        assert_eq!(d.lookup("土地"), None);
    }
}
