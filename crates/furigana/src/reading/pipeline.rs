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
use crate::single_overrides::SingleOverrides;

/// 単一チャンクを形態素解析 → dict 結合 → 読み解決
pub(super) fn tokenize_chunk(
    text: &str,
    analyzer: &Analyzer,
    context: &ContextData,
    dict: &Dict,
    single_overrides: &SingleOverrides,
) -> Vec<ReadingToken> {
    if text.is_empty() {
        return Vec::new();
    }

    let morph_tokens = analyzer.tokenize(text);
    let merged = merge_with_dict(&morph_tokens, dict);

    let mut result = Vec::with_capacity(merged.len());
    for (idx, mt) in merged.iter().enumerate() {
        let reading = resolve_reading(mt, &merged, idx, context, dict, single_overrides);
        result.push(ReadingToken {
            surface: mt.surface.clone(),
            reading,
        });
    }
    result
}

/// 個別トークンの読みを解決する
///
/// 優先順位 (公開 API パイプラインに準拠):
///
/// 1. 漢字を含まない → 読み不要 (`None`)
/// 2. **文脈ルール** ([`ContextData`]) — 同形異音語 (一日 / 上手 / 市場 等) の
///    動的読み分け
/// 3. **熟語辞書** ([`Dict::lookup_jukugo`]) — surface 2 文字以上の固定読み
///    (灰桜 / 円周率 / 駆逐艦 等)
/// 4. **単漢字 default override** ([`SingleOverrides::lookup`]) — 1 字 surface に
///    対する明示的 default 上書き (例: 「土 = ツチ」、 unihan 「ど」 と Lindera
///    「ド」 の両方より先に評価)。 全 unihan を Lindera より先にすると副作用
///    大 ([issue #15](https://github.com/RyuuNeko1107/ja-furigana/issues/15) の
///    R20 で 6 件 corpus regression 確認済み) のため、 明示的に override
///    したい単漢字だけを別 data file (`core/single_overrides.toml`) で管理する。
/// 5. **形態素解析 (lindera) の reading** — 動詞活用形などで Lindera が自然に
///    返してくる読み (これを 6 より優先することで、unihan の保守的な単漢字
///    読みが Lindera の文脈考慮を遮断するのを防ぐ)
/// 6. **単漢字辞書** ([`Dict::lookup_unihan`]) — 1 文字の最終 fallback
/// 7. fallback `None`
fn resolve_reading(
    token: &MorphToken,
    all_tokens: &[MorphToken],
    idx: usize,
    context: &ContextData,
    dict: &Dict,
    single_overrides: &SingleOverrides,
) -> Option<String> {
    let surface = &token.surface;

    if !kana::has_kanji(surface) {
        return None;
    }

    // 2. context rule
    if let Some(reading) = apply_context_rules(context, all_tokens, idx) {
        return Some(reading);
    }

    // 3. 熟語辞書 (≥ 2 文字)
    if let Some(reading) = dict.lookup_jukugo(surface) {
        return Some(reading.to_string());
    }

    // 4. 単漢字 default override (issue #15 の限定解): 明示的に 1 字 surface
    //    に対する override があれば Lindera より先に採用。
    //    lookup() は内部で「surface が 1 字」 制約を課すので、 ≥2 字 surface には影響しない。
    if let Some(reading) = single_overrides.lookup(surface) {
        return Some(reading.to_string());
    }

    // 4. Lindera reading
    if let Some(reading) = &token.reading {
        if kana::has_katakana(reading) && reading != surface {
            return Some(reading.clone());
        }
    }

    // 5. 単漢字 fallback (1 文字)
    if let Some(reading) = dict.lookup_unihan(surface) {
        return Some(reading.to_string());
    }

    None
}
