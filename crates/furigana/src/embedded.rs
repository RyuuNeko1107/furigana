//! 埋め込みルールデータ
//!
//! `data/rules/` 配下の TOML ファイルを `include_str!` でビルド時に取り込み、
//! [`crate::Furigana::minimal`] や `rules_dir` 未指定の builder 構築時に
//! このデータを使う。

use crate::error::Result;
use crate::loader::{
    parse_context_toml, parse_counters_toml, parse_days_toml, parse_latin_toml,
    parse_numeric_phrases_toml, parse_scales_toml, parse_symbols_toml, parse_units_toml,
};
use crate::rules::{CompatData, RulesData};

const COUNTERS: &str = include_str!("../../../data/rules/counters.toml");
const CONTEXT: &str = include_str!("../../../data/rules/context.toml");
const DAYS: &str = include_str!("../../../data/rules/days.toml");
const SCALES: &str = include_str!("../../../data/rules/scales.toml");
const UNITS: &str = include_str!("../../../data/rules/units.toml");
const SYMBOLS: &str = include_str!("../../../data/rules/symbols.toml");
const LATIN: &str = include_str!("../../../data/rules/latin.toml");
const NUMERIC_PHRASES: &str = include_str!("../../../data/rules/numeric_phrases.toml");

/// ビルド時に埋め込まれたルール群を [`RulesData`] として返す
///
/// 異体字マップ (`compat`) は本体に embed しない方針 (役割分離: 本体は engine
/// ルール、`furigana-dict` は語彙データ)。`Furigana::minimal()` 起動時の
/// `compat` は空。`furigana dict pull` で `furigana-dict` から取得するか、
/// builder の `core_dict_dir(...)` で独自パスを mount する。
///
/// # Errors
/// 埋め込みデータのパースに失敗した場合 (CI 通過済みなので通常は起きない)。
pub fn rules() -> Result<RulesData> {
    Ok(RulesData {
        counters: parse_counters_toml(COUNTERS, "embedded:counters.toml")?,
        context: parse_context_toml(CONTEXT, "embedded:context.toml")?,
        days: parse_days_toml(DAYS, "embedded:days.toml")?,
        scales: parse_scales_toml(SCALES, "embedded:scales.toml")?,
        units: parse_units_toml(UNITS, "embedded:units.toml")?,
        symbols: parse_symbols_toml(SYMBOLS, "embedded:symbols.toml")?,
        latin: parse_latin_toml(LATIN, "embedded:latin.toml")?,
        numeric_phrases: parse_numeric_phrases_toml(
            NUMERIC_PHRASES,
            "embedded:numeric_phrases.toml",
        )?,
        compat: CompatData::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_rules_parses_successfully() {
        let r = rules().expect("embedded rules parse failed");
        assert!(!r.counters.simple.is_empty());
        assert!(!r.counters.counter.is_empty());
        assert!(!r.context.rules.is_empty());
        assert_eq!(r.days.len(), 31);
        assert!(r.scales.lookup("万").is_some());
        assert_eq!(r.units.lookup("km"), Some("キロメートル"));
        assert_eq!(r.symbols.lookup("+"), Some("プラス"));
        assert_eq!(r.latin.lookup('A'), Some("エー"));
        assert_eq!(r.numeric_phrases.lookup("二十歳"), Some("ハタチ"));
        // compat は furigana-dict 側で管理するため embed には含めない
        assert!(r.compat.is_empty());
    }
}
