//! データローダー (TOML → [`RulesData`])
//!
//! ## ファイル名規約
//!
//! ディレクトリ単位で読み込む場合、以下のファイル名で配置する:
//!
//! | ファイル名 | 型 |
//! |---|---|
//! | `counters.toml` | [`CountersData`] |
//! | `context.toml` | [`ContextData`] |
//! | `days.toml` | [`DaysData`] |
//! | `scales.toml` | [`ScalesData`] |
//! | `units.toml` | [`UnitsData`] |
//! | `symbols.toml` | [`SymbolsData`] |
//! | `latin.toml` | [`LatinData`] |
//! | `numeric_phrases.toml` | [`NumericPhrasesData`] |
//! | `compat_map.toml` | [`CompatData`] |
//!
//! 個別ファイルが存在しない場合はその型のデフォルト値が返る。
//! ただし [`load_rules_dir`] は **ディレクトリ自体が存在しない** 場合はエラーを返す。
//!
//! ## API
//!
//! 単一ファイル系は generic で 1 つに集約してある:
//! - [`parse_toml`]      : 文字列 → 任意の `Deserialize` 型
//! - [`load_or_default`] : ファイル → 任意の `Deserialize + Default` 型
//!   (ファイルが無ければ `Default::default()` を返す)
//!
//! ディレクトリ全体を読み込む高レベル API は [`load_rules_dir`]。

use crate::error::{FuriganaError, Result};
use crate::rules::RulesData;
use serde::de::DeserializeOwned;
use std::path::Path;

// ─── ファイル名定数 ───────────────────────────────────────────────────────────

/// 助数詞ルール
pub const COUNTERS_FILE: &str = "counters.toml";
/// 文脈ルール
pub const CONTEXT_FILE: &str = "context.toml";
/// 日付特殊読み
pub const DAYS_FILE: &str = "days.toml";
/// 大数スケール
pub const SCALES_FILE: &str = "scales.toml";
/// SI 単位
pub const UNITS_FILE: &str = "units.toml";
/// 記号読み
pub const SYMBOLS_FILE: &str = "symbols.toml";
/// ラテン文字読み
pub const LATIN_FILE: &str = "latin.toml";
/// 慣用語句
pub const NUMERIC_PHRASES_FILE: &str = "numeric_phrases.toml";
/// 異体字マップ
pub const COMPAT_FILE: &str = "compat_map.toml";

// ─── 汎用 parse / load ──────────────────────────────────────────────────────

/// TOML 文字列を任意の型にパース
///
/// 失敗時は `file` をエラーメッセージに含めた [`FuriganaError::Toml`] を返す。
///
/// ```no_run
/// use furigana::loader::parse_toml;
/// use furigana::rules::CountersData;
///
/// let data: CountersData = parse_toml(r#"[simple]"#, "counters.toml").unwrap();
/// ```
pub fn parse_toml<T: DeserializeOwned>(content: &str, file: &str) -> Result<T> {
    toml::from_str(content).map_err(|e| FuriganaError::Toml {
        file: file.to_string(),
        source: e,
    })
}

/// ファイルから TOML を読み込む (存在しなければ `Default::default()` を返す)
///
/// ```no_run
/// use furigana::loader::load_or_default;
/// use furigana::rules::CountersData;
///
/// let data: CountersData = load_or_default("path/to/counters.toml").unwrap();
/// ```
pub fn load_or_default<T: DeserializeOwned + Default>(path: impl AsRef<Path>) -> Result<T> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(T::default());
    }
    let content = std::fs::read_to_string(path)?;
    parse_toml(&content, &path.display().to_string())
}

// ─── ディレクトリ全体 ────────────────────────────────────────────────────────

/// 指定ディレクトリから全ルールファイルを読み込んで [`RulesData`] を構築する。
///
/// - **ディレクトリ自体が存在しない**: [`FuriganaError::Validation`] でエラー
/// - **個別ファイルが存在しない**: その型のデフォルト値で埋める
/// - **個別ファイルがパース失敗**: そのファイル名付きでエラー
pub fn load_rules_dir<P: AsRef<Path>>(dir: P) -> Result<RulesData> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Err(FuriganaError::Validation(format!(
            "rules directory not found: {}",
            dir.display()
        )));
    }
    if !dir.is_dir() {
        return Err(FuriganaError::Validation(format!(
            "rules path is not a directory: {}",
            dir.display()
        )));
    }

    Ok(RulesData {
        counters: load_or_default(dir.join(COUNTERS_FILE))?,
        context: load_or_default(dir.join(CONTEXT_FILE))?,
        days: load_or_default(dir.join(DAYS_FILE))?,
        scales: load_or_default(dir.join(SCALES_FILE))?,
        units: load_or_default(dir.join(UNITS_FILE))?,
        symbols: load_or_default(dir.join(SYMBOLS_FILE))?,
        latin: load_or_default(dir.join(LATIN_FILE))?,
        numeric_phrases: load_or_default(dir.join(NUMERIC_PHRASES_FILE))?,
        compat: load_or_default(dir.join(COMPAT_FILE))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::CountersData;

    #[test]
    fn parses_counters_toml() {
        let toml_str = r#"
            [counter."本"]
            default = "ホン"

            [[counter."本".rules]]
            last_digit = [1, 6, 8, 0]
            suffix = "ポン"
            sokuonize = true
        "#;
        let data: CountersData = parse_toml(toml_str, "counters.toml").unwrap();
        let hon = data.counter.get("本").unwrap();
        assert_eq!(hon.default.as_deref(), Some("ホン"));
        assert_eq!(hon.rules[0].suffix, "ポン");
    }

    #[test]
    fn invalid_toml_error_includes_file_name() {
        let err = parse_toml::<CountersData>("[invalid", "counters.toml").unwrap_err();
        match err {
            FuriganaError::Toml { file, .. } => assert_eq!(file, "counters.toml"),
            other => panic!("expected Toml error, got {other:?}"),
        }
    }

    #[test]
    fn parses_days_toml_lookup_by_int() {
        let toml_str = r#"
            "1" = "ツイタチ"
            "20" = "ハツカ"
        "#;
        let data: crate::rules::DaysData = parse_toml(toml_str, "days.toml").unwrap();
        assert_eq!(data.get(1), Some("ツイタチ"));
        assert_eq!(data.get(20), Some("ハツカ"));
        assert_eq!(data.get(15), None);
    }

    #[test]
    fn parses_scales_toml_with_array_of_tables() {
        let toml_str = r#"
            [[entry]]
            kanji = "万"
            kana = "マン"
            [[entry]]
            kanji = "億"
            kana = "オク"
        "#;
        let data: crate::rules::ScalesData = parse_toml(toml_str, "scales.toml").unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data.lookup("億"), Some("オク"));
    }

    #[test]
    fn parses_units_toml_with_inline_tables() {
        let toml_str = r#"
            [entries]
            "km" = { kana = "キロメートル" }
            "L"  = { kana = "リットル", ci = true }
        "#;
        let data: crate::rules::UnitsData = parse_toml(toml_str, "units.toml").unwrap();
        assert_eq!(data.lookup("km"), Some("キロメートル"));
        assert_eq!(data.lookup("l"), Some("リットル"));
    }

    fn fresh_temp_dir(suffix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "furigana_loader_{}_{}_{}",
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
    fn load_rules_dir_errors_when_dir_missing() {
        let nonexistent = std::env::temp_dir().join("furigana_does_not_exist_xyz123");
        std::fs::remove_dir_all(&nonexistent).ok();
        let err = load_rules_dir(&nonexistent).unwrap_err();
        assert!(matches!(err, FuriganaError::Validation(_)));
    }

    #[test]
    fn load_rules_dir_with_no_files_yields_default() {
        let dir = fresh_temp_dir("empty");
        let data = load_rules_dir(&dir).unwrap();
        assert!(data.counters.simple.is_empty());
        assert!(data.scales.is_empty());
        assert!(data.context.rules.is_empty());
        assert!(data.compat.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_rules_dir_loads_files_when_present() {
        let dir = fresh_temp_dir("present");
        std::fs::write(
            dir.join(SCALES_FILE),
            "[[entry]]\nkanji = \"万\"\nkana = \"マン\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join(COUNTERS_FILE),
            "[counter.\"本\"]\ndefault = \"ホン\"\n",
        )
        .unwrap();
        std::fs::write(dir.join(COMPAT_FILE), "[map]\n\"髙\" = \"高\"\n").unwrap();

        let data = load_rules_dir(&dir).unwrap();
        assert_eq!(data.scales.lookup("万"), Some("マン"));
        assert_eq!(
            data.counters
                .counter
                .get("本")
                .and_then(|c| c.default.as_deref()),
            Some("ホン")
        );
        assert_eq!(data.compat.lookup("髙"), Some("高"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_rules_dir_propagates_parse_errors() {
        let dir = fresh_temp_dir("parse_err");
        std::fs::write(dir.join(COUNTERS_FILE), "壊れた").unwrap();
        let err = load_rules_dir(&dir).unwrap_err();
        assert!(matches!(err, FuriganaError::Toml { .. }));
        std::fs::remove_dir_all(&dir).ok();
    }
}
