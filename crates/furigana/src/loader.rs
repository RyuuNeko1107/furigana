//! データローダー (TOML / TSV → [`RulesData`])
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
//! | `scales.tsv` | [`ScalesData`] |
//! | `units.tsv` | [`UnitsData`] |
//! | `symbols.tsv` | [`SymbolsData`] |
//! | `latin.tsv` | [`LatinData`] |
//! | `numeric_phrases.tsv` | [`NumericPhrasesData`] |
//! | `compat_map.tsv` | [`CompatData`] |
//!
//! 個別ファイルが存在しない場合はその型のデフォルト値が返る。
//! ただし [`load_rules_dir`] は **ディレクトリ自体が存在しない** 場合はエラーを返す。
//!
//! ## TSV 形式
//! - フィールド区切り: タブ (`\t`)
//! - 行コメント: 行頭が `#`
//! - 空行は無視
//! - `\r\n` (CRLF) も許容

use crate::error::{FuriganaError, Result};
use crate::rules::{
    CompatData, CompatEntry, ContextData, CountersData, DaysData, LatinData, LatinEntry,
    NumericPhrase, NumericPhrasesData, RulesData, ScaleEntry, ScalesData, SymbolEntry, SymbolsData,
    UnitEntry, UnitsData,
};
use std::collections::HashMap;
use std::path::Path;

// ─── ファイル名定数 ───────────────────────────────────────────────────────────

/// 助数詞ルール TOML
pub const COUNTERS_FILE: &str = "counters.toml";
/// 文脈ルール TOML
pub const CONTEXT_FILE: &str = "context.toml";
/// 日付特殊読み TOML
pub const DAYS_FILE: &str = "days.toml";
/// 大数スケール TSV
pub const SCALES_FILE: &str = "scales.tsv";
/// SI 単位 TSV
pub const UNITS_FILE: &str = "units.tsv";
/// 記号読み TSV
pub const SYMBOLS_FILE: &str = "symbols.tsv";
/// ラテン文字読み TSV
pub const LATIN_FILE: &str = "latin.tsv";
/// 慣用語句 TSV
pub const NUMERIC_PHRASES_FILE: &str = "numeric_phrases.tsv";
/// 異体字マップ TSV
pub const COMPAT_FILE: &str = "compat_map.tsv";

// ─── Generic helpers ─────────────────────────────────────────────────────────

fn parse_toml<T: serde::de::DeserializeOwned>(content: &str, file: &str) -> Result<T> {
    toml::from_str(content).map_err(|e| FuriganaError::Toml {
        file: file.to_string(),
        source: e,
    })
}

/// TSV を行ごとに走査し、各行を `parse_line` で T に変換する。
///
/// - 行コメント (`#` 始まり) と空行はスキップ
/// - `parse_line` が `Err(msg)` を返した場合、行番号付きで [`FuriganaError::Tsv`] にラップ
fn parse_tsv_lines<T, F>(content: &str, file: &str, mut parse_line: F) -> Result<Vec<T>>
where
    F: FnMut(&[&str]) -> std::result::Result<T, String>,
{
    let mut out = Vec::new();
    for (idx, raw) in content.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        match parse_line(&cols) {
            Ok(t) => out.push(t),
            Err(msg) => {
                return Err(FuriganaError::Tsv {
                    file: file.to_string(),
                    line: idx + 1,
                    message: msg,
                });
            }
        }
    }
    Ok(out)
}

// ─── 個別 TOML パーサ ────────────────────────────────────────────────────────

/// counters.toml の文字列を解釈
pub fn parse_counters_toml(content: &str, file: &str) -> Result<CountersData> {
    parse_toml(content, file)
}

/// context.toml の文字列を解釈
pub fn parse_context_toml(content: &str, file: &str) -> Result<ContextData> {
    parse_toml(content, file)
}

/// days.toml の文字列を解釈
pub fn parse_days_toml(content: &str, file: &str) -> Result<DaysData> {
    parse_toml(content, file)
}

// ─── 個別 TSV パーサ ─────────────────────────────────────────────────────────

/// scales.tsv の文字列を解釈
pub fn parse_scales_tsv(content: &str, file: &str) -> Result<ScalesData> {
    let entries = parse_tsv_lines(content, file, |cols| {
        if cols.len() < 2 {
            return Err(format!(
                "expected 2 columns (kanji\\tkana), got {}",
                cols.len()
            ));
        }
        let kanji = cols[0].trim();
        let kana = cols[1].trim();
        if kanji.is_empty() || kana.is_empty() {
            return Err("kanji or kana column is empty".to_string());
        }
        Ok(ScaleEntry {
            kanji: kanji.to_string(),
            kana: kana.to_string(),
        })
    })?;
    Ok(ScalesData { entries })
}

/// units.tsv の文字列を解釈
pub fn parse_units_tsv(content: &str, file: &str) -> Result<UnitsData> {
    let entries = parse_tsv_lines(content, file, |cols| {
        if cols.len() < 2 {
            return Err(format!(
                "expected 2-3 columns (symbol\\tkana[\\tflag]), got {}",
                cols.len()
            ));
        }
        let symbol = cols[0].trim();
        let kana = cols[1].trim();
        if symbol.is_empty() || kana.is_empty() {
            return Err("symbol or kana column is empty".to_string());
        }
        let case_insensitive = cols.get(2).is_some_and(|f| f.trim() == "ci");
        Ok(UnitEntry {
            symbol: symbol.to_string(),
            kana: kana.to_string(),
            case_insensitive,
        })
    })?;
    Ok(UnitsData { entries })
}

/// symbols.tsv の文字列を解釈
pub fn parse_symbols_tsv(content: &str, file: &str) -> Result<SymbolsData> {
    let entries = parse_tsv_lines(content, file, |cols| {
        if cols.len() < 2 {
            return Err(format!(
                "expected 2 columns (symbol\\tkana), got {}",
                cols.len()
            ));
        }
        let symbol = cols[0].trim();
        let kana = cols[1].trim();
        if symbol.is_empty() || kana.is_empty() {
            return Err("symbol or kana column is empty".to_string());
        }
        Ok(SymbolEntry {
            symbol: symbol.to_string(),
            kana: kana.to_string(),
        })
    })?;
    Ok(SymbolsData { entries })
}

/// latin.tsv の文字列を解釈
pub fn parse_latin_tsv(content: &str, file: &str) -> Result<LatinData> {
    let entries = parse_tsv_lines(content, file, |cols| {
        if cols.len() < 2 {
            return Err(format!(
                "expected 2 columns (letter\\tkana), got {}",
                cols.len()
            ));
        }
        let letter = cols[0].trim();
        let kana = cols[1].trim();
        if letter.is_empty() || kana.is_empty() {
            return Err("letter or kana column is empty".to_string());
        }
        Ok(LatinEntry {
            letter: letter.to_string(),
            kana: kana.to_string(),
        })
    })?;
    Ok(LatinData { entries })
}

/// numeric_phrases.tsv の文字列を解釈
pub fn parse_numeric_phrases_tsv(content: &str, file: &str) -> Result<NumericPhrasesData> {
    let entries = parse_tsv_lines(content, file, |cols| {
        if cols.len() < 2 {
            return Err(format!(
                "expected 2 columns (surface\\tkana), got {}",
                cols.len()
            ));
        }
        let surface = cols[0].trim();
        let kana = cols[1].trim();
        if surface.is_empty() || kana.is_empty() {
            return Err("surface or kana column is empty".to_string());
        }
        Ok(NumericPhrase {
            surface: surface.to_string(),
            kana: kana.to_string(),
        })
    })?;
    Ok(NumericPhrasesData { entries })
}

/// compat_map.tsv の文字列を解釈 (`map` フィールドも自動再構築)
pub fn parse_compat_tsv(content: &str, file: &str) -> Result<CompatData> {
    let entries = parse_tsv_lines(content, file, |cols| {
        if cols.len() < 2 {
            return Err(format!(
                "expected 2 columns (variant\\tcanonical), got {}",
                cols.len()
            ));
        }
        let variant = cols[0].trim();
        let canonical = cols[1].trim();
        if variant.is_empty() || canonical.is_empty() {
            return Err("variant or canonical column is empty".to_string());
        }
        Ok(CompatEntry {
            variant: variant.to_string(),
            canonical: canonical.to_string(),
        })
    })?;
    let mut data = CompatData {
        entries,
        map: HashMap::new(),
    };
    data.rebuild_map();
    Ok(data)
}

// ─── ファイル読み込み (default fallback 付き) ────────────────────────────────

fn read_or_default<T, F>(path: &Path, parser: F) -> Result<T>
where
    T: Default,
    F: FnOnce(&str, &str) -> Result<T>,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let content = std::fs::read_to_string(path)?;
    parser(&content, &path.display().to_string())
}

// ─── 個別ファイルローダー ────────────────────────────────────────────────────

/// counters.toml をファイルから読み込む (存在しなければ default)
pub fn load_counters(path: impl AsRef<Path>) -> Result<CountersData> {
    read_or_default(path.as_ref(), parse_counters_toml)
}

/// context.toml をファイルから読み込む (存在しなければ default)
pub fn load_context(path: impl AsRef<Path>) -> Result<ContextData> {
    read_or_default(path.as_ref(), parse_context_toml)
}

/// days.toml をファイルから読み込む (存在しなければ default)
pub fn load_days(path: impl AsRef<Path>) -> Result<DaysData> {
    read_or_default(path.as_ref(), parse_days_toml)
}

/// scales.tsv をファイルから読み込む (存在しなければ default)
pub fn load_scales(path: impl AsRef<Path>) -> Result<ScalesData> {
    read_or_default(path.as_ref(), parse_scales_tsv)
}

/// units.tsv をファイルから読み込む (存在しなければ default)
pub fn load_units(path: impl AsRef<Path>) -> Result<UnitsData> {
    read_or_default(path.as_ref(), parse_units_tsv)
}

/// symbols.tsv をファイルから読み込む (存在しなければ default)
pub fn load_symbols(path: impl AsRef<Path>) -> Result<SymbolsData> {
    read_or_default(path.as_ref(), parse_symbols_tsv)
}

/// latin.tsv をファイルから読み込む (存在しなければ default)
pub fn load_latin(path: impl AsRef<Path>) -> Result<LatinData> {
    read_or_default(path.as_ref(), parse_latin_tsv)
}

/// numeric_phrases.tsv をファイルから読み込む (存在しなければ default)
pub fn load_numeric_phrases(path: impl AsRef<Path>) -> Result<NumericPhrasesData> {
    read_or_default(path.as_ref(), parse_numeric_phrases_tsv)
}

/// compat_map.tsv をファイルから読み込む (存在しなければ default)
pub fn load_compat(path: impl AsRef<Path>) -> Result<CompatData> {
    read_or_default(path.as_ref(), parse_compat_tsv)
}

// ─── ディレクトリ全体 ────────────────────────────────────────────────────────

/// 指定ディレクトリから全ルールファイルを読み込んで [`RulesData`] を構築する。
///
/// - **ディレクトリ自体が存在しない**: [`FuriganaError::Validation`] でエラー
/// - **個別ファイルが存在しない**: その型のデフォルト値で埋める
/// - **個別ファイルがパース失敗**: そのファイル名・行番号付きでエラー
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
        counters: load_counters(dir.join(COUNTERS_FILE))?,
        context: load_context(dir.join(CONTEXT_FILE))?,
        days: load_days(dir.join(DAYS_FILE))?,
        scales: load_scales(dir.join(SCALES_FILE))?,
        units: load_units(dir.join(UNITS_FILE))?,
        symbols: load_symbols(dir.join(SYMBOLS_FILE))?,
        latin: load_latin(dir.join(LATIN_FILE))?,
        numeric_phrases: load_numeric_phrases(dir.join(NUMERIC_PHRASES_FILE))?,
        compat: load_compat(dir.join(COMPAT_FILE))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── TSV: scales ────────────────────────────────────────────────────────

    #[test]
    fn parses_scales_tsv_basic() {
        let tsv = "万\tマン\n億\tオク\n兆\tチョウ\n";
        let data = parse_scales_tsv(tsv, "test.tsv").unwrap();
        assert_eq!(data.len(), 3);
        assert_eq!(data.lookup("万"), Some("マン"));
        assert_eq!(data.lookup("兆"), Some("チョウ"));
    }

    #[test]
    fn parses_scales_tsv_skips_comments_and_blanks() {
        let tsv = "# 大数スケール\n\n万\tマン\n\n# コメント\n億\tオク\n";
        let data = parse_scales_tsv(tsv, "test.tsv").unwrap();
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn parses_scales_tsv_handles_crlf() {
        let tsv = "万\tマン\r\n億\tオク\r\n";
        let data = parse_scales_tsv(tsv, "test.tsv").unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data.lookup("億"), Some("オク"));
    }

    #[test]
    fn tsv_too_few_columns_errors_with_line_number() {
        let tsv = "万\tマン\n壊れた行\n";
        let err = parse_scales_tsv(tsv, "test.tsv").unwrap_err();
        match err {
            FuriganaError::Tsv { line, file, .. } => {
                assert_eq!(line, 2);
                assert_eq!(file, "test.tsv");
            }
            other => panic!("expected Tsv error, got {other:?}"),
        }
    }

    #[test]
    fn tsv_empty_field_errors() {
        let tsv = "万\t\n";
        let err = parse_scales_tsv(tsv, "test.tsv").unwrap_err();
        assert!(matches!(err, FuriganaError::Tsv { .. }));
    }

    // ─── TSV: units ─────────────────────────────────────────────────────────

    #[test]
    fn parses_units_tsv_with_ci_flag() {
        let tsv = "km\tキロメートル\nL\tリットル\tci\nmL\tミリリットル\tci\n";
        let data = parse_units_tsv(tsv, "test.tsv").unwrap();
        assert_eq!(data.len(), 3);
        assert_eq!(data.lookup("km"), Some("キロメートル"));
        assert_eq!(data.lookup("l"), Some("リットル"));
        assert_eq!(data.lookup("ML"), Some("ミリリットル"));
        assert_eq!(data.lookup("KM"), None); // ci フラグなしなので大文字小文字区別
    }

    #[test]
    fn parses_units_tsv_unknown_flag_treated_as_default() {
        let tsv = "km\tキロメートル\tunknown_flag\n";
        let data = parse_units_tsv(tsv, "test.tsv").unwrap();
        assert!(!data.entries[0].case_insensitive);
    }

    // ─── TSV: compat ────────────────────────────────────────────────────────

    #[test]
    fn parses_compat_tsv_and_rebuilds_map() {
        let tsv = "髙\t高\n﨑\t崎\n德\t徳\n";
        let data = parse_compat_tsv(tsv, "compat.tsv").unwrap();
        assert_eq!(data.lookup("髙"), Some("高"));
        assert_eq!(data.lookup("﨑"), Some("崎"));
        assert_eq!(data.lookup("德"), Some("徳"));
        assert_eq!(data.lookup("高"), None); // 逆引きはしない
    }

    // ─── TOML ───────────────────────────────────────────────────────────────

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
        let data = parse_counters_toml(toml_str, "counters.toml").unwrap();
        let hon = data.counter.get("本").unwrap();
        assert_eq!(hon.default.as_deref(), Some("ホン"));
        assert_eq!(hon.rules[0].suffix, "ポン");
    }

    #[test]
    fn invalid_toml_error_includes_file_name() {
        let err = parse_counters_toml("[invalid", "counters.toml").unwrap_err();
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
        let data = parse_days_toml(toml_str, "days.toml").unwrap();
        assert_eq!(data.get(1), Some("ツイタチ"));
        assert_eq!(data.get(20), Some("ハツカ"));
        assert_eq!(data.get(15), None);
    }

    // ─── ディレクトリローダー ────────────────────────────────────────────────

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
        // 念のため
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
        std::fs::write(dir.join(SCALES_FILE), "万\tマン\n億\tオク\n").unwrap();
        std::fs::write(
            dir.join(COUNTERS_FILE),
            "[counter.\"本\"]\ndefault = \"ホン\"\n",
        )
        .unwrap();
        std::fs::write(dir.join(COMPAT_FILE), "髙\t高\n").unwrap();

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
        // 他は default のまま
        assert!(data.context.rules.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_rules_dir_propagates_parse_errors() {
        let dir = fresh_temp_dir("parse_err");
        std::fs::write(dir.join(SCALES_FILE), "壊れた\n").unwrap();
        let err = load_rules_dir(&dir).unwrap_err();
        match err {
            FuriganaError::Tsv { line, .. } => assert_eq!(line, 1),
            other => panic!("expected Tsv error, got {other:?}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
