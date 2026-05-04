//! 数値処理 (data-driven)
//!
//! 数値文字列をカタカナ読みに変換するロジック群。
//! 助数詞・スケール・記号・単位・慣用句のルール表は [`crate::rules`]
//! からロードしたデータを引数で受け取る (引数は明示的に渡す方針)。
//!
//! ## 構成
//! - [`helpers`] (crate-internal): 全角半角・カンマ・末尾数字・促音化等の小関数
//! - [`digit`]   : `number_to_katakana` (純粋アルゴリズム、データ非依存)
//! - [`counter`] : `euphonic_counter_read` (連濁・促音化・kana 末尾置換)
//! - [`phrase`]  : `NumericPhraseMatcher` (慣用句先行確定、regex pre-compile)
//! - [`extras`]  : スケール / SI 単位 / 記号の単発読み変換
//!
//! ## 公開 API (主なもの)
//! - [`number_to_katakana`]
//! - [`euphonic_counter_read`]
//! - [`NumericPhraseMatcher`] / [`apply_numeric_overrides`]
//! - [`scale_reading`] / [`si_unit_reading`] / [`symbol_char_reading`]

mod counter;
mod digit;
mod extras;
pub(crate) mod helpers;
mod phrase;

pub use extras::{scale_reading, si_unit_reading, symbol_char_reading};
pub use counter::euphonic_counter_read;
pub use digit::number_to_katakana;
pub use phrase::{apply_numeric_overrides, NumericPhraseMatcher};
