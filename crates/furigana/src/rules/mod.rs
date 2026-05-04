//! ルールデータ型定義
//!
//! 各 sub module は `data/rules/*.{toml,tsv}` 1 ファイルに対応する Rust 型を提供する。
//! ロード処理は別 module (Task #3) で実装される。
//!
//! 集約型 [`RulesData`] が全ルールをまとめて保持する。

pub mod compat;
pub mod context;
pub mod counters;
pub mod days;
pub mod latin;
pub mod numeric_phrases;
pub mod scales;
pub mod symbols;
pub mod units;

pub use compat::{CompatData, CompatEntry};
pub use context::{ContextData, ContextMatch, ContextRule};
pub use counters::{CounterMode, CounterRule, CountersData, EuphonicRule, KanaReplacement};
pub use days::DaysData;
pub use latin::{LatinData, LatinEntry};
pub use numeric_phrases::{NumericPhrase, NumericPhrasesData};
pub use scales::{ScaleEntry, ScalesData};
pub use symbols::{SymbolEntry, SymbolsData};
pub use units::{UnitEntry, UnitsData};

/// 全ルールを束ねるトップレベル構造体
#[derive(Debug, Default, Clone)]
pub struct RulesData {
    /// 助数詞 (counters.toml)
    pub counters: CountersData,
    /// 文脈依存読み (context.toml)
    pub context: ContextData,
    /// 1〜31 日の特殊読み (days.toml)
    pub days: DaysData,
    /// 大数スケール: 万/億/兆/京… (scales.tsv)
    pub scales: ScalesData,
    /// SI 単位 (units.tsv)
    pub units: UnitsData,
    /// 記号読み (symbols.tsv)
    pub symbols: SymbolsData,
    /// ラテン文字読み (latin.tsv)
    pub latin: LatinData,
    /// 例外語句 (numeric_phrases.tsv)
    pub numeric_phrases: NumericPhrasesData,
    /// 異体字マップ (compat_map.tsv)
    pub compat: CompatData,
}
