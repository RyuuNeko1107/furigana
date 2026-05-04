//! [`ReadingToken`] 列の出力形式変換
//!
//! - `tokens_to_hiragana`: TTS / ひらがな展開
//! - `tokens_to_ruby`    : `{kanji|hiragana}` 形式 (HTML ルビ生成等の前段)

use super::ReadingToken;
use crate::kana;

/// トークン列をひらがな文字列に変換 (TTS 等向け)
///
/// - 読みあり → カタカナをひらがな化
/// - 読みなし → surface をそのまま
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
}
