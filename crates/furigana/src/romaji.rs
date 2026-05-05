//! ひらがな → ローマ字変換
//!
//! - **ヘボン式** (default): し→shi / ち→chi / つ→tsu / じゃ→ja / 撥音 ん は b/m/p の前で m
//! - **訓令式**: し→si / ち→ti / つ→tu / じゃ→zya / 撥音 ん は常に n
//!
//! 入力はひらがな (またはカタカナでも内部で hira 化) を想定。漢字を含む場合は先に
//! [`crate::Furigana::to_hiragana`] で変換してから呼ぶ。
//!
//! ## 例
//! ```
//! use furigana::romaji::{hiragana_to_romaji, RomajiStyle};
//! assert_eq!(hiragana_to_romaji("はいざくら", RomajiStyle::Hepburn), "haizakura");
//! assert_eq!(hiragana_to_romaji("つき", RomajiStyle::Hepburn), "tsuki");
//! assert_eq!(hiragana_to_romaji("つき", RomajiStyle::Kunrei), "tuki");
//! assert_eq!(hiragana_to_romaji("がっこう", RomajiStyle::Hepburn), "gakkou");
//! assert_eq!(hiragana_to_romaji("しんぶん", RomajiStyle::Hepburn), "shimbun");
//! ```

use crate::kana::kata_to_hira;

/// ローマ字スタイル
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RomajiStyle {
    /// ヘボン式 (英語圏で一般的、TTS / 名前ローマ字向け)
    #[default]
    Hepburn,
    /// 訓令式 (日本の学校教育で教わる、規則性が高い)
    Kunrei,
}

/// ひらがな (or カタカナ) 文字列をローマ字に変換する
#[must_use]
pub fn hiragana_to_romaji(s: &str, style: RomajiStyle) -> String {
    // カタカナ混じりでも安全に動くよう先に hira 化
    let hira = kata_to_hira(s);
    let chars: Vec<char> = hira.chars().collect();
    let mut out = String::with_capacity(hira.len() * 2);
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // 促音 っ → 次の子音を重ねる
        if c == 'っ' {
            if let Some(next_pair) = peek_kana(&chars, i + 1, style) {
                if let Some(first_consonant) = next_pair.romaji.bytes().next() {
                    if !is_vowel(first_consonant as char) {
                        // ヘボン式 ち系 (chi/cha/...) の前は t
                        let to_push = if style == RomajiStyle::Hepburn
                            && next_pair.romaji.starts_with("ch")
                        {
                            't'
                        } else {
                            first_consonant as char
                        };
                        out.push(to_push);
                    }
                }
            }
            i += 1;
            continue;
        }

        // 撥音 ん
        if c == 'ん' {
            // ヘボン式: b/m/p の前で m、それ以外は n
            // 訓令式: 常に n
            let next_first = chars
                .get(i + 1)
                .and_then(|_| peek_kana(&chars, i + 1, style))
                .and_then(|p| p.romaji.bytes().next());
            let romaji_n = if style == RomajiStyle::Hepburn {
                match next_first {
                    Some(b'b') | Some(b'm') | Some(b'p') => "m",
                    _ => "n",
                }
            } else {
                "n"
            };
            // 母音や y の前は ' で区切る (例: しんあい → shin'ai)
            let needs_apostrophe = matches!(
                next_first,
                Some(b'a') | Some(b'i') | Some(b'u') | Some(b'e') | Some(b'o') | Some(b'y')
            );
            out.push_str(romaji_n);
            if needs_apostrophe {
                out.push('\'');
            }
            i += 1;
            continue;
        }

        // 長音 ー → 直前の母音を repeat
        if c == 'ー' {
            if let Some(last) = out.chars().last() {
                if is_vowel(last) {
                    out.push(last);
                }
            }
            i += 1;
            continue;
        }

        // 拗音 (2 char、例: しゃ きゃ)
        if i + 1 < chars.len() {
            if let Some(romaji) = lookup_youon(chars[i], chars[i + 1], style) {
                out.push_str(romaji);
                i += 2;
                continue;
            }
        }

        // 単独 (1 char)
        if let Some(romaji) = lookup_single(c, style) {
            out.push_str(romaji);
        } else {
            // ひらがな以外 (英数字や記号) はそのまま透過
            out.push(c);
        }
        i += 1;
    }

    out
}

struct Pair<'a> {
    romaji: &'a str,
}

/// `i` 位置から始まる kana 1〜2 文字分の romaji 候補を返す (撥音 / 促音側で覗き見する用)
fn peek_kana(chars: &[char], i: usize, style: RomajiStyle) -> Option<Pair<'static>> {
    if i >= chars.len() {
        return None;
    }
    if i + 1 < chars.len() {
        if let Some(r) = lookup_youon(chars[i], chars[i + 1], style) {
            return Some(Pair { romaji: r });
        }
    }
    lookup_single(chars[i], style).map(|r| Pair { romaji: r })
}

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'i' | 'u' | 'e' | 'o')
}

fn lookup_single(c: char, style: RomajiStyle) -> Option<&'static str> {
    use RomajiStyle::{Hepburn, Kunrei};
    Some(match (c, style) {
        // 母音
        ('あ', _) => "a",
        ('い', _) => "i",
        ('う', _) => "u",
        ('え', _) => "e",
        ('お', _) => "o",
        // か行
        ('か', _) => "ka",
        ('き', _) => "ki",
        ('く', _) => "ku",
        ('け', _) => "ke",
        ('こ', _) => "ko",
        ('が', _) => "ga",
        ('ぎ', _) => "gi",
        ('ぐ', _) => "gu",
        ('げ', _) => "ge",
        ('ご', _) => "go",
        // さ行
        ('さ', _) => "sa",
        ('し', Hepburn) => "shi",
        ('し', Kunrei) => "si",
        ('す', _) => "su",
        ('せ', _) => "se",
        ('そ', _) => "so",
        ('ざ', _) => "za",
        ('じ', Hepburn) => "ji",
        ('じ', Kunrei) => "zi",
        ('ず', _) => "zu",
        ('ぜ', _) => "ze",
        ('ぞ', _) => "zo",
        // た行
        ('た', _) => "ta",
        ('ち', Hepburn) => "chi",
        ('ち', Kunrei) => "ti",
        ('つ', Hepburn) => "tsu",
        ('つ', Kunrei) => "tu",
        ('て', _) => "te",
        ('と', _) => "to",
        ('だ', _) => "da",
        ('ぢ', Hepburn) => "ji",
        ('ぢ', Kunrei) => "zi",
        ('づ', Hepburn) => "zu",
        ('づ', Kunrei) => "zu",
        ('で', _) => "de",
        ('ど', _) => "do",
        // な行
        ('な', _) => "na",
        ('に', _) => "ni",
        ('ぬ', _) => "nu",
        ('ね', _) => "ne",
        ('の', _) => "no",
        // は行
        ('は', _) => "ha",
        ('ひ', _) => "hi",
        ('ふ', Hepburn) => "fu",
        ('ふ', Kunrei) => "hu",
        ('へ', _) => "he",
        ('ほ', _) => "ho",
        ('ば', _) => "ba",
        ('び', _) => "bi",
        ('ぶ', _) => "bu",
        ('べ', _) => "be",
        ('ぼ', _) => "bo",
        ('ぱ', _) => "pa",
        ('ぴ', _) => "pi",
        ('ぷ', _) => "pu",
        ('ぺ', _) => "pe",
        ('ぽ', _) => "po",
        // ま行
        ('ま', _) => "ma",
        ('み', _) => "mi",
        ('む', _) => "mu",
        ('め', _) => "me",
        ('も', _) => "mo",
        // や行
        ('や', _) => "ya",
        ('ゆ', _) => "yu",
        ('よ', _) => "yo",
        // ら行
        ('ら', _) => "ra",
        ('り', _) => "ri",
        ('る', _) => "ru",
        ('れ', _) => "re",
        ('ろ', _) => "ro",
        // わ行
        ('わ', _) => "wa",
        ('ゐ', _) => "i",
        ('ゑ', _) => "e",
        ('を', _) => "wo",
        // ヴ
        ('ゔ', _) => "vu",
        // 小書き (単独で出現するときの fallback)
        ('ぁ', _) => "a",
        ('ぃ', _) => "i",
        ('ぅ', _) => "u",
        ('ぇ', _) => "e",
        ('ぉ', _) => "o",
        ('ゃ', _) => "ya",
        ('ゅ', _) => "yu",
        ('ょ', _) => "yo",
        _ => return None,
    })
}

fn lookup_youon(c1: char, c2: char, style: RomajiStyle) -> Option<&'static str> {
    use RomajiStyle::{Hepburn, Kunrei};
    let small_y = matches!(c2, 'ゃ' | 'ゅ' | 'ょ');
    if !small_y {
        return None;
    }
    Some(match (c1, c2, style) {
        // き
        ('き', 'ゃ', _) => "kya",
        ('き', 'ゅ', _) => "kyu",
        ('き', 'ょ', _) => "kyo",
        ('ぎ', 'ゃ', _) => "gya",
        ('ぎ', 'ゅ', _) => "gyu",
        ('ぎ', 'ょ', _) => "gyo",
        // し
        ('し', 'ゃ', Hepburn) => "sha",
        ('し', 'ゅ', Hepburn) => "shu",
        ('し', 'ょ', Hepburn) => "sho",
        ('し', 'ゃ', Kunrei) => "sya",
        ('し', 'ゅ', Kunrei) => "syu",
        ('し', 'ょ', Kunrei) => "syo",
        ('じ', 'ゃ', Hepburn) => "ja",
        ('じ', 'ゅ', Hepburn) => "ju",
        ('じ', 'ょ', Hepburn) => "jo",
        ('じ', 'ゃ', Kunrei) => "zya",
        ('じ', 'ゅ', Kunrei) => "zyu",
        ('じ', 'ょ', Kunrei) => "zyo",
        // ち
        ('ち', 'ゃ', Hepburn) => "cha",
        ('ち', 'ゅ', Hepburn) => "chu",
        ('ち', 'ょ', Hepburn) => "cho",
        ('ち', 'ゃ', Kunrei) => "tya",
        ('ち', 'ゅ', Kunrei) => "tyu",
        ('ち', 'ょ', Kunrei) => "tyo",
        // に
        ('に', 'ゃ', _) => "nya",
        ('に', 'ゅ', _) => "nyu",
        ('に', 'ょ', _) => "nyo",
        // ひ
        ('ひ', 'ゃ', _) => "hya",
        ('ひ', 'ゅ', _) => "hyu",
        ('ひ', 'ょ', _) => "hyo",
        ('び', 'ゃ', _) => "bya",
        ('び', 'ゅ', _) => "byu",
        ('び', 'ょ', _) => "byo",
        ('ぴ', 'ゃ', _) => "pya",
        ('ぴ', 'ゅ', _) => "pyu",
        ('ぴ', 'ょ', _) => "pyo",
        // み
        ('み', 'ゃ', _) => "mya",
        ('み', 'ゅ', _) => "myu",
        ('み', 'ょ', _) => "myo",
        // り
        ('り', 'ゃ', _) => "rya",
        ('り', 'ゅ', _) => "ryu",
        ('り', 'ょ', _) => "ryo",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_hepburn() {
        assert_eq!(
            hiragana_to_romaji("はいざくら", RomajiStyle::Hepburn),
            "haizakura"
        );
        assert_eq!(hiragana_to_romaji("つき", RomajiStyle::Hepburn), "tsuki");
        assert_eq!(hiragana_to_romaji("ちず", RomajiStyle::Hepburn), "chizu");
        assert_eq!(
            hiragana_to_romaji("しゃしん", RomajiStyle::Hepburn),
            "shashin"
        );
        assert_eq!(
            hiragana_to_romaji("じゅっぷん", RomajiStyle::Hepburn),
            "juppun"
        );
    }

    #[test]
    fn basic_kunrei() {
        assert_eq!(hiragana_to_romaji("つき", RomajiStyle::Kunrei), "tuki");
        assert_eq!(
            hiragana_to_romaji("しゃしん", RomajiStyle::Kunrei),
            "syasin"
        );
        assert_eq!(
            hiragana_to_romaji("じゅっぷん", RomajiStyle::Kunrei),
            "zyuppun"
        );
    }

    #[test]
    fn sokuon() {
        // 促音
        assert_eq!(
            hiragana_to_romaji("がっこう", RomajiStyle::Hepburn),
            "gakkou"
        );
        assert_eq!(hiragana_to_romaji("もっち", RomajiStyle::Hepburn), "motchi");
        assert_eq!(hiragana_to_romaji("もっち", RomajiStyle::Kunrei), "motti");
    }

    #[test]
    fn hatsuon_n_before_bmp() {
        // 撥音: ヘボン式は b/m/p の前で m
        assert_eq!(
            hiragana_to_romaji("しんぶん", RomajiStyle::Hepburn),
            "shimbun"
        );
        assert_eq!(hiragana_to_romaji("さんぽ", RomajiStyle::Hepburn), "sampo");
        // 訓令式は常に n
        assert_eq!(
            hiragana_to_romaji("しんぶん", RomajiStyle::Kunrei),
            "sinbun"
        );
    }

    #[test]
    fn hatsuon_apostrophe() {
        // 母音 / y の前はアポストロフィ区切り (しん + あい != し + んあ + い 等)
        assert_eq!(
            hiragana_to_romaji("しんあい", RomajiStyle::Hepburn),
            "shin'ai"
        );
        assert_eq!(hiragana_to_romaji("ほんや", RomajiStyle::Hepburn), "hon'ya");
    }

    #[test]
    fn chouon() {
        // 長音 ー → 直前の母音を repeat (簡易、外来語拡張拗音 ぃ ぇ 等は単独扱い)
        assert_eq!(
            hiragana_to_romaji("らーめん", RomajiStyle::Hepburn),
            "raamen"
        );
        assert_eq!(
            hiragana_to_romaji("ぱーてぃー", RomajiStyle::Hepburn),
            "paateii"
        );
    }

    #[test]
    fn katakana_input_is_normalized() {
        // 入力がカタカナでも内部で hira 化されて動く
        assert_eq!(
            hiragana_to_romaji("ハイザクラ", RomajiStyle::Hepburn),
            "haizakura"
        );
    }
}
