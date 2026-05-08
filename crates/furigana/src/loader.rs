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
//! ### 細分化サポート (counters / context)
//!
//! 大きくなりがちな counters.toml / context.toml は、それぞれ
//! `counters/*.toml` / `context/*.toml` のサブディレクトリに分割して
//! 配置することもできる。サブディレクトリ配下の `*.toml` はファイル名
//! ソート順で読み込まれ、[`CountersData::merge`] / [`ContextData::merge`]
//! で 1 つにまとめられる。
//!
//! 単一ファイル `counters.toml` とサブディレクトリ `counters/` が両方
//! ある場合は単一ファイルが優先される (混在防止のため、いずれか一方に
//! 統一することを推奨)。
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
use crate::rules::{ContextData, CountersData, PostProcessData, PostProcessSpec, RulesData};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::path::{Path, PathBuf};

// ─── [meta] role tag の取得 ──────────────────────────────────────────────────
//
// 各 dict / rule TOML の冒頭に `[meta] role = "..."` を declare できる。
// lib loader はこれを見て role 別に dispatch する。 role tag 無い file は
// file 名から推定 (`infer_role_from_path`) して backwards compat 維持。

#[derive(Deserialize, Default)]
struct MetaWrapper {
    #[serde(default)]
    meta: Option<MetaTag>,
}

#[derive(Deserialize, Default)]
struct MetaTag {
    #[serde(default)]
    role: Option<String>,
}

/// TOML 内の `[meta] role` を返す (無ければ None)。 失敗は None 扱い。
#[must_use]
pub fn parse_meta_role(content: &str) -> Option<String> {
    toml::from_str::<MetaWrapper>(content)
        .ok()
        .and_then(|w| w.meta)
        .and_then(|m| m.role)
}

/// file path から role を推定 (role tag 無い file の互換 fallback)。
///
/// 既知の hardcoded path / 名 から「これは何の role か」 を推定する:
/// - `<dir>/days.toml` → "days"
/// - `<dir>/scales.toml` → "scales"
/// - `<dir>/counters.toml` or `<dir>/counters/*.toml` → "counters"
/// - `<dir>/context.toml` or `<dir>/context/*.toml` → "context"
/// - 等
///
/// 不明 path は None。
fn infer_role_from_path(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_str()?;
    // subdir 親で counters / context を識別
    if let Some(parent) = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
    {
        match parent {
            "counters" => return Some("counters"),
            "context" => return Some("context"),
            _ => {}
        }
    }
    match name {
        "counters.toml" => Some("counters"),
        "context.toml" => Some("context"),
        "days.toml" => Some("days"),
        "scales.toml" => Some("scales"),
        "units.toml" => Some("units"),
        "symbols.toml" => Some("symbols"),
        "latin.toml" => Some("latin"),
        "numeric_phrases.toml" => Some("numeric_phrases"),
        "compat.toml" | "compat_map.toml" => Some("compat"),
        "postprocess.toml" => Some("postprocess"),
        _ => None,
    }
}

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
/// 異体字マップ。 配布物 (ja-furigana-dict release tar) 内のファイル名と一致させる。
/// 旧 alpha 系で「`compat_map.toml`」 にしていた時代があったが、 dict 側は当初
/// から `core/compat.toml` で配布しており、 lib が探していたファイル名と乖離して
/// **異体字正規化が無効化されていた** (reading::tokenize_text Step 1 が no-op
/// になり、 「髙橋」 / 「檜風呂」 等が compat 経由で標準字に変換されないまま
/// chunker / Lindera に流れていた)。 R15 で corpus に「檜風呂に入る」 が追加され
/// るまで気付かれなかった構造的 bug の修正。
pub const COMPAT_FILE: &str = "compat.toml";
/// 後処理ルール (Step 7 (mode 別後処理 regex))
pub const POSTPROCESS_FILE: &str = "postprocess.toml";

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

    // 全 *.toml を再帰 walk して、 各 file の `[meta] role` で振り分け。
    // role tag 無い file は file 名から推定 (`infer_role_from_path`)。
    // これで rule file は dir 構造に依存せず配置自由 (例
    // `rules/text/postprocess.toml` のような階層化が可能)。
    let mut data = RulesData::default();
    let mut postprocess_specs: Vec<(PathBuf, PostProcessSpec)> = Vec::new();
    for path in walk_rule_files(dir)? {
        let content = std::fs::read_to_string(&path)?;
        let role =
            parse_meta_role(&content).or_else(|| infer_role_from_path(&path).map(String::from));
        let role_str = role.as_deref();
        let from = path.display().to_string();
        match role_str {
            Some("counters") => {
                let part: CountersData = parse_toml(&content, &from)?;
                data.counters.merge(part);
            }
            Some("context") => {
                let part: ContextData = parse_toml(&content, &from)?;
                data.context.merge(part);
            }
            Some("days") => {
                data.days = parse_toml(&content, &from)?;
            }
            Some("scales") => {
                data.scales = parse_toml(&content, &from)?;
            }
            Some("units") => {
                data.units = parse_toml(&content, &from)?;
            }
            Some("symbols") => {
                data.symbols = parse_toml(&content, &from)?;
            }
            Some("latin") => {
                data.latin = parse_toml(&content, &from)?;
            }
            Some("numeric_phrases") => {
                data.numeric_phrases = parse_toml(&content, &from)?;
            }
            Some("compat") => {
                data.compat = parse_toml(&content, &from)?;
            }
            Some("postprocess") => {
                // regex compile は最後にまとめて (複数 file を merge した後)
                let spec: PostProcessSpec = parse_toml(&content, &from)?;
                postprocess_specs.push((path.clone(), spec));
            }
            _ => {
                // role 不明 / 認識外 / dict 系 (jukugo/unihan/works/loanwords/
                // single_overrides) は rules には不要、 silent skip。
            }
        }
    }
    // postprocess を merge して compile
    let mut merged_spec = PostProcessSpec::default();
    for (_path, spec) in postprocess_specs {
        merged_spec.rules.extend(spec.rules);
    }
    data.postprocess = PostProcessData::from_spec(merged_spec)
        .map_err(|e| FuriganaError::Validation(format!("postprocess regex compile failed: {e}")))?;
    Ok(data)
}

/// `dir` 配下の rules 用 *.toml を再帰 walk して列挙。
///
/// `_genre.toml` (STATS sub-section meta) と `*.test.toml` (CI 専用) は除外。
/// 同じ subdir に dict 系 file (jukugo / unihan / works / loanwords /
/// single_overrides) が混在しても、 それらは role 不明 / dict role として
/// `load_rules_dir` 内で silent skip される (= rules に取り込まれない)。
fn walk_rule_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_rule_files_inner(dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk_rule_files_inner(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "toml") {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "_genre.toml" || name.ends_with(".test.toml") {
                continue;
            }
            out.push(path);
        } else if path.is_dir() {
            walk_rule_files_inner(&path, out)?;
        }
    }
    Ok(())
}

// (旧 load_counters / load_context / load_postprocess / list_toml_files_sorted は
// load_rules_dir の再帰 walk + role 駆動 dispatch に統合された。
// hardcoded path / file 名定数 は infer_role_from_path の fallback で使われる。)

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

    #[test]
    fn load_rules_dir_merges_counters_subdir() {
        let dir = fresh_temp_dir("counters_subdir");
        let sub = dir.join("counters");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("01_simple.toml"), "[simple]\n\"円\" = \"エン\"\n").unwrap();
        std::fs::write(
            sub.join("02_objects.toml"),
            "[counter.\"本\"]\ndefault = \"ホン\"\n",
        )
        .unwrap();
        std::fs::write(
            sub.join("03_time.toml"),
            "[counter.\"時\"]\ndefault = \"ジ\"\n",
        )
        .unwrap();

        let data = load_rules_dir(&dir).unwrap();
        assert_eq!(
            data.counters.simple.get("円").map(String::as_str),
            Some("エン")
        );
        assert_eq!(
            data.counters
                .counter
                .get("本")
                .and_then(|c| c.default.as_deref()),
            Some("ホン")
        );
        assert_eq!(
            data.counters
                .counter
                .get("時")
                .and_then(|c| c.default.as_deref()),
            Some("ジ")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_rules_dir_merges_context_subdir_in_filename_order() {
        let dir = fresh_temp_dir("context_subdir");
        let sub = dir.join("context");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("01_a.toml"),
            "[[rule]]\nsurface = \"一日\"\ndefault = \"イチニチ\"\n",
        )
        .unwrap();
        std::fs::write(
            sub.join("02_b.toml"),
            "[[rule]]\nsurface = \"二日\"\ndefault = \"フツカ\"\n",
        )
        .unwrap();

        let data = load_rules_dir(&dir).unwrap();
        assert_eq!(data.context.rules.len(), 2);
        assert_eq!(data.context.rules[0].surface, "一日");
        assert_eq!(data.context.rules[1].surface, "二日");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_rules_dir_merges_single_file_and_subdir() {
        // 新 loader (role 駆動 + 再帰 walk) は単一 file と subdir 配下を **両方 merge**
        // する。 旧 loader では「単一 file 優先で subdir は ignore」 だったが、
        // role tag 駆動なら同 role の file が複数 dir に存在しても自然に merge できる。
        let dir = fresh_temp_dir("counters_merge");
        let sub = dir.join("counters");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            dir.join(COUNTERS_FILE),
            "[counter.\"本\"]\ndefault = \"ホン\"\n",
        )
        .unwrap();
        std::fs::write(
            sub.join("more.toml"),
            "[counter.\"匹\"]\ndefault = \"ヒキ\"\n",
        )
        .unwrap();

        let data = load_rules_dir(&dir).unwrap();
        assert!(data.counters.counter.contains_key("本"));
        assert!(data.counters.counter.contains_key("匹"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_rules_dir_role_tag_overrides_filename() {
        // file 名から推定できない場所に置いても [meta] role があれば認識される。
        let dir = fresh_temp_dir("role_tag_override");
        std::fs::create_dir_all(dir.join("custom")).unwrap();
        std::fs::write(
            dir.join("custom").join("anywhere.toml"),
            "[meta]\nrole = \"counters\"\n\n[counter.\"枚\"]\ndefault = \"マイ\"\n",
        )
        .unwrap();

        let data = load_rules_dir(&dir).unwrap();
        assert!(data.counters.counter.contains_key("枚"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
