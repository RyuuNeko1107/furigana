//! 読み解決パイプライン
//!
//! テキスト → [`ReadingToken`] 列を生成する pipeline。
//! 内部で形態素解析・辞書ルックアップ・文脈ルール・数値処理を統合する。
//!
//! ## 優先順位 (resolve_reading)
//! 1. 漢字を含まない → 読み不要 (None)
//! 2. dict (override + user + core) lookup → ヒットすればそれ
//! 3. 文脈ルール (ContextData) → ヒットすればそれ
//! 4. 形態素解析が出した読み (lindera) → 漢字以外なら採用
//! 5. fallback: None

use crate::analyzer::{Analyzer, MorphToken};
use crate::chunks::NumberChunker;
use crate::dict::Dict;
use crate::kana;
use crate::numbers::NumericPhraseMatcher;
use crate::rules::{ContextData, ContextMatch, RulesData};

/// 読み付きトークン
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingToken {
    /// 表層形
    pub surface: String,
    /// カタカナ読み (漢字無しなら `None`)
    pub reading: Option<String>,
}

// ============================================================================
// 1. テキスト全体のパイプライン
// ============================================================================

/// テキストを `ReadingToken` 列に変換
///
/// 流れ:
/// 1. 異体字正規化 (`compat_map`)
/// 2. 慣用語句先行確定 (`numeric_phrases`)
/// 3. 数値テキストオーケストレーション (`NumberChunker`、日付/時刻/助数詞等)
/// 4. 残ったチャンクを形態素解析 + dict + 文脈ルール
#[must_use]
pub fn tokenize_text(
    text: &str,
    analyzer: &Analyzer,
    rules: &RulesData,
    dict: &Dict,
    phrase_matcher: &NumericPhraseMatcher,
    chunker: &NumberChunker,
) -> Vec<ReadingToken> {
    if text.is_empty() {
        return Vec::new();
    }

    // 1. 異体字正規化
    let normalized = kana::normalize_text(text, &rules.compat);

    // 2. 慣用語句先行確定
    let phrase_chunks = phrase_matcher.apply(&normalized);

    // 3 & 4. チャンクごとに処理
    let mut result = Vec::new();
    for (surface, reading) in phrase_chunks {
        if let Some(r) = reading {
            // 慣用句確定済み
            result.push(ReadingToken {
                surface,
                reading: Some(r),
            });
            continue;
        }

        // 慣用句外 → 数値オーケストレーションに通す
        let num_chunks = chunker.split(&surface);
        for (s, num_reading) in num_chunks {
            if let Some(r) = num_reading {
                result.push(ReadingToken {
                    surface: s,
                    reading: Some(r),
                });
            } else {
                // 数値でも無い → 形態素解析
                let chunk_tokens = tokenize_chunk(&s, analyzer, &rules.context, dict);
                result.extend(chunk_tokens);
            }
        }
    }

    result
}

fn tokenize_chunk(
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

// ============================================================================
// 2. 隣接トークンの dict 結合
// ============================================================================

const MAX_MERGE: usize = 5;

/// 隣接する形態素トークンを最長一致で dict マッチさせる。
///
/// 例: ["所", "謂"] → dict に "所謂" がある → ["所謂"] に結合
fn merge_with_dict(tokens: &[MorphToken], dict: &Dict) -> Vec<MorphToken> {
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
            if j > i && dict.lookup(&combined).is_some() {
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

// ============================================================================
// 3. 個別トークンの読み解決
// ============================================================================

fn resolve_reading(
    token: &MorphToken,
    all_tokens: &[MorphToken],
    idx: usize,
    context: &ContextData,
    dict: &Dict,
) -> Option<String> {
    let surface = &token.surface;

    // 1. 漢字を含まない → 読み不要
    if !kana::has_kanji(surface) {
        return None;
    }

    // 2. dict lookup (最優先)
    if let Some(reading) = dict.lookup(surface) {
        return Some(reading.to_string());
    }

    // 3. 文脈ルール
    if let Some(reading) = apply_context_rules(context, all_tokens, idx) {
        return Some(reading);
    }

    // 4. 形態素解析の読み
    if let Some(reading) = &token.reading {
        if kana::has_katakana(reading) && reading != surface {
            return Some(reading.clone());
        }
    }

    // 5. fallback
    None
}

// ============================================================================
// 4. 文脈ルールエンジン (data-driven)
// ============================================================================

/// 当該 token (idx) の surface に対応する [`ContextRule`] を引き、
/// match 条件を順に評価して読みを決定する。
///
/// - どの match にもヒット → その reading
/// - default のみあり → それ
/// - rule がそもそも無い OR どれにもヒットせず default も無い → None
#[must_use]
pub fn apply_context_rules(
    context: &ContextData,
    tokens: &[MorphToken],
    idx: usize,
) -> Option<String> {
    let token = tokens.get(idx)?;
    let surface = token.surface.as_str();

    let rule = context.rules.iter().find(|r| r.surface == surface)?;

    for m in &rule.matches {
        if context_match_eval(m, tokens, idx) {
            return Some(m.reading.clone());
        }
    }

    rule.default.clone()
}

fn context_match_eval(m: &ContextMatch, tokens: &[MorphToken], idx: usize) -> bool {
    let token = &tokens[idx];
    let prev = idx.checked_sub(1).and_then(|i| tokens.get(i));
    let next = tokens.get(idx + 1);
    let next_next = tokens.get(idx + 2);

    // ─── 前トークン条件 ────────────────────────────────────────────────────
    if let Some(eq) = &m.prev_eq {
        if prev.map(|t| t.surface.as_str()) != Some(eq.as_str()) {
            return false;
        }
    }
    if !m.prev_ends_with_any.is_empty() {
        let ok = prev.is_some_and(|t| {
            m.prev_ends_with_any
                .iter()
                .any(|s| t.surface.ends_with(s.as_str()))
        });
        if !ok {
            return false;
        }
    }
    if m.prev_ends_with_month {
        let ok = prev.is_some_and(|t| ends_with_month(&t.surface));
        if !ok {
            return false;
        }
    }

    // ─── 次トークン条件 ────────────────────────────────────────────────────
    if let Some(eq) = &m.next_eq {
        if next.map(|t| t.surface.as_str()) != Some(eq.as_str()) {
            return false;
        }
    }
    if let Some(prefix) = &m.next_starts_with {
        let ok = next.is_some_and(|t| t.surface.starts_with(prefix.as_str()));
        if !ok {
            return false;
        }
    }
    if !m.next_starts_with_any.is_empty() {
        let ok = next.is_some_and(|t| {
            m.next_starts_with_any
                .iter()
                .any(|s| t.surface.starts_with(s.as_str()))
        });
        if !ok {
            return false;
        }
    }
    if m.next_starts_with_digit {
        let ok = next.is_some_and(|t| starts_with_digit(&t.surface));
        if !ok {
            return false;
        }
    }

    // ─── 次の次トークン条件 ────────────────────────────────────────────────
    if !m.next_next_starts_with_any.is_empty() {
        let ok = next_next.is_some_and(|t| {
            m.next_next_starts_with_any
                .iter()
                .any(|s| t.surface.starts_with(s.as_str()))
        });
        if !ok {
            return false;
        }
    }

    // ─── 品詞条件 ──────────────────────────────────────────────────────────
    if let Some(eq) = &m.pos_eq {
        if token.pos.as_deref() != Some(eq.as_str()) {
            return false;
        }
    }

    true
}

/// 月名 (一月〜十二月、1月〜12月、全角含む) で終わるか
fn ends_with_month(s: &str) -> bool {
    const MONTHS: &[&str] = &[
        "一月",
        "二月",
        "三月",
        "四月",
        "五月",
        "六月",
        "七月",
        "八月",
        "九月",
        "十月",
        "十一月",
        "十二月",
        "1月",
        "2月",
        "3月",
        "4月",
        "5月",
        "6月",
        "7月",
        "8月",
        "9月",
        "10月",
        "11月",
        "12月",
        "１月",
        "２月",
        "３月",
        "４月",
        "５月",
        "６月",
        "７月",
        "８月",
        "９月",
    ];
    MONTHS.iter().any(|m| s.ends_with(m))
}

fn starts_with_digit(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || ('０'..='９').contains(&c))
}

// ============================================================================
// 5. 出力形式変換
// ============================================================================

/// トークン列をひらがな文字列に変換 (TTS 等向け)
///
/// - 読みあり → カタカナをひらがな化
/// - 読みなし & 純カタカナ → そのまま
/// - 読みなし & その他 → そのまま
#[must_use]
pub fn tokens_to_hiragana(tokens: &[ReadingToken]) -> String {
    let mut out = String::new();
    for t in tokens {
        if let Some(reading) = &t.reading {
            out.push_str(&kana::kata_to_hira(reading));
        } else {
            out.push_str(&t.surface);
        }
    }
    out
}

/// トークン列を `{漢字|ひらがな}` 形式の ruby 文字列に変換
///
/// - 読みあり & ひらがな化後 surface と異なる → `{surface|reading}`
/// - 読みあり & ひらがな化後 surface と同じ → そのまま (ruby 不要)
/// - 読みなし → そのまま
#[must_use]
pub fn tokens_to_ruby(tokens: &[ReadingToken]) -> String {
    let mut out = String::new();
    for t in tokens {
        match &t.reading {
            Some(reading) => {
                let hira = kana::kata_to_hira(reading);
                if hira == t.surface {
                    out.push_str(&t.surface);
                } else {
                    out.push('{');
                    out.push_str(&t.surface);
                    out.push('|');
                    out.push_str(&hira);
                    out.push('}');
                }
            }
            None => out.push_str(&t.surface),
        }
    }
    out
}

// ============================================================================
// テスト
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load_rules_dir;
    use std::path::PathBuf;

    fn rules() -> RulesData {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data")
            .join("rules");
        load_rules_dir(&dir).expect("load rules failed")
    }

    fn analyzer() -> Analyzer {
        Analyzer::new().expect("Analyzer init failed")
    }

    fn empty_phrase_matcher() -> NumericPhraseMatcher {
        NumericPhraseMatcher::empty()
    }

    fn empty_dict() -> Dict {
        Dict::new()
    }

    fn make_chunker(r: &RulesData) -> NumberChunker {
        NumberChunker::new(r)
    }

    fn morph(surface: &str, pos: Option<&str>) -> MorphToken {
        MorphToken {
            surface: surface.to_string(),
            reading: None,
            pos: pos.map(ToString::to_string),
            pos_detail: None,
            conjugation_type: None,
            conjugation_form: None,
            base_form: None,
        }
    }

    // ─── apply_context_rules ──────────────────────────────────────────────

    #[test]
    fn context_rule_hitori() {
        let rules = rules();
        let tokens = vec![morph("一人", Some("名詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 0),
            Some("ヒトリ".to_string())
        );
    }

    #[test]
    fn context_rule_tsuitachi_after_month() {
        let rules = rules();
        let tokens = vec![morph("一月", Some("名詞")), morph("一日", Some("名詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 1),
            Some("ツイタチ".to_string())
        );
    }

    #[test]
    fn context_rule_ichinichi_with_duration_suffix() {
        let rules = rules();
        let tokens = vec![morph("一日", Some("名詞")), morph("中", Some("名詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 0),
            Some("イチニチ".to_string())
        );
    }

    #[test]
    fn context_rule_ichinichi_with_duration_prefix() {
        let rules = rules();
        let tokens = vec![morph("丸", Some("名詞")), morph("一日", Some("名詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 1),
            Some("イチニチ".to_string())
        );
    }

    #[test]
    fn context_rule_jouzu_only_for_noun() {
        let rules = rules();
        let nominal = vec![morph("上手", Some("名詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &nominal, 0),
            Some("ジョウズ".to_string())
        );
        // 品詞条件にヒットしない → default なし → None
        let other = vec![morph("上手", Some("動詞"))];
        assert_eq!(apply_context_rules(&rules.context, &other, 0), None);
    }

    #[test]
    fn context_rule_otonage_with_na() {
        let rules = rules();
        let tokens = vec![morph("大人気", Some("名詞")), morph("ない", Some("形容詞"))];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 0),
            Some("オトナゲ".to_string())
        );
    }

    #[test]
    fn context_rule_otonage_with_no_nai() {
        let rules = rules();
        // 「大人気」「の」「ない」 → オトナゲ
        let tokens = vec![
            morph("大人気", Some("名詞")),
            morph("の", Some("助詞")),
            morph("ない", Some("形容詞")),
        ];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 0),
            Some("オトナゲ".to_string())
        );
    }

    #[test]
    fn context_rule_dainkinki_default() {
        let rules = rules();
        // 「大人気」「の」「映画」 → デフォルト ダイニンキ
        let tokens = vec![
            morph("大人気", Some("名詞")),
            morph("の", Some("助詞")),
            morph("映画", Some("名詞")),
        ];
        assert_eq!(
            apply_context_rules(&rules.context, &tokens, 0),
            Some("ダイニンキ".to_string())
        );
    }

    #[test]
    fn context_rule_no_match_returns_none() {
        let rules = rules();
        let tokens = vec![morph("無関係な単語", None)];
        assert_eq!(apply_context_rules(&rules.context, &tokens, 0), None);
    }

    // ─── tokens_to_hiragana / tokens_to_ruby ──────────────────────────────

    #[test]
    fn tokens_to_hiragana_basic() {
        let tokens = vec![
            ReadingToken {
                surface: "灰桜".to_string(),
                reading: Some("ハイザクラ".to_string()),
            },
            ReadingToken {
                surface: "の".to_string(),
                reading: None,
            },
        ];
        assert_eq!(tokens_to_hiragana(&tokens), "はいざくらの");
    }

    #[test]
    fn tokens_to_ruby_basic() {
        let tokens = vec![
            ReadingToken {
                surface: "灰桜".to_string(),
                reading: Some("ハイザクラ".to_string()),
            },
            ReadingToken {
                surface: "の".to_string(),
                reading: None,
            },
        ];
        assert_eq!(tokens_to_ruby(&tokens), "{灰桜|はいざくら}の");
    }

    #[test]
    fn tokens_to_ruby_skips_when_kana_matches_surface() {
        // surface 「あ」, reading 「ア」 → ひらがな化で「あ」と一致 → ruby 不要
        let tokens = vec![ReadingToken {
            surface: "あ".to_string(),
            reading: Some("ア".to_string()),
        }];
        assert_eq!(tokens_to_ruby(&tokens), "あ");
    }

    // ─── 統合 (tokenize_text) ─────────────────────────────────────────────

    #[test]
    fn tokenize_text_empty() {
        let r = rules();
        let a = analyzer();
        let c = make_chunker(&r);
        let result = tokenize_text("", &a, &r, &empty_dict(), &empty_phrase_matcher(), &c);
        assert!(result.is_empty());
    }

    #[test]
    fn tokenize_text_with_dict_overrides_lindera() {
        let r = rules();
        let a = analyzer();
        let mut d = Dict::new();
        d.insert("灰桜", "ハイザクラ");
        let m = empty_phrase_matcher();
        let c = make_chunker(&r);

        let tokens = tokenize_text("灰桜", &a, &r, &d, &m, &c);
        assert!(tokens
            .iter()
            .any(|t| t.reading.as_deref() == Some("ハイザクラ")));
    }

    #[test]
    fn tokenize_text_applies_phrase_matcher() {
        let r = rules();
        let a = analyzer();
        let phrases = NumericPhraseMatcher::new(&r.numeric_phrases);
        let c = make_chunker(&r);

        let tokens = tokenize_text("二十歳", &a, &r, &empty_dict(), &phrases, &c);
        assert!(
            tokens
                .iter()
                .any(|t| t.surface == "二十歳" && t.reading.as_deref() == Some("ハタチ")),
            "tokens: {tokens:?}"
        );
    }

    #[test]
    fn tokenize_text_to_ruby_pipeline() {
        let r = rules();
        let a = analyzer();
        let mut d = Dict::new();
        d.insert("灰桜", "ハイザクラ");
        let m = empty_phrase_matcher();
        let c = make_chunker(&r);

        let tokens = tokenize_text("灰桜の道", &a, &r, &d, &m, &c);
        let ruby = tokens_to_ruby(&tokens);
        assert!(ruby.contains("{灰桜|はいざくら}"), "ruby: {ruby}");
    }

    #[test]
    fn tokenize_text_normalizes_compat_chars() {
        // 異体字データは本体に embed されないため、テスト用に手動で注入
        let mut r = rules();
        r.compat.map.insert("髙".to_string(), "高".to_string());

        let a = analyzer();
        let mut d = Dict::new();
        d.insert("高崎", "タカサキ");
        let c = make_chunker(&r);

        // 「髙崎」 (異体字) → compat_map で「高崎」に正規化されて dict ヒット
        let tokens = tokenize_text("髙崎", &a, &r, &d, &empty_phrase_matcher(), &c);
        assert!(
            tokens
                .iter()
                .any(|t| t.reading.as_deref() == Some("タカサキ")),
            "tokens: {tokens:?}"
        );
    }

    #[test]
    fn tokenize_text_handles_counter_via_chunker() {
        let r = rules();
        let a = analyzer();
        let m = empty_phrase_matcher();
        let c = make_chunker(&r);
        let tokens = tokenize_text("3本のバナナ", &a, &r, &empty_dict(), &m, &c);
        // 3本 → サンボン がチャンク段階で確定する
        assert!(
            tokens
                .iter()
                .any(|t| t.surface == "3本" && t.reading.as_deref() == Some("サンボン")),
            "tokens: {tokens:?}"
        );
    }
}
