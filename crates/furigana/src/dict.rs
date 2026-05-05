//! 単純な surface → reading 辞書
//!
//! TOML ファイルから読み込む。フォーマット:
//!
//! ```toml
//! [entries]
//! "灰桜" = "ハイザクラ"
//! "黎明" = "レイメイ"
//! ```
//!
//! 起動時に user/core dict ディレクトリ配下の `*.toml` を全 scan し、
//! `HashMap<String, String>` にマージする。
//!
//! 優先度の制御は呼び出し側 (Furigana 構造体) で行う想定。
//! Dict 自体は単一階層 — 後に挿入したエントリが先のエントリを上書きする。

use crate::error::{FuriganaError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// TOML ファイルの `[entries]` セクションを受ける defensive な型。
///
/// value を `toml::Value` で受けて、後段で「string 値だけを拾う」フィルタを
/// かける。これにより rules 系ファイル (例: `units.toml` の `[entries]` は
/// `{ kana = "..." }` の inline table) と同じディレクトリに置かれていても
/// silent skip できる。core 辞書と rules を `data/` 1 階層に flat 配置する
/// ユースケース (paths::Paths::dict_core_dir == rules_dir) のための防御。
#[derive(Debug, Default, Deserialize)]
struct DictFile {
    #[serde(default)]
    entries: HashMap<String, toml::Value>,
}

/// 単純 HashMap ベースの surface→reading 辞書
#[derive(Debug, Default, Clone)]
pub struct Dict {
    entries: HashMap<String, String>,
}

impl Dict {
    /// 空辞書を作成
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// TOML 文字列から辞書を構築
    ///
    /// `[entries]` セクション直下の key→value を全て取り込む。
    /// 後勝ち (同じ surface があれば最後のものが採用)。
    ///
    /// # Errors
    /// TOML パース失敗時 [`FuriganaError::Toml`]。
    pub fn from_toml_str(content: &str, file: &str) -> Result<Self> {
        let parsed: DictFile = toml::from_str(content).map_err(|e| FuriganaError::Toml {
            file: file.to_string(),
            source: e,
        })?;
        // string 値だけ採用。inline table 等は rules 用ファイルなので silent skip。
        let entries = parsed
            .entries
            .into_iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
            .collect();
        Ok(Self { entries })
    }

    /// 単一 TOML ファイルから辞書を構築
    ///
    /// # Errors
    /// I/O 失敗 / TOML パース失敗。
    pub fn from_toml_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        Self::from_toml_str(&content, &path.display().to_string())
    }

    /// ディレクトリ配下の `*.toml` 全てから辞書をマージ構築
    ///
    /// - **サブディレクトリは 1 階層まで再帰** (`core/jukugo/general.toml` 等)
    /// - 直下 + サブディレクトリ直下の `*.toml` を全集合してファイル名ソート順で
    ///   読み込み、後に来るファイルが上書きする
    /// - ディレクトリが存在しない場合は空辞書を返す
    ///
    /// # Errors
    /// I/O 失敗 / TOML パース失敗。
    pub fn from_toml_dir<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let dir = dir.as_ref();
        if !dir.exists() {
            return Ok(Self::default());
        }
        if !dir.is_dir() {
            return Err(FuriganaError::Validation(format!(
                "dict path is not a directory: {}",
                dir.display()
            )));
        }

        // 直下 + 1 階層のサブディレクトリ直下の *.toml を集める
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        for entry in std::fs::read_dir(dir)?.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "toml") {
                files.push(path);
            } else if path.is_dir() {
                for sub in std::fs::read_dir(&path)?.filter_map(std::result::Result::ok) {
                    let sub_path = sub.path();
                    if sub_path.is_file() && sub_path.extension().is_some_and(|e| e == "toml") {
                        files.push(sub_path);
                    }
                }
            }
        }
        files.sort();

        let mut merged = Self::default();
        for f in files {
            let part = Self::from_toml_file(&f)?;
            merged.merge(part);
        }
        Ok(merged)
    }

    /// surface に対応する読みを返す (なければ None)
    #[must_use]
    pub fn lookup(&self, surface: &str) -> Option<&str> {
        self.entries.get(surface).map(String::as_str)
    }

    /// エントリを追加 (既存 surface は上書き)
    pub fn insert(&mut self, surface: impl Into<String>, reading: impl Into<String>) {
        self.entries.insert(surface.into(), reading.into());
    }

    /// 別の Dict を merge (other の方が後勝ち)
    pub fn merge(&mut self, other: Self) {
        self.entries.extend(other.entries);
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
    fn from_toml_str_basic() {
        let toml_str = r#"
            [entries]
            "灰桜" = "ハイザクラ"
            "黎明" = "レイメイ"
        "#;
        let d = Dict::from_toml_str(toml_str, "test.toml").unwrap();
        assert_eq!(d.lookup("灰桜"), Some("ハイザクラ"));
        assert_eq!(d.lookup("黎明"), Some("レイメイ"));
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn from_toml_str_empty() {
        let d = Dict::from_toml_str("", "test.toml").unwrap();
        assert!(d.is_empty());
    }

    #[test]
    fn from_toml_str_with_comments() {
        let toml_str = r#"
            # コメント
            [entries]
            "灰桜" = "ハイザクラ"  # inline comment
        "#;
        let d = Dict::from_toml_str(toml_str, "test.toml").unwrap();
        assert_eq!(d.lookup("灰桜"), Some("ハイザクラ"));
    }

    #[test]
    fn from_toml_str_invalid_errors() {
        let err = Dict::from_toml_str("[invalid", "test.toml").unwrap_err();
        assert!(matches!(err, FuriganaError::Toml { .. }));
    }

    #[test]
    fn merge_overwrites() {
        let mut a =
            Dict::from_toml_str("[entries]\n\"灰桜\" = \"ハイザクラ\"\n", "a.toml").unwrap();
        let b = Dict::from_toml_str(
            "[entries]\n\"灰桜\" = \"カイオウ\"\n\"黎明\" = \"レイメイ\"\n",
            "b.toml",
        )
        .unwrap();
        a.merge(b);
        assert_eq!(a.lookup("灰桜"), Some("カイオウ")); // b が後勝ち
        assert_eq!(a.lookup("黎明"), Some("レイメイ"));
    }

    #[test]
    fn insert_works() {
        let mut d = Dict::new();
        d.insert("灰桜", "ハイザクラ");
        assert_eq!(d.lookup("灰桜"), Some("ハイザクラ"));
    }

    fn fresh_temp_dir(suffix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "furigana_dict_test_{}_{}_{}",
            std::process::id(),
            suffix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn from_toml_dir_loads_multiple_files() {
        let dir = fresh_temp_dir("dir_load");
        std::fs::write(
            dir.join("01_a.toml"),
            "[entries]\n\"灰桜\" = \"ハイザクラ\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("02_b.toml"),
            "[entries]\n\"黎明\" = \"レイメイ\"\n",
        )
        .unwrap();

        let d = Dict::from_toml_dir(&dir).unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d.lookup("灰桜"), Some("ハイザクラ"));
        assert_eq!(d.lookup("黎明"), Some("レイメイ"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn from_toml_dir_filename_order_decides_priority() {
        let dir = fresh_temp_dir("dir_priority");
        std::fs::write(
            dir.join("01_lower.toml"),
            "[entries]\n\"灰桜\" = \"ハイザクラ\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("02_higher.toml"),
            "[entries]\n\"灰桜\" = \"カイオウ\"\n",
        )
        .unwrap();

        let d = Dict::from_toml_dir(&dir).unwrap();
        assert_eq!(d.lookup("灰桜"), Some("カイオウ"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn from_toml_dir_missing_returns_empty() {
        let d = Dict::from_toml_dir("/nonexistent/dir/path/xyz_furigana_test").unwrap();
        assert!(d.is_empty());
    }

    #[test]
    fn from_toml_dir_recurses_one_level_into_subdirs() {
        // jukugo/general.toml + jukugo/places.toml のような構造を扱えること
        let dir = fresh_temp_dir("dir_subdir");
        let sub = dir.join("jukugo");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("general.toml"),
            "[entries]\n\"灰桜\" = \"ハイザクラ\"\n",
        )
        .unwrap();
        std::fs::write(
            sub.join("places.toml"),
            "[entries]\n\"湯島\" = \"ユシマ\"\n",
        )
        .unwrap();
        // 直下のファイルもまだ拾えること
        std::fs::write(dir.join("top.toml"), "[entries]\n\"黎明\" = \"レイメイ\"\n").unwrap();

        let d = Dict::from_toml_dir(&dir).unwrap();
        assert_eq!(d.lookup("灰桜"), Some("ハイザクラ"));
        assert_eq!(d.lookup("湯島"), Some("ユシマ"));
        assert_eq!(d.lookup("黎明"), Some("レイメイ"));
        assert_eq!(d.len(), 3);

        std::fs::remove_dir_all(&dir).ok();
    }
}
