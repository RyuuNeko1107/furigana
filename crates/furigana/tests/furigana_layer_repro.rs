//! Furigana の処理パイプラインのどの層で巨大 alloc 暴走が起きるかを切り分ける。
//!
//! Lindera 単体は問題なしと判明したので (lindera_minimal_repro.rs)、
//! - reading::tokenize_text (内部で Analyzer + dict + rules + phrase_matcher + chunker)
//! - tokens_to_hiragana / tokens_to_ruby
//! - tts::normalize_for_tts
//! のどこに bug があるかを順番に試す。
//!
//! 実行:
//! ```sh
//! cargo test -p ja-furigana --test furigana_layer_repro -- --include-ignored
//! ```

use furigana::{Furigana, TtsOptions};

const SAMPLE: &str = "こんにちは。さようなら。";

/// (1) `Furigana::minimal()` だけ。tokenize は呼ばない。
#[test]
#[ignore = "minimal reproduce"]
fn just_init() {
    let _f = Furigana::minimal().expect("init");
}

/// (2) tokenize() を呼ぶ。
#[test]
#[ignore = "minimal reproduce"]
fn just_tokenize() {
    let f = Furigana::minimal().expect("init");
    let tokens = f.tokenize(SAMPLE);
    assert!(!tokens.is_empty());
}

/// (3) to_hiragana() = tokenize + tokens_to_hiragana。
#[test]
#[ignore = "minimal reproduce"]
fn to_hiragana_only() {
    let f = Furigana::minimal().expect("init");
    let _r = f.to_hiragana(SAMPLE);
}

/// (4) to_ruby() = tokenize + tokens_to_ruby。
#[test]
#[ignore = "minimal reproduce"]
fn to_ruby_only() {
    let f = Furigana::minimal().expect("init");
    let _r = f.to_ruby(SAMPLE);
}

/// (5) to_tts() = to_hiragana + normalize_for_tts。
/// オリジナルの `api::tests::to_tts_inserts_pauses` と同じ。再現するはず。
#[test]
#[ignore = "minimal reproduce"]
fn to_tts_full() {
    let f = Furigana::minimal().expect("init");
    let opts = TtsOptions::default();
    let _r = f.to_tts(SAMPLE, &opts);
}

/// (6) tokens_to_hiragana の結果を normalize_for_tts に直接食わせる
/// (= to_tts の中身を分解)。to_hiragana が OK で normalize で panic なら
/// normalize 側に bug。
#[test]
#[ignore = "minimal reproduce"]
fn manual_to_tts_pipeline() {
    let f = Furigana::minimal().expect("init");
    let hira = f.to_hiragana(SAMPLE);
    let opts = TtsOptions::default();
    let _r = furigana::tts::normalize_for_tts(&hira, &opts);
}

/// (7) 句読点なしの to_tts (control)
#[test]
#[ignore = "minimal reproduce"]
fn to_tts_no_period() {
    let f = Furigana::minimal().expect("init");
    let opts = TtsOptions::default();
    let _r = f.to_tts("こんにちはさようなら", &opts);
}

/// (8) 句点 1 個だけの to_tts
#[test]
#[ignore = "minimal reproduce"]
fn to_tts_single_period() {
    let f = Furigana::minimal().expect("init");
    let opts = TtsOptions::default();
    let _r = f.to_tts("こんにちは。", &opts);
}
