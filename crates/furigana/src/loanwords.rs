//! 外来語 (loanwords) 辞書
//!
//! IT 用語 / プログラミング言語 / OSS / クラウドサービス / 企業名 等の
//! ASCII surface に対する読み (カタカナ) を提供する。
//!
//! ## 特徴
//!
//! - **完全一致のみ** (substring 切断なし): chunk 全体が dict に存在する場合のみ採用。
//!   「PostgreSQL」 chunk が「Post」 entry に部分一致しても採用しない。
//! - **正規化**: case-fold (大文字小文字無視) + 全角→半角 (Ｋｕｂｅｒｎｅｔｅｓ → Kubernetes)。
//!   dict 登録 surface は canonical form で書く想定だが、 入力側のブレを吸収する。
//!
//! ## ファイル構成
//!
//! `core/loanwords/**/*.toml` を全階層再帰で `[entries]` セクションを load し、
//! 単一の HashMap にマージする (jukugo と同じパターン)。
//!
//! ## ロード後の lookup
//!
//! [`Loanwords::lookup`] は surface を正規化してから完全一致で reading を返す。
//! 呼び出し側 ([`crate::chunks::NumberChunker::split`] の階層 4.7) は、
//! 英単語 chunk を 1 unit として切り出してから lookup する。

use crate::error::{FuriganaError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// `[entries]` セクションを受ける defensive な型。
#[derive(Debug, Default, Deserialize)]
struct LoanwordsFile {
    #[serde(default)]
    entries: HashMap<String, toml::Value>,
}

/// 外来語辞書 (case-folded + 全角→半角 正規化済 key で保持)
#[derive(Debug, Default, Clone)]
pub struct Loanwords {
    /// key は normalize() で正規化済 (lookup 側で同じ normalize を通す)
    map: HashMap<String, String>,
}

impl Loanwords {
    /// 空辞書
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// surface を case-fold + 全角→半角 で正規化
    ///
    /// - ASCII 大文字 → 小文字 (`A`-`Z` → `a`-`z`)
    /// - 全角英数字 (U+FF21-FF5A 等) → 半角 (U+0041-007A) に変換してから case-fold
    /// - その他 (記号 . - _ + #) はそのまま保持
    #[must_use]
    pub fn normalize(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            let half = match c {
                // 全角英大文字 Ａ-Ｚ → 半角 A-Z
                'Ａ'..='Ｚ' => char::from_u32((c as u32) - 0xFF21 + 0x41).unwrap_or(c),
                // 全角英小文字 ａ-ｚ → 半角 a-z
                'ａ'..='ｚ' => char::from_u32((c as u32) - 0xFF41 + 0x61).unwrap_or(c),
                // 全角数字 ０-９ → 半角 0-9
                '０'..='９' => char::from_u32((c as u32) - 0xFF10 + 0x30).unwrap_or(c),
                // 全角ハイフン・記号 → 半角
                '－' => '-',
                '＋' => '+',
                '．' => '.',
                '＿' => '_',
                '＃' => '#',
                _ => c,
            };
            // case-fold: ASCII A-Z → a-z
            let folded = if half.is_ascii_uppercase() {
                half.to_ascii_lowercase()
            } else {
                half
            };
            out.push(folded);
        }
        out
    }

    /// TOML 文字列から辞書を構築
    ///
    /// # Errors
    /// TOML パース失敗時 [`FuriganaError::Toml`]。
    pub fn from_toml_str(content: &str, file: &str) -> Result<Self> {
        let parsed: LoanwordsFile = toml::from_str(content).map_err(|e| FuriganaError::Toml {
            file: file.to_string(),
            source: e,
        })?;
        let mut d = Self::default();
        for (k, v) in parsed.entries {
            if let toml::Value::String(reading) = v {
                // sanitize: 制御文字 / bidi override / zero-width / 過大長 reject
                crate::sanitize::sanitize_dict_value("loanword surface", &k)
                    .map_err(|e| FuriganaError::Validation(format!("{file}: {e}")))?;
                crate::sanitize::sanitize_dict_value("loanword reading", &reading)
                    .map_err(|e| FuriganaError::Validation(format!("{file}: {e}")))?;
                d.insert(k, reading);
            }
        }
        Ok(d)
    }

    /// TOML ファイルから辞書を構築
    ///
    /// # Errors
    /// I/O 失敗 / TOML パース失敗。
    pub fn from_toml_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        Self::from_toml_str(&content, &path.display().to_string())
    }

    /// ディレクトリを再帰的に walk して `*.toml` を全て merge
    ///
    /// jukugo と同じパターン (`Dict::from_toml_dir` と同じ挙動)。
    ///
    /// # Errors
    /// I/O 失敗 / TOML パース失敗。
    pub fn from_toml_dir<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let dir = dir.as_ref();
        let mut files: Vec<PathBuf> = Vec::new();
        if dir.is_dir() {
            collect_toml_recursive(dir, &mut files)?;
        }
        files.sort();
        let mut merged = Self::default();
        for f in files {
            merged.merge(Self::from_toml_file(&f)?);
        }
        Ok(merged)
    }

    /// エントリ追加 (key は正規化される、 後勝ち上書き)
    pub fn insert(&mut self, surface: impl Into<String>, reading: impl Into<String>) {
        let s = surface.into();
        let r = reading.into();
        let normalized = Self::normalize(&s);
        self.map.insert(normalized, r);
    }

    /// 別の Loanwords をマージ (other の方が後勝ち)
    pub fn merge(&mut self, other: Self) {
        self.map.extend(other.map);
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

    /// surface を正規化して完全一致 lookup
    ///
    /// 完全一致のみ (substring 不可)。 hit すれば dict 登録時の reading を返す。
    #[must_use]
    pub fn lookup(&self, surface: &str) -> Option<&str> {
        let normalized = Self::normalize(surface);
        self.map.get(&normalized).map(String::as_str)
    }
}

fn collect_toml_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "toml") {
            out.push(path);
        } else if path.is_dir() {
            collect_toml_recursive(&path, out)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_case_fold() {
        assert_eq!(Loanwords::normalize("Kubernetes"), "kubernetes");
        assert_eq!(Loanwords::normalize("KUBERNETES"), "kubernetes");
        assert_eq!(Loanwords::normalize("kubernetes"), "kubernetes");
    }

    #[test]
    fn normalize_full_to_half() {
        assert_eq!(Loanwords::normalize("Ｋｕｂｅｒｎｅｔｅｓ"), "kubernetes");
        assert_eq!(Loanwords::normalize("PostgreSQL"), "postgresql");
        assert_eq!(Loanwords::normalize("Ｐｏｓｔｇｒｅ１６"), "postgre16");
    }

    #[test]
    fn normalize_keeps_symbols() {
        assert_eq!(Loanwords::normalize("C++"), "c++");
        assert_eq!(Loanwords::normalize(".NET"), ".net");
        assert_eq!(Loanwords::normalize("node_modules"), "node_modules");
        assert_eq!(
            Loanwords::normalize("TypeScript-config"),
            "typescript-config"
        );
    }

    #[test]
    fn lookup_exact_match_only() {
        let mut d = Loanwords::default();
        d.insert("Kubernetes", "クバネティス");
        assert_eq!(d.lookup("Kubernetes"), Some("クバネティス"));
        assert_eq!(d.lookup("kubernetes"), Some("クバネティス")); // case-fold OK
        assert_eq!(d.lookup("Ｋｕｂｅｒｎｅｔｅｓ"), Some("クバネティス")); // 全角 OK
        assert_eq!(d.lookup("Kuber"), None); // substring NG
        assert_eq!(d.lookup("KubernetesXXX"), None);
    }

    #[test]
    fn from_toml_str_basic() {
        let toml = r#"
[entries]
"Kubernetes" = "クバネティス"
"Docker" = "ドッカー"
"#;
        let d = Loanwords::from_toml_str(toml, "test").unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d.lookup("kubernetes"), Some("クバネティス"));
        assert_eq!(d.lookup("DOCKER"), Some("ドッカー"));
    }
}
