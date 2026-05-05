//! `Furigana::minimal()` の **空 rules** で暴走する component を絞る。
//! reading::tokenize_text の流れを順に直接呼ぶ。
//!
//! 実行: `cargo test -p ja-furigana --test components_repro -- --include-ignored`

use furigana::chunks::NumberChunker;
use furigana::dict::Dict;
use furigana::numbers::NumericPhraseMatcher;
use furigana::rules::RulesData;
use furigana::Furigana;

const SAMPLE: &str = "こんにちは。さようなら。";

#[test]
#[ignore = "minimal reproduce"]
fn empty_phrase_matcher_apply() {
    let m = NumericPhraseMatcher::empty();
    let chunks = m.apply(SAMPLE);
    assert!(!chunks.is_empty());
}

#[test]
#[ignore = "minimal reproduce"]
fn empty_chunker_split() {
    let r = RulesData::default();
    let c = NumberChunker::new(&r);
    let parts = c.split(SAMPLE);
    assert!(!parts.is_empty());
}

/// pipeline::tokenize_chunk は private、ja-furigana 経由で同等のことをする
/// (Furigana::tokenize は内部で chunker.split → tokenize_chunk を呼ぶ)
#[test]
#[ignore = "minimal reproduce"]
fn tokenize_via_furigana_just_kana_no_period() {
    let f = Furigana::minimal().expect("init");
    let _ = f.tokenize("こんにちは");
}

#[test]
#[ignore = "minimal reproduce"]
fn tokenize_via_furigana_period_only() {
    let f = Furigana::minimal().expect("init");
    let _ = f.tokenize("。");
}

#[test]
#[ignore = "minimal reproduce"]
fn tokenize_via_furigana_with_period() {
    let f = Furigana::minimal().expect("init");
    let _ = f.tokenize("こんにちは。");
}
