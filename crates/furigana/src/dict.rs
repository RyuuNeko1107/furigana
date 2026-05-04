//! 単純な surface → reading 辞書
//!
//! 起動時に user/core dict ディレクトリから TSV を全 scan し、
//! `HashMap<String, String>` に展開する。
//!
//! 優先度の制御は呼び出し側 (Furigana 構造体) で行う想定。
//! Dict 自体は単一階層 — 後に挿入したエントリが先のエントリを上書きする。

use crate::error::{FuriganaError, Result};
use std::collections::HashMap;
use std::path::Path;

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

    /// TSV 文字列から辞書を構築
    ///
    /// - 各行: `surface\treading`
    /// - 行コメント (`#`) と空行はスキップ
    /// - reading は前後の空白を trim
    /// - 後勝ち (同じ surface があれば最後のものが採用)
    ///
    /// # Errors
    /// 行のフォーマット不正時 [`FuriganaError::Tsv`]。
    pub fn from_tsv_str(content: &str, file: &str) -> Result<Self> {
        let mut entries = HashMap::new();
        for (idx, raw) in content.lines().enumerate() {
            let line = raw.trim_end_matches('\r');
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let mut cols = line.splitn(2, '\t');
            let surface = cols.next().unwrap_or("").trim();
            let reading = cols.next().unwrap_or("").trim();
            if surface.is_empty() || reading.is_empty() {
                return Err(FuriganaError::Tsv {
                    file: file.to_string(),
                    line: idx + 1,
                    message: "expected 2 tab-separated columns (surface, reading)".to_string(),
                });
            }
            entries.insert(surface.to_string(), reading.to_string());
        }
        Ok(Self { entries })
    }

    /// 単一 TSV ファイルから辞書を構築
    ///
    /// # Errors
    /// I/O 失敗 / TSV パース失敗。
    pub fn from_tsv_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        Self::from_tsv_str(&content, &path.display().to_string())
    }

    /// ディレクトリ配下の `*.tsv` 全てから辞書をマージ構築
    ///
    /// - サブディレクトリは再帰しない (1 階層のみ)
    /// - ファイル名のソート順で読み込み (decision: 後に来るファイルが上書き)
    /// - ディレクトリが存在しない場合は空辞書を返す (エラーにしない)
    ///
    /// # Errors
    /// I/O 失敗 / TSV パース失敗。
    pub fn from_tsv_dir<P: AsRef<Path>>(dir: P) -> Result<Self> {
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

        // サブディレクトリは無視、*.tsv のみ収集してソート
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "tsv"))
            .collect();
        files.sort();

        let mut merged = Self::default();
        for f in files {
            let part = Self::from_tsv_file(&f)?;
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
    fn from_tsv_str_basic() {
        let tsv = "灰桜\tハイザクラ\n\
                   黎明\tレイメイ\n";
        let d = Dict::from_tsv_str(tsv, "test.tsv").unwrap();
        assert_eq!(d.lookup("灰桜"), Some("ハイザクラ"));
        assert_eq!(d.lookup("黎明"), Some("レイメイ"));
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn from_tsv_str_skips_comments_and_blanks() {
        let tsv = "# comment\n\n灰桜\tハイザクラ\n# another\n\n";
        let d = Dict::from_tsv_str(tsv, "test.tsv").unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d.lookup("灰桜"), Some("ハイザクラ"));
    }

    #[test]
    fn from_tsv_str_handles_crlf() {
        let tsv = "灰桜\tハイザクラ\r\n黎明\tレイメイ\r\n";
        let d = Dict::from_tsv_str(tsv, "test.tsv").unwrap();
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn from_tsv_str_errors_on_missing_column() {
        let tsv = "灰桜のみ\n";
        let err = Dict::from_tsv_str(tsv, "test.tsv").unwrap_err();
        assert!(matches!(err, FuriganaError::Tsv { line: 1, .. }));
    }

    #[test]
    fn from_tsv_str_last_wins() {
        let tsv = "灰桜\tハイザクラ\n灰桜\tカイオウ\n";
        let d = Dict::from_tsv_str(tsv, "test.tsv").unwrap();
        assert_eq!(d.lookup("灰桜"), Some("カイオウ"));
    }

    #[test]
    fn merge_overwrites() {
        let mut a = Dict::from_tsv_str("灰桜\tハイザクラ\n", "a.tsv").unwrap();
        let b = Dict::from_tsv_str("灰桜\tカイオウ\n黎明\tレイメイ\n", "b.tsv").unwrap();
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
    fn from_tsv_dir_loads_multiple_files() {
        let dir = fresh_temp_dir("dir_load");
        std::fs::write(dir.join("01_a.tsv"), "灰桜\tハイザクラ\n").unwrap();
        std::fs::write(dir.join("02_b.tsv"), "黎明\tレイメイ\n").unwrap();

        let d = Dict::from_tsv_dir(&dir).unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d.lookup("灰桜"), Some("ハイザクラ"));
        assert_eq!(d.lookup("黎明"), Some("レイメイ"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn from_tsv_dir_filename_order_decides_priority() {
        let dir = fresh_temp_dir("dir_priority");
        // 02_higher.tsv が後 → 上書き
        std::fs::write(dir.join("01_lower.tsv"), "灰桜\tハイザクラ\n").unwrap();
        std::fs::write(dir.join("02_higher.tsv"), "灰桜\tカイオウ\n").unwrap();

        let d = Dict::from_tsv_dir(&dir).unwrap();
        assert_eq!(d.lookup("灰桜"), Some("カイオウ"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn from_tsv_dir_missing_returns_empty() {
        let d = Dict::from_tsv_dir("/nonexistent/dir/path/xyz_furigana_test").unwrap();
        assert!(d.is_empty());
    }
}
