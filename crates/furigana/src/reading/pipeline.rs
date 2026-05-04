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
/// 優先順位:
/// 1. 漢字を含まない → 読み不要 (`None`)
/// 2. dict lookup
/// 3. 文脈ルール ([`ContextData`])
/// 4. 形態素解析 (lindera) の reading
/// 5. fallback `None`
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

    if let Some(reading) = dict.lookup(surface) {
        return Some(reading.to_string());
    }

    if let Some(reading) = apply_context_rules(context, all_tokens, idx) {
        return Some(reading);
    }

    if let Some(reading) = &token.reading {
        if kana::has_katakana(reading) && reading != surface {
            return Some(reading.clone());
        }
    }

    None
}
