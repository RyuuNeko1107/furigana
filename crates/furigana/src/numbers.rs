//! 数値処理 (data-driven)
//!
//! 旧 furigana_api_rust の `core/numbers.rs` を、`RulesData` を引数に取る
//! 形へ書き換えたもの。ルール表 (助数詞・スケール・記号・単位・慣用句) は
//! 全て [`crate::rules`] からロードされたデータを参照する。
//!
//! 主要 API:
//! - [`number_to_katakana`] : 数値文字列 → カタカナ
//! - [`euphonic_counter_read`] : 助数詞の連濁・促音化処理
//! - [`NumericPhraseMatcher`] : 慣用語句先行確定 (regex pre-compiled)
//!
//! `split_num_chunks` (全体オーケストレーション) は後続の `chunks` module で。

use crate::rules::{CountersData, DaysData, NumericPhrasesData, ScalesData, SymbolsData};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

// ============================================================================
// 0. 内部ヘルパー (正規化 / 文字列操作)
// ============================================================================

/// 全角英数字・記号 → 半角 (numbers モジュール内専用、kana::zen_to_han の縮小版)
fn zen2han(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
            '．' => '.',
            '，' => ',',
            '％' => '%',
            '＋' => '+',
            '－' | '\u{2212}' | '\u{2013}' | '\u{2014}' => '-',
            '〜' | '～' => '~',
            '／' => '/',
            _ => c,
        })
        .collect()
}

/// `zen2han` した上でカンマを除去
fn norm_num(s: &str) -> String {
    zen2han(s).replace(',', "")
}

/// 数値文字列 → i64 (全角・カンマ対応)
fn to_int(s: &str) -> Option<i64> {
    norm_num(s).parse::<i64>().ok()
}

/// 文字列末尾の数字 (1 桁)。数字が無ければ 0。
fn last_digit(s: &str) -> u32 {
    let norm = norm_num(s);
    for ch in norm.chars().rev() {
        if ch.is_ascii_digit() {
            return ch.to_digit(10).unwrap_or(0);
        }
    }
    0
}

/// 末尾を促音化 (イチ→イッ、ロク→ロッ、ハチ→ハッ、ジュウ→ジュッ)
fn sokuonize_last(num_kata: &str) -> String {
    for (src, dst) in &[
        ("イチ", "イッ"),
        ("ロク", "ロッ"),
        ("ハチ", "ハッ"),
        ("ジュウ", "ジュッ"),
    ] {
        if let Some(stripped) = num_kata.strip_suffix(src) {
            return format!("{stripped}{dst}");
        }
    }
    num_kata.to_string()
}

// ============================================================================
// 1. number_to_katakana
// ============================================================================

/// 0〜9999 のカタカナ読み
fn num_under_10000(n: u64) -> String {
    const ICHI: &[&str] = &[
        "",
        "イチ",
        "ニ",
        "サン",
        "ヨン",
        "ゴ",
        "ロク",
        "ナナ",
        "ハチ",
        "キュウ",
    ];
    const JUU: &[&str] = &[
        "",
        "ジュウ",
        "ニジュウ",
        "サンジュウ",
        "ヨンジュウ",
        "ゴジュウ",
        "ロクジュウ",
        "ナナジュウ",
        "ハチジュウ",
        "キュウジュウ",
    ];
    const HYAKU: &[&str] = &[
        "",
        "ヒャク",
        "ニヒャク",
        "サンビャク",
        "ヨンヒャク",
        "ゴヒャク",
        "ロッピャク",
        "ナナヒャク",
        "ハッピャク",
        "キュウヒャク",
    ];
    const SEN: &[&str] = &[
        "",
        "セン",
        "ニセン",
        "サンゼン",
        "ヨンセン",
        "ゴセン",
        "ロクセン",
        "ナナセン",
        "ハッセン",
        "キュウセン",
    ];

    let s = SEN[((n / 1000) % 10) as usize];
    let h = HYAKU[((n / 100) % 10) as usize];
    let j = JUU[((n / 10) % 10) as usize];
    let i = ICHI[(n % 10) as usize];
    format!("{s}{h}{j}{i}")
}

/// 大数スケール (10^4 刻み) — 万から澗 (10^36) まで。
/// 重要: u128::MAX ≈ 3.4×10^38 なので、これ以上のスケールは i/o しない。
const SCALE_UNITS: &[(&str, u128)] = &[
    ("カン", 10_u128.pow(36)),
    ("コウ", 10_u128.pow(32)),
    ("ジョウ", 10_u128.pow(28)),
    ("ジョ", 10_u128.pow(24)),
    ("ガイ", 10_u128.pow(20)),
    ("ケイ", 10_u128.pow(16)),
    ("チョウ", 10_u128.pow(12)),
    ("オク", 10_u128.pow(8)),
    ("マン", 10_u128.pow(4)),
];

/// 任意の数値文字列をカタカナ読みに変換
///
/// 全角→半角・カンマ除去・符号 (+/-/±)・小数点 (.) に対応。
/// 不正な形式は元の文字列を [`crate::kana::hira_to_kata`] したものを返す。
#[must_use]
pub fn number_to_katakana(num_str: &str) -> String {
    let s = norm_num(num_str);

    // ─── 符号処理 ──────────────────────────────────────────────────────────
    let (sign_read, s) = if let Some(rest) = s.strip_prefix('±') {
        ("プラスマイナス", rest.to_string())
    } else if let Some(rest) = s.strip_prefix('+') {
        ("プラス", rest.to_string())
    } else if let Some(rest) = s.strip_prefix('-') {
        ("マイナス", rest.to_string())
    } else {
        ("", s)
    };

    // ─── 不正な形式の早期 return ────────────────────────────────────────────
    let dot_count = s.chars().filter(|&c| c == '.').count();
    if dot_count > 1 || !s.replace('.', "").chars().all(|c| c.is_ascii_digit()) || s.is_empty() {
        let fallback = crate::kana::hira_to_kata(&zen2han(num_str));
        return format!("{sign_read}{fallback}");
    }

    // 先頭 . → 0. に補完
    let s = if s.starts_with('.') {
        format!("0{s}")
    } else {
        s
    };

    let (int_part, frac) = if let Some(idx) = s.find('.') {
        (&s[..idx], Some(&s[idx + 1..]))
    } else {
        (s.as_str(), None)
    };

    // ─── 整数部 ───────────────────────────────────────────────────────────
    let int_read = if int_part.is_empty() || int_part == "0" {
        "ゼロ".to_string()
    } else {
        let n: u128 = match int_part.parse() {
            Ok(v) => v,
            Err(_) => {
                let fallback = crate::kana::hira_to_kata(&zen2han(num_str));
                return format!("{sign_read}{fallback}");
            }
        };
        if n == 0 {
            "ゼロ".to_string()
        } else {
            let mut parts = Vec::new();
            let mut rem = n;
            for &(label, base) in SCALE_UNITS {
                let q = rem / base;
                rem %= base;
                if q > 0 {
                    parts.push(format!("{}{}", num_under_10000(q as u64), label));
                }
            }
            if rem > 0 {
                parts.push(num_under_10000(rem as u64));
            }
            if parts.is_empty() {
                "ゼロ".to_string()
            } else {
                parts.join("")
            }
        }
    };

    // ─── 小数部 ───────────────────────────────────────────────────────────
    let frac_read = match frac {
        Some(f) if !f.is_empty() => {
            const DIGIT: &[&str] = &[
                "ゼロ",
                "イチ",
                "ニ",
                "サン",
                "ヨン",
                "ゴ",
                "ロク",
                "ナナ",
                "ハチ",
                "キュウ",
            ];
            let digits: String = f
                .chars()
                .filter_map(|c| c.to_digit(10).map(|d| DIGIT[d as usize]))
                .collect::<Vec<_>>()
                .join("");
            format!("テン{digits}")
        }
        _ => String::new(),
    };

    format!("{sign_read}{int_read}{frac_read}")
}

// ============================================================================
// 2. euphonic_counter_read (data-driven)
// ============================================================================

/// 「数値 + 助数詞」の読みを構築する (data-driven)
///
/// 優先順位:
/// 1. counter が `simple` 表にあれば: `num_kata + simple[counter]`
/// 2. counter が `日` なら: days.toml の特殊読み (該当しなければ「ニチ」)
/// 3. counter が `counter` 表 (CounterRule) にあれば、specials → replacements
///    → rules (last_digit + sokuonize) → default の順で評価
/// 4. counter が末尾「目」を持つなら: base counter で再帰、最後に「メ」
/// 5. fallback: num_kata + counter (そのまま連結)
///
/// 0 は連濁・促音化を抑制する (例: ゼロ本 → ゼロホン)。
#[must_use]
pub fn euphonic_counter_read(
    num_kata: &str,
    counter: &str,
    raw_num: &str,
    counters: &CountersData,
    days: &DaysData,
) -> String {
    // 0 (ゼロ/レイ) は連濁・促音化対象外 — 99 を番兵として全ルールから外す
    let sd = if norm_num(raw_num) == "0" {
        99
    } else {
        last_digit(raw_num)
    };

    // ─── 1. simple 表 ──────────────────────────────────────────────────────
    if let Some(suffix) = counters.simple.get(counter) {
        return format!("{num_kata}{suffix}");
    }

    // ─── 2. 日: days.toml 優先 ─────────────────────────────────────────────
    if counter == "日" {
        if let Some(n) = to_int(raw_num) {
            if n >= 0 {
                if let Some(day) = days.get(n as u32) {
                    return day.to_string();
                }
            }
        }
        // days.toml に無い → counter."日".default → "ニチ" にフォールバック
        if let Some(rule) = counters.counter.get("日") {
            if let Some(default) = &rule.default {
                return format!("{num_kata}{default}");
            }
        }
        return format!("{num_kata}ニチ");
    }

    // ─── 3. counter 表 ─────────────────────────────────────────────────────
    if let Some(rule) = counters.counter.get(counter) {
        // 3a. recursive モードはここでは処理しない (呼び出し側責務)
        // recursive 助数詞単体は通常は呼ばれない。来たら fallback する。

        // 3b. 数値 specials (full override)
        let raw_normalized = norm_num(raw_num);
        if let Some(special) = rule.specials.get(&raw_normalized) {
            return special.clone();
        }

        // 3c. kana 末尾置換 (replacements)
        let mut adjusted_kana = num_kata.to_string();
        if sd != 99 {
            for repl in &rule.replacements {
                if repl.last_digit.contains(&sd) && adjusted_kana.ends_with(&repl.from) {
                    let cut = adjusted_kana.len() - repl.from.len();
                    adjusted_kana.truncate(cut);
                    adjusted_kana.push_str(&repl.to);
                    break;
                }
            }
        }

        // 3d. rules (last_digit + 連濁/促音化)
        if sd != 99 {
            for r in &rule.rules {
                if r.last_digit.contains(&sd) {
                    let body = if r.sokuonize {
                        sokuonize_last(&adjusted_kana)
                    } else {
                        adjusted_kana.clone()
                    };
                    return format!("{}{}", body, r.suffix);
                }
            }
        }

        // 3e. default
        if let Some(default) = &rule.default {
            return format!("{adjusted_kana}{default}");
        }
    }

    // ─── 4. 末尾「目」: 既存助数詞末尾の再帰モード ──────────────────────────
    if let Some(base_counter) = counter.strip_suffix('目') {
        if !base_counter.is_empty() {
            let base_read = euphonic_counter_read(num_kata, base_counter, raw_num, counters, days);
            return format!("{base_read}メ");
        }
    }

    // ─── 5. fallback ───────────────────────────────────────────────────────
    format!("{num_kata}{counter}")
}

// ============================================================================
// 3. NumericPhraseMatcher (慣用語句先行確定)
// ============================================================================

/// 慣用語句マッチャー (regex pre-compiled)
///
/// `apply` でテキストを (表層, Option<読み>) のチャンク列に分割する。
/// マッチした表層は読み確定 (Some)、間の文字列は読み未確定 (None)。
#[derive(Debug, Clone)]
pub struct NumericPhraseMatcher {
    regex: Option<Regex>,
    table: HashMap<String, String>,
}

impl NumericPhraseMatcher {
    /// `phrases` から正規表現を構築
    #[must_use]
    pub fn new(phrases: &NumericPhrasesData) -> Self {
        let table: HashMap<String, String> = phrases.entries.clone();

        let regex = if phrases.entries.is_empty() {
            None
        } else {
            // 長い表層を優先するため文字数降順ソート
            let mut surfaces: Vec<&str> = phrases.surfaces().collect();
            surfaces.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
            let alts: Vec<String> = surfaces.iter().map(|s| regex::escape(s)).collect();
            let pattern = format!("(?:{})", alts.join("|"));
            Regex::new(&pattern).ok()
        };

        Self { regex, table }
    }

    /// 空マッチャー (テスト・default 用)
    #[must_use]
    pub fn empty() -> Self {
        Self {
            regex: None,
            table: HashMap::new(),
        }
    }

    /// テキストを慣用句で分割し、(表層, Option<読み>) 列を返す
    #[must_use]
    pub fn apply(&self, text: &str) -> Vec<(String, Option<String>)> {
        let Some(regex) = &self.regex else {
            return vec![(text.to_string(), None)];
        };

        let mut parts: Vec<(String, Option<String>)> = Vec::new();
        let mut last_end = 0;

        for m in regex.find_iter(text) {
            if m.start() > last_end {
                parts.push((text[last_end..m.start()].to_string(), None));
            }
            let surf = m.as_str();
            let reading = self.table.get(surf).cloned();
            parts.push((surf.to_string(), reading));
            last_end = m.end();
        }

        if last_end < text.len() {
            parts.push((text[last_end..].to_string(), None));
        }

        if parts.is_empty() {
            vec![(text.to_string(), None)]
        } else {
            parts
        }
    }
}

/// 互換 API: phrases を引数に取って 1 回限り適用
#[must_use]
pub fn apply_numeric_overrides(
    text: &str,
    phrases: &NumericPhrasesData,
) -> Vec<(String, Option<String>)> {
    NumericPhraseMatcher::new(phrases).apply(text)
}

// ============================================================================
// 4. その他ユーティリティ
// ============================================================================

/// 記号 1 文字の読みを引く (全角/半角を正規化してから lookup)
#[must_use]
pub fn symbol_char_reading(ch: char, symbols: &SymbolsData) -> Option<String> {
    let normalized = match ch {
        '＋' => '+',
        '－' | '\u{2212}' => '-',
        '％' => '%',
        '／' => '/',
        _ => ch,
    };
    symbols.lookup_char(normalized).map(ToString::to_string)
}

/// SI 単位の読みを `units.toml` から引く
#[must_use]
pub fn si_unit_reading(num_str: &str, unit: &str, units: &crate::rules::UnitsData) -> String {
    let nk = number_to_katakana(num_str);
    let read = units.lookup(unit).map(str::to_string).unwrap_or_default();
    format!("{nk}{read}")
}

/// 大数スケール (万/億/兆…) 読み
#[must_use]
pub fn scale_reading(num_str: &str, scale: &str, scales: &ScalesData) -> String {
    let nk = number_to_katakana(num_str);
    let scale_kana = scales.lookup(scale).unwrap_or("");

    // 兆のみ末尾促音化 (1, 8, 0)
    let last = last_digit(num_str);
    let nk_adj = if scale == "兆" && matches!(last, 1 | 8 | 0) {
        sokuonize_last(&nk)
    } else {
        nk
    };

    format!("{nk_adj}{scale_kana}")
}

// ============================================================================
// regex 共通定数 (chunks module 追加時に使用)
// ============================================================================

/// 数値パターン (符号付き、カンマ・小数対応)
#[allow(dead_code)]
pub(crate) const NUM_PAT: &str =
    r"[+\-\u{2212}\u{FF0D}\u{FF0B}]?[0-9０-９]+(?:,[0-9０-９]{3})*(?:\.[0-9０-９]+)?";

/// レンジ区切り
#[allow(dead_code)]
pub(crate) const RANGE_SEP: &str = r"[~〜～\-\u{2212}\u{2013}\u{2014}]";

/// URL 検出 (split_num_chunks で skip 用)
#[allow(dead_code)]
pub(crate) static URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?xi)(?:(?:https?://|ftp://|file://|www\.)[^\s<>"'\(\)\{\}\[\]]+|(?:[A-Za-z0-9\-]+\.)+[A-Za-z]{2,}(?::\d+)?(?:/[^\s<>"'\(\)\{\}\[\]]*)?|\d{1,3}(?:\.\d{1,3}){3}(?::\d+)?(?:/[^\s<>"'\(\)\{\}\[\]]*)?)"#).unwrap()
});

/// メール検出 (split_num_chunks で skip 用)
#[allow(dead_code)]
pub(crate) static EMAIL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap());

// ============================================================================
// テスト
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{
        parse_counters_toml, parse_days_toml, parse_numeric_phrases_toml, parse_scales_toml,
        parse_symbols_toml, parse_units_toml,
    };

    // ─── helpers ──────────────────────────────────────────────────────────

    #[test]
    fn zen2han_works() {
        assert_eq!(zen2han("１２３"), "123");
        assert_eq!(zen2han("１，２３４"), "1,234");
        assert_eq!(zen2han("－５"), "-5");
    }

    #[test]
    fn norm_num_strips_commas() {
        assert_eq!(norm_num("1,234,567"), "1234567");
        assert_eq!(norm_num("１，２３４"), "1234");
    }

    #[test]
    fn to_int_handles_zenkaku() {
        assert_eq!(to_int("１２３"), Some(123));
        assert_eq!(to_int("-５"), Some(-5));
        assert_eq!(to_int("abc"), None);
    }

    #[test]
    fn last_digit_works() {
        assert_eq!(last_digit("123"), 3);
        assert_eq!(last_digit("100"), 0);
        assert_eq!(last_digit("1,234"), 4);
        assert_eq!(last_digit("abc"), 0);
    }

    #[test]
    fn sokuonize_last_works() {
        assert_eq!(sokuonize_last("イチ"), "イッ");
        assert_eq!(sokuonize_last("ロク"), "ロッ");
        assert_eq!(sokuonize_last("ハチ"), "ハッ");
        assert_eq!(sokuonize_last("ジュウ"), "ジュッ");
        assert_eq!(sokuonize_last("ニ"), "ニ"); // 該当なし
    }

    // ─── number_to_katakana ───────────────────────────────────────────────

    #[test]
    fn number_to_katakana_basic() {
        assert_eq!(number_to_katakana("0"), "ゼロ");
        assert_eq!(number_to_katakana("1"), "イチ");
        assert_eq!(number_to_katakana("10"), "ジュウ");
        assert_eq!(number_to_katakana("100"), "ヒャク");
        assert_eq!(number_to_katakana("300"), "サンビャク");
        assert_eq!(number_to_katakana("600"), "ロッピャク");
        assert_eq!(number_to_katakana("800"), "ハッピャク");
        assert_eq!(number_to_katakana("1000"), "セン");
        assert_eq!(number_to_katakana("3000"), "サンゼン");
        assert_eq!(number_to_katakana("8000"), "ハッセン");
    }

    #[test]
    fn number_to_katakana_large() {
        assert_eq!(number_to_katakana("10000"), "イチマン");
        assert_eq!(number_to_katakana("100000000"), "イチオク");
        // number_to_katakana 単体では兆の促音化は適用しない
        // (兆は scale_reading 側で「イッチョウ」になる)
        assert_eq!(number_to_katakana("1000000000000"), "イチチョウ");
    }

    #[test]
    fn number_to_katakana_decimal() {
        assert_eq!(number_to_katakana("3.14"), "サンテンイチヨン");
        assert_eq!(number_to_katakana(".5"), "ゼロテンゴ");
    }

    #[test]
    fn number_to_katakana_signs() {
        assert_eq!(number_to_katakana("-10"), "マイナスジュウ");
        assert_eq!(number_to_katakana("+5"), "プラスゴ");
        assert_eq!(number_to_katakana("±3"), "プラスマイナスサン");
    }

    #[test]
    fn number_to_katakana_fullwidth() {
        assert_eq!(number_to_katakana("１２３"), "ヒャクニジュウサン");
    }

    #[test]
    fn number_to_katakana_with_commas() {
        assert_eq!(number_to_katakana("1,234"), "センニヒャクサンジュウヨン");
    }

    // ─── euphonic_counter_read ────────────────────────────────────────────

    fn load_counters() -> CountersData {
        let raw = include_str!("../tests/fixtures/rules/counters.toml");
        parse_counters_toml(raw, "counters.toml").unwrap()
    }

    fn load_days() -> DaysData {
        let raw = include_str!("../tests/fixtures/rules/days.toml");
        parse_days_toml(raw, "days.toml").unwrap()
    }

    #[test]
    fn euphonic_basic_hon() {
        let c = load_counters();
        let d = load_days();
        assert_eq!(euphonic_counter_read("イチ", "本", "1", &c, &d), "イッポン");
        assert_eq!(euphonic_counter_read("サン", "本", "3", &c, &d), "サンボン");
        assert_eq!(euphonic_counter_read("ニ", "本", "2", &c, &d), "ニホン");
    }

    #[test]
    fn euphonic_fun() {
        let c = load_counters();
        let d = load_days();
        assert_eq!(euphonic_counter_read("イチ", "分", "1", &c, &d), "イップン");
        assert_eq!(euphonic_counter_read("ニ", "分", "2", &c, &d), "ニフン");
        assert_eq!(euphonic_counter_read("サン", "分", "3", &c, &d), "サンプン");
    }

    #[test]
    fn euphonic_person_specials() {
        let c = load_counters();
        let d = load_days();
        assert_eq!(euphonic_counter_read("イチ", "人", "1", &c, &d), "ヒトリ");
        assert_eq!(euphonic_counter_read("ニ", "人", "2", &c, &d), "フタリ");
        assert_eq!(euphonic_counter_read("サン", "人", "3", &c, &d), "サンニン");
    }

    #[test]
    fn euphonic_day_uses_days_table() {
        let c = load_counters();
        let d = load_days();
        assert_eq!(euphonic_counter_read("イチ", "日", "1", &c, &d), "ツイタチ");
        assert_eq!(euphonic_counter_read("ニ", "日", "2", &c, &d), "フツカ");
        assert_eq!(
            euphonic_counter_read("ニジュウ", "日", "20", &c, &d),
            "ハツカ"
        );
    }

    #[test]
    fn euphonic_hour_replacements() {
        let c = load_counters();
        let d = load_days();
        assert_eq!(euphonic_counter_read("ヨン", "時", "4", &c, &d), "ヨジ");
        assert_eq!(euphonic_counter_read("ナナ", "時", "7", &c, &d), "シチジ");
        assert_eq!(euphonic_counter_read("キュウ", "時", "9", &c, &d), "クジ");
        assert_eq!(
            euphonic_counter_read("ジュウヨン", "時", "14", &c, &d),
            "ジュウヨジ"
        );
        assert_eq!(euphonic_counter_read("ゼロ", "時", "0", &c, &d), "レイジ");
    }

    #[test]
    fn euphonic_month_specials() {
        let c = load_counters();
        let d = load_days();
        assert_eq!(euphonic_counter_read("ヨン", "月", "4", &c, &d), "シガツ");
        assert_eq!(euphonic_counter_read("ナナ", "月", "7", &c, &d), "シチガツ");
        assert_eq!(euphonic_counter_read("キュウ", "月", "9", &c, &d), "クガツ");
        assert_eq!(euphonic_counter_read("イチ", "月", "1", &c, &d), "イチガツ");
    }

    #[test]
    fn euphonic_recursive_me() {
        let c = load_counters();
        let d = load_days();
        // 「3回目」→ サンカイメ (回 last_digit=3 で「カイ」→ "回" は ci 3 なし → default カイ)
        assert_eq!(
            euphonic_counter_read("サン", "回目", "3", &c, &d),
            "サンカイメ"
        );
        // 「2人目」→ フタリメ (人 special 2 → フタリ + メ)
        assert_eq!(euphonic_counter_read("ニ", "人目", "2", &c, &d), "フタリメ");
    }

    #[test]
    fn euphonic_zero_no_sokuon() {
        let c = load_counters();
        let d = load_days();
        // 0 は連濁・促音化対象外
        assert_eq!(euphonic_counter_read("ゼロ", "本", "0", &c, &d), "ゼロホン");
        assert_eq!(euphonic_counter_read("ゼロ", "匹", "0", &c, &d), "ゼロヒキ");
        assert_eq!(euphonic_counter_read("ゼロ", "杯", "0", &c, &d), "ゼロハイ");
        assert_eq!(euphonic_counter_read("ゼロ", "分", "0", &c, &d), "ゼロフン");
        assert_eq!(euphonic_counter_read("ゼロ", "回", "0", &c, &d), "ゼロカイ");
    }

    #[test]
    fn euphonic_jujji_sokuon() {
        let c = load_counters();
        let d = load_days();
        // 10+助数詞は促音化する
        assert_eq!(
            euphonic_counter_read("ジュウ", "本", "10", &c, &d),
            "ジュッポン"
        );
    }

    #[test]
    fn euphonic_simple_suffix() {
        let c = load_counters();
        let d = load_days();
        assert_eq!(
            euphonic_counter_read("ヒャク", "円", "100", &c, &d),
            "ヒャクエン"
        );
        assert_eq!(
            euphonic_counter_read("イチ", "ヶ月", "1", &c, &d),
            "イチカゲツ"
        );
    }

    #[test]
    fn euphonic_unknown_counter_passthrough() {
        let c = load_counters();
        let d = load_days();
        // 未知の助数詞は num_kata + counter の素朴連結
        assert_eq!(euphonic_counter_read("イチ", "謎", "1", &c, &d), "イチ謎");
    }

    // ─── NumericPhraseMatcher ─────────────────────────────────────────────

    fn load_phrases() -> NumericPhrasesData {
        let raw = include_str!("../tests/fixtures/rules/numeric_phrases.toml");
        parse_numeric_phrases_toml(raw, "numeric_phrases.toml").unwrap()
    }

    #[test]
    fn phrase_match_hatachi() {
        let p = load_phrases();
        let m = NumericPhraseMatcher::new(&p);
        let result = m.apply("二十歳になった");
        assert!(result
            .iter()
            .any(|(s, r)| s == "二十歳" && r.as_deref() == Some("ハタチ")));
    }

    #[test]
    fn phrase_match_multiple() {
        let p = load_phrases();
        let m = NumericPhraseMatcher::new(&p);
        let result = m.apply("明後日と一昨日");
        assert!(result
            .iter()
            .any(|(s, r)| s == "明後日" && r.as_deref() == Some("アサッテ")));
        assert!(result
            .iter()
            .any(|(s, r)| s == "一昨日" && r.as_deref() == Some("オトトイ")));
    }

    #[test]
    fn phrase_no_match_passthrough() {
        let p = load_phrases();
        let m = NumericPhraseMatcher::new(&p);
        let result = m.apply("こんにちは");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ("こんにちは".to_string(), None));
    }

    #[test]
    fn phrase_empty_matcher() {
        let m = NumericPhraseMatcher::empty();
        let result = m.apply("test");
        assert_eq!(result, vec![("test".to_string(), None)]);
    }

    #[test]
    fn phrase_longer_match_wins() {
        let p = load_phrases();
        let m = NumericPhraseMatcher::new(&p);
        // "一人前" は "一人" よりも先に確定する (長いものを優先)
        let result = m.apply("一人前");
        assert!(result
            .iter()
            .any(|(s, r)| s == "一人前" && r.as_deref() == Some("イチニンマエ")));
    }

    // ─── scale / si_unit / symbol ─────────────────────────────────────────

    #[test]
    fn scale_reading_basic() {
        let raw = include_str!("../tests/fixtures/rules/scales.toml");
        let scales = parse_scales_toml(raw, "scales.toml").unwrap();
        assert_eq!(scale_reading("3", "万", &scales), "サンマン");
        assert_eq!(scale_reading("1", "兆", &scales), "イッチョウ");
        assert_eq!(scale_reading("8", "兆", &scales), "ハッチョウ");
        assert_eq!(scale_reading("2", "兆", &scales), "ニチョウ"); // 連濁なし
    }

    #[test]
    fn si_unit_reading_basic() {
        let raw = include_str!("../tests/fixtures/rules/units.toml");
        let units = parse_units_toml(raw, "units.toml").unwrap();
        assert_eq!(si_unit_reading("100", "km", &units), "ヒャクキロメートル");
        assert_eq!(si_unit_reading("3", "L", &units), "サンリットル");
    }

    #[test]
    fn symbol_char_reading_basic() {
        let raw = include_str!("../tests/fixtures/rules/symbols.toml");
        let symbols = parse_symbols_toml(raw, "symbols.toml").unwrap();
        assert_eq!(
            symbol_char_reading('+', &symbols).as_deref(),
            Some("プラス")
        );
        assert_eq!(
            symbol_char_reading('％', &symbols).as_deref(),
            Some("パーセント")
        );
        assert_eq!(symbol_char_reading('a', &symbols), None);
    }
}
