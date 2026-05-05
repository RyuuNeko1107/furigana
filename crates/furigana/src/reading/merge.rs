//! 隣接トークンの dict 最長一致結合
//!
//! 形態素解析が `[所, 謂]` のように分割した連続トークンを、辞書に登録された
//! `所謂` 等の複合語があれば 1 つの結合トークンに圧縮する。

use crate::analyzer::MorphToken;
use crate::dict::Dict;

/// 結合する最大トークン数 (これ以上長い複合語は dict 直接 lookup 側で扱う)
const MAX_MERGE: usize = 5;

/// 隣接する形態素トークンを最長一致で dict マッチさせる。
///
/// 例: `[所, 謂]` → dict に "所謂" がある → `[所謂]` に結合。
pub(super) fn merge_with_dict(tokens: &[MorphToken], dict: &Dict) -> Vec<MorphToken> {
    let len = tokens.len();
    if len == 0 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(len);
    let mut i = 0;

    while i < len {
        let mut best_end = i;
        let limit = (i + MAX_MERGE).min(len);
        let mut combined = String::new();

        for (j, t) in tokens.iter().enumerate().take(limit).skip(i) {
            combined.push_str(&t.surface);
            // 「複合語として結合する」というのは熟語辞書 (≥ 2 文字) でのみ起きる動作。
            // 単漢字 unihan は結合トリガにしない (j > i で既に 2 文字以上ある前提)。
            if j > i && dict.lookup_jukugo(&combined).is_some() {
                best_end = j + 1;
            }
        }

        if best_end > i {
            // 結合トークンを作成
            let mut surface = String::new();
            for t in &tokens[i..best_end] {
                surface.push_str(&t.surface);
            }
            result.push(MorphToken {
                surface,
                // dict ベースなので reading 等は最初のトークンを継承するに留め、
                // resolve_reading で改めて dict 引きで上書きされる
                reading: None,
                pos: tokens[i].pos.clone(),
                pos_detail: tokens[i].pos_detail.clone(),
                conjugation_type: tokens[i].conjugation_type.clone(),
                conjugation_form: tokens[i].conjugation_form.clone(),
                base_form: tokens[i].base_form.clone(),
            });
            i = best_end;
        } else {
            result.push(tokens[i].clone());
            i += 1;
        }
    }

    result
}
