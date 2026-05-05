//! Lindera v3.x の cargo test harness 経由で巨大 alloc 暴走するバグの
//! minimal reproduce 候補。
//!
//! ja-furigana の wrapper は通さず、Lindera の素 API のみで再現するか確認する。
//! 再現すれば upstream (lindera/lindera) に投げる材料になる。
//!
//! 実行:
//! ```sh
//! cargo test -p ja-furigana --test lindera_minimal_repro -- --include-ignored
//! ```

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;

fn make_tokenizer() -> Tokenizer {
    let dict = load_dictionary("embedded://ipadic").expect("load ipadic");
    let seg = Segmenter::new(Mode::Normal, dict, None);
    Tokenizer::new(seg)
}

/// (1) tokenize だけ呼ぶ。details() は触らない。
/// → これが panic しないなら、details() 経路に bug がある。
#[test]
#[ignore = "minimal reproduce"]
fn just_tokenize_no_details() {
    let t = make_tokenizer();
    let tokens = t.tokenize("こんにちは。さようなら。").expect("tokenize");
    let count = tokens.len();
    assert!(count > 0, "should produce some tokens, got {count}");
}

/// (2) tokenize + 全 token の details() を呼ぶ。
/// → ja-furigana の Analyzer::tokenize と同じパターン。
/// 再現すれば Lindera 単体で 51 GB alloc fail が出る。
#[test]
#[ignore = "minimal reproduce"]
fn tokenize_and_call_details_for_each() {
    let t = make_tokenizer();
    let mut tokens = t.tokenize("こんにちは。さようなら。").expect("tokenize");
    for tok in tokens.iter_mut() {
        let _details = tok.details();
        // ここで `_details` を黙って drop。Lindera 内部の何かが trigger される疑い。
    }
}

/// (3) 数字を含む test (issue #326 と同じ japanese_number filter trigger 想定)
#[test]
#[ignore = "minimal reproduce"]
fn tokenize_with_digits() {
    let t = make_tokenizer();
    let mut tokens = t.tokenize("ぶん1。ぶん2。ぶん3。").expect("tokenize");
    for tok in tokens.iter_mut() {
        let _details = tok.details();
    }
}

/// (4) 句読点なし
#[test]
#[ignore = "minimal reproduce"]
fn tokenize_no_period() {
    let t = make_tokenizer();
    let mut tokens = t.tokenize("こんにちはさようなら").expect("tokenize");
    for tok in tokens.iter_mut() {
        let _details = tok.details();
    }
}

/// (5) 単一の「。」だけ
#[test]
#[ignore = "minimal reproduce"]
fn tokenize_only_period() {
    let t = make_tokenizer();
    let mut tokens = t.tokenize("。").expect("tokenize");
    for tok in tokens.iter_mut() {
        let _details = tok.details();
    }
}

/// (6) 同じ tokenizer で複数回 tokenize (cache や internal state の累積疑い)
#[test]
#[ignore = "minimal reproduce"]
fn tokenize_multiple_times() {
    let t = make_tokenizer();
    for _ in 0..5 {
        let mut tokens = t.tokenize("こんにちは。さようなら。").expect("tokenize");
        for tok in tokens.iter_mut() {
            let _details = tok.details();
        }
    }
}
