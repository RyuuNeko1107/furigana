//! 読み解決パイプライン
//!
//! テキスト → [`ReadingToken`] 列を生成する。
//! 内部で形態素解析・辞書ルックアップ・文脈ルール・数値処理を統合する。
//!
//! ## 構成
//! - `pipeline` (private) : 形態素解析の各チャンクの処理 (tokenize_chunk + resolve_reading)
//! - `merge`    (private) : 隣接トークンの dict 最長一致結合
//! - [`context`]          : 文脈ルールエンジン (apply_context_rules)
//! - [`output`]           : ReadingToken 列 → ひらがな / ruby
//!
//! ## 公開 API
//! - [`ReadingToken`]
//! - [`tokenize_text`]
//! - [`apply_context_rules`](context::apply_context_rules)
//! - [`tokens_to_hiragana`](output::tokens_to_hiragana) / [`tokens_to_ruby`](output::tokens_to_ruby)

pub mod context;
mod merge;
pub mod output;
mod pipeline;

pub use context::apply_context_rules;
pub use output::{tokens_to_hiragana, tokens_to_ruby};

use crate::analyzer::Analyzer;
use crate::chunks::NumberChunker;
use crate::dict::Dict;
use crate::kana;
use crate::numbers::NumericPhraseMatcher;
use crate::rules::RulesData;

/// 読み付きトークン
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingToken {
    /// 表層形
    pub surface: String,
    /// カタカナ読み (漢字無しなら `None`)
    pub reading: Option<String>,
}

/// テキストを [`ReadingToken`] 列に変換する (top-level)
///
/// 流れ:
/// 1. 異体字正規化 (`compat_map`)
/// 2. 慣用語句先行確定 (`numeric_phrases`)
/// 3. 数値テキストオーケストレーション ([`NumberChunker`]、日付/時刻/助数詞 等)
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
                let chunk_tokens = pipeline::tokenize_chunk(&s, analyzer, &rules.context, dict);
                result.extend(chunk_tokens);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    //! `tokenize_text` の統合テスト (各サブモジュールの単体テストはそれぞれの mod に)

    use super::*;
    use crate::loader::load_rules_dir;
    use std::path::PathBuf;

    fn rules() -> RulesData {
        // 本体に rules を embed しないため、テスト用 fixture を使う。
        // 実データは furigana-dict 側、`furigana dict pull` で配布される。
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
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

    #[test]
    fn empty_input() {
        let r = rules();
        let a = analyzer();
        let c = make_chunker(&r);
        let result = tokenize_text("", &a, &r, &empty_dict(), &empty_phrase_matcher(), &c);
        assert!(result.is_empty());
    }

    #[test]
    fn dict_overrides_lindera() {
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
    fn applies_phrase_matcher() {
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
    fn to_ruby_pipeline() {
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
    fn normalizes_compat_chars() {
        // 異体字データは本体に embed されないため、テスト用に手動で注入
        let mut r = rules();
        r.compat.map.insert("髙".to_string(), "高".to_string());

        let a = analyzer();
        let mut d = Dict::new();
        d.insert("高崎", "タカサキ");
        let c = make_chunker(&r);

        let tokens = tokenize_text("髙崎", &a, &r, &d, &empty_phrase_matcher(), &c);
        assert!(
            tokens
                .iter()
                .any(|t| t.reading.as_deref() == Some("タカサキ")),
            "tokens: {tokens:?}"
        );
    }

    #[test]
    fn handles_counter_via_chunker() {
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
