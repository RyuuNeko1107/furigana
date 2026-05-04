//! 埋め込みルールデータ
//!
//! `data/rules/` 配下のファイルを `include_str!` でビルド時に取り込み、
//! [`crate::Furigana::minimal`] や `rules_dir` 未指定の builder 構築時に
//! このデータを使う。

use crate::error::Result;
use crate::loader::{
    parse_compat_tsv, parse_context_toml, parse_counters_toml, parse_days_toml, parse_latin_tsv,
    parse_numeric_phrases_tsv, parse_scales_tsv, parse_symbols_tsv, parse_units_tsv,
};
use crate::rules::RulesData;

const COUNTERS: &str = include_str!("../../../data/rules/counters.toml");
const CONTEXT: &str = include_str!("../../../data/rules/context.toml");
const DAYS: &str = include_str!("../../../data/rules/days.toml");
const SCALES: &str = include_str!("../../../data/rules/scales.tsv");
const UNITS: &str = include_str!("../../../data/rules/units.tsv");
const SYMBOLS: &str = include_str!("../../../data/rules/symbols.tsv");
const LATIN: &str = include_str!("../../../data/rules/latin.tsv");
const NUMERIC_PHRASES: &str = include_str!("../../../data/rules/numeric_phrases.tsv");
const COMPAT: &str = include_str!("../../../data/rules/compat_map.tsv");

/// ビルド時に埋め込まれたルール群を [`RulesData`] として返す
///
/// # Errors
/// 埋め込みデータのパースに失敗した場合 (CI 通過済みなので通常は起きない)。
pub fn rules() -> Result<RulesData> {
    Ok(RulesData {
        counters: parse_counters_toml(COUNTERS, "embedded:counters.toml")?,
        context: parse_context_toml(CONTEXT, "embedded:context.toml")?,
        days: parse_days_toml(DAYS, "embedded:days.toml")?,
        scales: parse_scales_tsv(SCALES, "embedded:scales.tsv")?,
        units: parse_units_tsv(UNITS, "embedded:units.tsv")?,
        symbols: parse_symbols_tsv(SYMBOLS, "embedded:symbols.tsv")?,
        latin: parse_latin_tsv(LATIN, "embedded:latin.tsv")?,
        numeric_phrases: parse_numeric_phrases_tsv(
            NUMERIC_PHRASES,
            "embedded:numeric_phrases.tsv",
        )?,
        compat: parse_compat_tsv(COMPAT, "embedded:compat_map.tsv")?,
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
        assert_eq!(r.compat.lookup("髙"), Some("高"));
    }
}
