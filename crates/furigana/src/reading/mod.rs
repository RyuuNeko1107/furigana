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
    single_overrides: &crate::single_overrides::SingleOverrides,
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
                let chunk_tokens = pipeline::tokenize_chunk(
                    &s,
                    analyzer,
                    &rules.context,
                    dict,
                    single_overrides,
                );
                result.extend(chunk_tokens);
            }
        }
    }

    // 踊り字「々」 後処理: Lindera が「神々」 を「神」 + 「々」 で分解した場合に
    // 「々」 token の reading が None になる問題への対応 (issue #16)。
    // token 列を走査し、 surface = "々" / reading = None の token を見つけたら、
    // 直前 token の reading を複製する (連濁判定はせず、 連濁が必要な語句は
    // rules/postprocess.toml に個別 regex rule で蓄積する方針)。
    expand_odoriji_inplace(&mut result);

    result
}

/// 踊り字「々」 を直前 token の reading + 連濁判定で展開する (in-place)
///
/// 「神々」 → tokens=[(「神」, カミ), (「々」, None)] → tokens=[(「神」, カミ), (「々」, ガミ)]
/// （連濁あり → カミガミ）
///
/// 連濁判定 (簡易版):
/// - 直前 reading の **第 1 音がカ/サ/タ/ハ 行** → 濁音化して「々」 reading に採用
///   (神→ガミ、 人→ビト、 時→ドキ、 様→ザマ、 国→グニ)
/// - **ナ/マ/ヤ/ラ/ワ/ア 行など** → 連濁対象外 → そのまま複製
///   (我→レ、 山→マ、 年→ン 等は連濁ルールが無いので清音のままコピー =
///    我々=ワレワレ、 山々=ヤマヤマ、 年々=ネンネン)
///
/// 例外語 (個々=ココ、 我々=ワレワレ など) で誤連濁が出る場合は
/// `core/jukugo/*.toml` に固有 entry を登録すれば 5 段階優先順位で先に hit する。
fn expand_odoriji_inplace(tokens: &mut [ReadingToken]) {
    for i in 1..tokens.len() {
        if tokens[i].surface == "々" && tokens[i].reading.is_none() {
            if let Some(prev_reading) = tokens[i - 1].reading.clone() {
                let voiced = voice_first_kana(&prev_reading).unwrap_or(prev_reading);
                tokens[i].reading = Some(voiced);
            }
        }
    }
}

/// 全角カタカナ reading の **第 1 音を連濁化** する。
///
/// カ/サ/タ/ハ 行の清音 → 対応する濁音 / 半濁音前の濁音に変換。
/// 連濁対象外 (ア/ナ/マ/ヤ/ラ/ワ 行 + 既に濁音 + ハ 行半濁音) は `None` を返し、
/// 呼び出し側で「清音のまま複製」 にフォールバックする。
fn voice_first_kana(reading: &str) -> Option<String> {
    let mut chars = reading.chars();
    let first = chars.next()?;
    let voiced = match first {
        'カ' => 'ガ', 'キ' => 'ギ', 'ク' => 'グ', 'ケ' => 'ゲ', 'コ' => 'ゴ',
        'サ' => 'ザ', 'シ' => 'ジ', 'ス' => 'ズ', 'セ' => 'ゼ', 'ソ' => 'ゾ',
        'タ' => 'ダ', 'チ' => 'ヂ', 'ツ' => 'ヅ', 'テ' => 'デ', 'ト' => 'ド',
        'ハ' => 'バ', 'ヒ' => 'ビ', 'フ' => 'ブ', 'ヘ' => 'ベ', 'ホ' => 'ボ',
        _ => return None,
    };
    let mut out = String::new();
    out.push(voiced);
    out.push_str(chars.as_str());
    Some(out)
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

    fn empty_overrides() -> crate::single_overrides::SingleOverrides {
        crate::single_overrides::SingleOverrides::new()
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
        let result = tokenize_text("", &a, &r, &empty_dict(), &empty_phrase_matcher(), &c, &empty_overrides());
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

        let tokens = tokenize_text("灰桜", &a, &r, &d, &m, &c, &empty_overrides());
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

        let tokens = tokenize_text("二十歳", &a, &r, &empty_dict(), &phrases, &c, &empty_overrides());
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

        let tokens = tokenize_text("灰桜の道", &a, &r, &d, &m, &c, &empty_overrides());
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

        let tokens = tokenize_text("髙崎", &a, &r, &d, &empty_phrase_matcher(), &c, &empty_overrides());
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
        let tokens = tokenize_text("3本のバナナ", &a, &r, &empty_dict(), &m, &c, &empty_overrides());
        // 3本 → サンボン がチャンク段階で確定する
        assert!(
            tokens
                .iter()
                .any(|t| t.surface == "3本" && t.reading.as_deref() == Some("サンボン")),
            "tokens: {tokens:?}"
        );
    }

    /// 踊り字「々」 展開 + 連濁: 「神々」 → カミ + ガミ (カ → ガ 連濁) (issue #16)
    #[test]
    fn expand_odoriji_with_rendaku() {
        let mut tokens = vec![
            ReadingToken {
                surface: "神".to_string(),
                reading: Some("カミ".to_string()),
            },
            ReadingToken {
                surface: "々".to_string(),
                reading: None,
            },
        ];
        expand_odoriji_inplace(&mut tokens);
        assert_eq!(tokens[1].reading.as_deref(), Some("ガミ"));
    }

    /// 連濁対象外 (ナ/マ/ヤ/ラ/ワ/ア 行始まり) は清音のまま複製
    /// 我々 = ワレワレ、 山々 = ヤマヤマ、 年々 = ネンネン
    #[test]
    fn expand_odoriji_no_rendaku_for_non_voiceable() {
        for (surface_first, reading_first) in [
            ("我", "ワレ"),
            ("山", "ヤマ"),
            ("年", "ネン"),
            ("代", "ダイ"),  // 既に濁音 → 連濁不可
            ("色", "イロ"),
        ] {
            let mut tokens = vec![
                ReadingToken {
                    surface: surface_first.to_string(),
                    reading: Some(reading_first.to_string()),
                },
                ReadingToken {
                    surface: "々".to_string(),
                    reading: None,
                },
            ];
            expand_odoriji_inplace(&mut tokens);
            assert_eq!(
                tokens[1].reading.as_deref(),
                Some(reading_first),
                "{surface_first}々: 連濁対象外なのでそのままコピーされるはず"
            );
        }
    }

    /// 「々」 token に既に reading がある場合は上書きしない (idempotent)
    #[test]
    fn expand_odoriji_skips_when_reading_exists() {
        let mut tokens = vec![
            ReadingToken {
                surface: "時々".to_string(),
                reading: Some("トキドキ".to_string()),
            },
            ReadingToken {
                surface: "々".to_string(),
                reading: Some("ドキ".to_string()), // すでに何か入ってる仮定
            },
        ];
        expand_odoriji_inplace(&mut tokens);
        assert_eq!(tokens[1].reading.as_deref(), Some("ドキ")); // 上書きされない
    }
}
