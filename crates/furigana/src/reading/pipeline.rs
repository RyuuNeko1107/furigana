//! 形態素解析チャンクの読み解決
//!
//! `tokenize_text` から呼ばれ、慣用句にも数値にも該当しなかった
//! テキストチャンクを Lindera で形態素解析し、各トークンに読みを付ける。

use super::context::apply_context_rules;
use super::merge::merge_with_dict;
use super::ReadingToken;
use crate::analyzer::{Analyzer, MorphToken};
use crate::dict::Dict;
use crate::kana;
use crate::rules::ContextData;

/// 単一チャンクを形態素解析 → dict 結合 → 読み解決
pub(super) fn tokenize_chunk(
    text: &str,
    analyzer: &Analyzer,
    context: &ContextData,
    dict: &Dict,
) -> Vec<ReadingToken> {
    if text.is_empty() {
        return Vec::new();
    }

    let morph_tokens = analyzer.tokenize(text);
    let merged = merge_with_dict(&morph_tokens, dict);

    let mut result = Vec::with_capacity(merged.len());
    for (idx, mt) in merged.iter().enumerate() {
        let reading = resolve_reading(mt, &merged, idx, context, dict);
        result.push(ReadingToken {
            surface: mt.surface.clone(),
            reading,
        });
    }
    result
}

/// 個別トークンの読みを解決する
///
/// 優先順位 (新):
/// 1. 漢字を含まない → 読み不要 (`None`)
/// 2. **文脈ルール** ([`ContextData`]) — 同形異音語 (一日 / 上手 / 市場 等) の
///    動的読み分けが効くように、辞書 lookup より先に評価する。
///    rule にマッチしない or default 無しなら次へ。
/// 3. dict lookup — 熟語固定読み (灰桜=ハイザクラ 等)
/// 4. 形態素解析 (lindera) の reading
/// 5. fallback `None`
///
/// 旧版では dict lookup が context rule より先だった結果、unihan に登録された
/// 単漢字読み (能=あたう、本=もと 等の動詞活用形 / 訓読み) が context rule の
/// default を遮断してしまっていた。
fn resolve_reading(
    token: &MorphToken,
    all_tokens: &[MorphToken],
    idx: usize,
    context: &ContextData,
    dict: &Dict,
) -> Option<String> {
    let surface = &token.surface;

    if !kana::has_kanji(surface) {
        return None;
    }

    if let Some(reading) = apply_context_rules(context, all_tokens, idx) {
        return Some(reading);
    }

    if let Some(reading) = dict.lookup(surface) {
        return Some(reading.to_string());
    }

    if let Some(reading) = &token.reading {
        if kana::has_katakana(reading) && reading != surface {
            return Some(reading.clone());
        }
    }

    None
}
