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
//! 起動時に user/core dict ディレクトリ配下の `*.toml` を **全階層再帰** で scan し、
//! `HashMap<String, String>` にマージする (`core/jukugo/general.toml` も
//! `core/works/game/series/touhou.toml` も同じく拾われる)。
//!
//! 優先度の制御は呼び出し側 (Furigana 構造体) で行う想定。
//! Dict 自体は単一階層 — 後に挿入したエントリが先のエントリを上書きする。
//!
//! ## 内部構造 (jukugo / unihan の二段)
//!
//! 内部では surface 文字数で 2 つの HashMap に振り分ける:
//!
//! - **`jukugo`** : surface が 2 文字以上 (= 漢字熟語 / 固有名詞 / 複合語)
//! - **`unihan`** : surface が 1 文字 (= 単漢字フォールバック)
//!
//! [`Self::lookup_jukugo`] と [`Self::lookup_unihan`] で別々に lookup できる。
//! [`Self::lookup`] は両者を試す互換 API (jukugo 優先)。
//!
//! 呼び出し側 ([`crate::reading::pipeline::resolve_reading`]) は
//! `context rule → jukugo lookup → Lindera reading → unihan lookup` の優先順位で評価する。
//! こうすることで、Lindera が動詞活用形 surface に対して持っている自然な reading を
//! 単漢字 unihan の保守的読みが横取りすることがなくなる。

use crate::error::{FuriganaError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// ディレクトリを再帰的に walk して `*.toml` のフルパスを収集する。
///
/// 配布 tar.gz の展開結果を想定するため、symlink ループや権限なしディレクトリは
/// std::fs のエラーが上に伝播する (caller 側で `?` で素直に返る)。
///
/// 以下は **意図的に skip** する (Dict::from_toml_dir の対象外):
/// - `loanwords/` サブディレクトリ: ASCII surface 専用、 `Loanwords::from_toml_dir`
///   経由で別管理 (jukugo prefix-match で「TypeScript」 等が誤って hit するのを防ぐ)
/// - `single_overrides.toml`: 単漢字 surface の default reading override 専用、
///   `SingleOverrides::from_toml_file` 経由で別管理 (Dict に取り込むと既存 unihan を
///   silent merge で上書きしてしまうため別経路で持つ)
fn collect_toml_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "toml") {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // single_overrides.toml は SingleOverrides 側で別 load
            if name == "single_overrides.toml" {
                continue;
            }
            // *.test.toml は CI 専用の inline test、 lib runtime には不要
            // (release tar からも `--exclude='*.test.toml'` で除外、 通常 dev
            // checkout にだけ存在する想定)
            if name.ends_with(".test.toml") {
                continue;
            }
            // _genre.toml は STATS.md sub-section description 用メタ、 entries なし
            if name == "_genre.toml" {
                continue;
            }
            out.push(path);
        } else if path.is_dir() {
            // loanwords/ は ASCII surface 専用 (Loanwords 側で別 load)
            if path.file_name().is_some_and(|n| n == "loanwords") {
                continue;
            }
            collect_toml_files_recursive(&path, out)?;
        }
    }
    Ok(())
}

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
///
/// 内部では surface 文字数で `jukugo` (≥2 文字) と `unihan` (1 文字) を分けて保持。
#[derive(Debug, Default, Clone)]
pub struct Dict {
    /// 熟語・固有名詞・複合語 (surface ≥ 2 文字)
    jukugo: HashMap<String, String>,
    /// 単漢字フォールバック (surface = 1 文字)
    unihan: HashMap<String, String>,
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
    /// surface 文字数で内部的に jukugo / unihan に振り分け。
    ///
    /// # Errors
    /// TOML パース失敗時 [`FuriganaError::Toml`]。
    pub fn from_toml_str(content: &str, file: &str) -> Result<Self> {
        let parsed: DictFile = toml::from_str(content).map_err(|e| FuriganaError::Toml {
            file: file.to_string(),
            source: e,
        })?;
        let mut d = Self::default();
        // string 値だけ採用。inline table 等は rules 用ファイルなので silent skip。
        // 各 entry の surface (key) と reading (value) は sanitize_dict_value で
        // 制御文字 / Unicode bidi override / zero-width / 過大長 を reject する
        // (任意コード埋め込み / Trojan Source 攻撃 / homoglyph 詐称防御)。
        for (k, v) in parsed.entries {
            if let Some(s) = v.as_str() {
                crate::sanitize::sanitize_dict_value("dict surface", &k)
                    .map_err(|e| FuriganaError::Validation(format!("{file}: {e}")))?;
                crate::sanitize::sanitize_dict_value("dict reading", s)
                    .map_err(|e| FuriganaError::Validation(format!("{file}: {e}")))?;
                d.insert(k, s.to_string());
            }
        }
        Ok(d)
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
    /// - **サブディレクトリは無制限に再帰** (例: `core/jukugo/general.toml`、
    ///   `core/works/game/touhou.toml` 等の任意の深さ)
    /// - 全 `*.toml` を集めて絶対パス順でソートし、後に来るファイルが上書きする
    /// - ディレクトリが存在しない場合は空辞書を返す
    /// - 配布 tar.gz から展開した静的データを想定するため、symlink ループ対策は持たない
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

        let mut files: Vec<std::path::PathBuf> = Vec::new();
        collect_toml_files_recursive(dir, &mut files)?;
        files.sort();

        let mut merged = Self::default();
        for f in files {
            let part = Self::from_toml_file(&f)?;
            merged.merge(part);
        }
        Ok(merged)
    }

    /// surface に対応する読みを返す (jukugo 優先で fallback で unihan を見る、互換 API)
    ///
    /// 新規コードは [`Self::lookup_jukugo`] / [`Self::lookup_unihan`] を分けて
    /// 使い、resolve_reading の優先順位に組み込むのが推奨。
    #[must_use]
    pub fn lookup(&self, surface: &str) -> Option<&str> {
        self.jukugo
            .get(surface)
            .or_else(|| self.unihan.get(surface))
            .map(String::as_str)
    }

    /// 熟語辞書 (surface ≥ 2 文字) のみを lookup
    #[must_use]
    pub fn lookup_jukugo(&self, surface: &str) -> Option<&str> {
        self.jukugo.get(surface).map(String::as_str)
    }

    /// 単漢字辞書 (surface = 1 文字) のみを lookup
    #[must_use]
    pub fn lookup_unihan(&self, surface: &str) -> Option<&str> {
        self.unihan.get(surface).map(String::as_str)
    }

    /// エントリを追加 (既存 surface は上書き)
    ///
    /// surface 文字数で内部的に jukugo / unihan に振り分け。
    pub fn insert(&mut self, surface: impl Into<String>, reading: impl Into<String>) {
        let s = surface.into();
        let r = reading.into();
        if s.chars().count() == 1 {
            self.unihan.insert(s, r);
        } else {
            self.jukugo.insert(s, r);
        }
    }

    /// 別の Dict を merge (other の方が後勝ち)
    pub fn merge(&mut self, other: Self) {
        self.jukugo.extend(other.jukugo);
        self.unihan.extend(other.unihan);
    }

    /// 件数 (jukugo + unihan の合計)
    #[must_use]
    pub fn len(&self) -> usize {
        self.jukugo.len() + self.unihan.len()
    }

    /// 空判定
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.jukugo.is_empty() && self.unihan.is_empty()
    }

    /// 熟語のみの件数 (デバッグ用)
    #[must_use]
    pub fn jukugo_len(&self) -> usize {
        self.jukugo.len()
    }

    /// 単漢字のみの件数 (デバッグ用)
    #[must_use]
    pub fn unihan_len(&self) -> usize {
        self.unihan.len()
    }

    /// 熟語の (surface, reading) ペアを iter 公開
    ///
    /// `chunks::NumberChunker` が起動時に jukugo の Aho-Corasick automaton を
    /// build するために使う (counter chunk が jukugo entry の真部分集合になって
    /// いる場合に jukugo を優先するため)。
    pub fn jukugo_iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.jukugo.iter().map(|(k, v)| (k.as_str(), v.as_str()))
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

    #[test]
    fn from_toml_dir_recurses_arbitrary_depth() {
        // works/game/touhou.toml のような任意深度の構造を扱えること
        let dir = fresh_temp_dir("dir_deep");
        let deep = dir.join("works").join("game").join("series");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(
            deep.join("touhou.toml"),
            "[entries]\n\"霊夢\" = \"レイム\"\n\"魔理沙\" = \"マリサ\"\n",
        )
        .unwrap();
        // 別の深い階層
        let deep2 = dir.join("works").join("anime");
        std::fs::create_dir_all(&deep2).unwrap();
        std::fs::write(
            deep2.join("placeholder.toml"),
            "[entries]\n\"宵闇\" = \"ヨイヤミ\"\n",
        )
        .unwrap();

        let d = Dict::from_toml_dir(&dir).unwrap();
        assert_eq!(d.lookup("霊夢"), Some("レイム"));
        assert_eq!(d.lookup("魔理沙"), Some("マリサ"));
        assert_eq!(d.lookup("宵闇"), Some("ヨイヤミ"));
        assert_eq!(d.len(), 3);

        std::fs::remove_dir_all(&dir).ok();
    }
}
