//! 埋め込みルールデータ
//!
//! 本体バイナリに rules / lookup データは embed しない方針 (バイナリ肥大化を避ける + 役割分離)。
//! [`Furigana::minimal`](crate::Furigana::minimal) は空の [`RulesData`] で起動する。
//!
//! 実データは [`furigana-dict`](https://github.com/RyuuNeko1107/furigana-dict)
//! リポジトリで配布され、`furigana dict pull` で取得 → builder の
//! `core_dict_dir(...)` / `rules_dir(...)` で mount する想定。

use crate::error::Result;
use crate::rules::RulesData;

/// 空の [`RulesData`] を返す
///
/// すべての rules / lookup が空なので、`Furigana::minimal()` 単独では
/// 形態素解析 (Lindera) と直接 dict 投入のみで動作する。
/// 助数詞・文脈・スケール等の高度な処理は dict pull で外部データを
/// 取り込んでから有効化される。
///
/// # Errors
/// 現在のところ常に Ok。将来 lazy validation を入れる時のためのシグネチャ。
pub fn rules() -> Result<RulesData> {
    Ok(RulesData::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_returns_empty_default() {
        let r = rules().expect("default rules");
        assert!(r.counters.simple.is_empty());
        assert!(r.counters.counter.is_empty());
        assert!(r.context.rules.is_empty());
        assert_eq!(r.days.len(), 0);
        assert!(r.scales.is_empty());
        assert!(r.units.is_empty());
        assert!(r.symbols.is_empty());
        assert!(r.latin.is_empty());
        assert!(r.numeric_phrases.is_empty());
        assert!(r.compat.is_empty());
    }
}
