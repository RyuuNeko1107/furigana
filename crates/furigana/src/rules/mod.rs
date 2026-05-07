//! ルールデータ型定義
//!
//! 各 sub module は `data/rules/*.toml` 1 ファイルに対応する Rust 型を提供する。
//! ロード処理は [`crate::loader`] で実装される。
//!
//! 集約型 [`RulesData`] が全ルールをまとめて保持する。

pub mod compat;
pub mod context;
pub mod counters;
pub mod days;
pub mod latin;
pub mod numeric_phrases;
pub mod postprocess;
pub mod scales;
pub mod symbols;
pub mod units;

pub use compat::CompatData;
pub use context::{ContextData, ContextMatch, ContextRule};
pub use counters::{CounterMode, CounterRule, CountersData, EuphonicRule, KanaReplacement};
pub use days::DaysData;
pub use latin::LatinData;
pub use numeric_phrases::NumericPhrasesData;
pub use postprocess::{PostProcessData, PostProcessRuleSpec, PostProcessSpec};
pub use scales::{ScaleEntry, ScalesData};
pub use symbols::SymbolsData;
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
    /// 大数スケール: 万/億/兆/京… (scales.toml)
    pub scales: ScalesData,
    /// SI 単位 (units.toml)
    pub units: UnitsData,
    /// 記号読み (symbols.toml)
    pub symbols: SymbolsData,
    /// ラテン文字読み (latin.toml)
    pub latin: LatinData,
    /// 例外語句 (numeric_phrases.toml)
    pub numeric_phrases: NumericPhrasesData,
    /// 異体字マップ (compat_map.toml)
    pub compat: CompatData,
    /// 出力後処理 (postprocess.toml、Step 7 (mode 別後処理 regex))
    pub postprocess: PostProcessData,
}
