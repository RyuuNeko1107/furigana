//! [`ReadingToken`] 列の出力形式変換
//!
//! - `tokens_to_hiragana`: TTS / ひらがな展開 (surface 文字種ごとに切替)
//! - `tokens_to_ruby`    : `{kanji|hiragana}` 形式 (HTML ルビ生成等の前段)

use super::ReadingToken;
use crate::kana;

/// surface が「漢字を含む」 かどうかで reading 出力を切替えるルールに使う判定
///
/// 出力ルール:
/// - **漢字を含む** → reading をひらがな化 (例: 「灰桜」 → 「はいざくら」)
/// - **漢字を含まない** (= ASCII / 全角英字 / カタカナ / ひらがな / 数字 / 記号 のみ) →
///   reading を **カタカナのまま維持** (例: 「Kubernetes」 → 「クバネティス」、
///   「3本」 (漢字「本」 含む) → 「さんぼん」 だが、 「3」 単独 → 「サン」)
///
/// 「ひらがな / カタカナ / ASCII 英字 のみ」 の surface は通常 reading が surface と
/// 同等 (ひらがなは reading なしで surface のまま、 カタカナや英字は dict ヨミ) なので
/// そのまま出力すれば自然な日本語表現になる。
fn surface_has_kanji(surface: &str) -> bool {
    surface.chars().any(kana::is_kanji_char)
}

/// トークン列をひらがな文字列に変換 (TTS 等向け)
///
/// - 読みあり + surface が漢字を含む → reading を **ひらがな化** (kata_to_hira)
/// - 読みあり + surface が漢字を含まない → reading を **カタカナに統一** (hira_to_kata)
///   - アルファベット / 数字 / 記号 / 既存カタカナ surface は元 reading の表記を問わず
///     カタカナで出力 (例: `rules/symbols.toml` の 「〜=から」 もカタカナ「カラ」 に揃える)
/// - 読みなし → surface をそのまま
#[must_use]
pub fn tokens_to_hiragana(tokens: &[ReadingToken]) -> String {
    let mut out = String::new();
    for t in tokens {
        if let Some(reading) = &t.reading {
            if surface_has_kanji(&t.surface) {
                out.push_str(&kana::kata_to_hira(reading));
            } else {
                // ASCII 英字 / 数字 / 記号 / カタカナ / ひらがな のみの surface は
                // reading をカタカナに統一して出力 (アルファベット由来 / 数字・記号系を
                // 一貫してカタカナ表記にする)。
                out.push_str(&kana::hira_to_kata(reading));
            }
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
/// - 読みなし → surface をそのまま
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hiragana_basic() {
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
    fn ruby_basic() {
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
    fn ruby_skips_when_kana_matches_surface() {
        // surface 「あ」, reading 「ア」 → ひらがな化で「あ」と一致 → ruby 不要
        let tokens = vec![ReadingToken {
            surface: "あ".to_string(),
            reading: Some("ア".to_string()),
        }];
        assert_eq!(tokens_to_ruby(&tokens), "あ");
    }

    /// アルファベット surface (loanword 等) は reading をカタカナのまま維持
    #[test]
    fn hiragana_keeps_katakana_for_alphabet_surface() {
        let tokens = vec![
            ReadingToken {
                surface: "Kubernetes".to_string(),
                reading: Some("クバネティス".to_string()),
            },
            ReadingToken {
                surface: "が安定".to_string(), // 漢字含むのでひらがな化
                reading: Some("ガアンテイ".to_string()),
            },
        ];
        // Kubernetes はカタカナ維持、 「が安定」 部分はひらがな化
        assert_eq!(tokens_to_hiragana(&tokens), "クバネティスがあんてい");
    }

    /// 漢字を含まない混在 (英字 + 記号) もカタカナ維持
    #[test]
    fn hiragana_keeps_katakana_for_symbol_surface() {
        let tokens = vec![ReadingToken {
            surface: "C++".to_string(),
            reading: Some("シープラスプラス".to_string()),
        }];
        assert_eq!(tokens_to_hiragana(&tokens), "シープラスプラス");
    }

    /// 漢字を含むなら従来通りひらがな化
    #[test]
    fn hiragana_lowers_when_surface_has_kanji() {
        let tokens = vec![ReadingToken {
            surface: "灰桜".to_string(),
            reading: Some("ハイザクラ".to_string()),
        }];
        assert_eq!(tokens_to_hiragana(&tokens), "はいざくら");
    }
}
