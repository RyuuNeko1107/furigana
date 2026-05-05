//! `NumberChunker` で使う regex 群と小ヘルパ
//!
//! - **静的 regex**: URL / メール / 時刻 / 日付 / 素の数字 (テキスト依存しない)
//! - **動的 regex builders**: 助数詞 / スケール / SI 単位 (data 依存、ルールから build)
//! - **ヘルパ関数**: `at_start` (位置 0 マッチ判定) / `merge_non_numeric` (隣接 None 連結)

use crate::rules::{CountersData, ScalesData, UnitsData};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};

/// 数値パターン (符号付き、カンマ・小数対応)
pub(super) const NUM_PAT: &str =
    r"[+\-\u{2212}\u{FF0D}\u{FF0B}]?[0-9０-９]+(?:,[0-9０-９]{3})*(?:\.[0-9０-９]+)?";

/// 日付・月日用に「Arabic 数字または漢数字 (一〜九十、二十一 等)」を 1〜3 文字
/// マッチさせる pattern。NumberChunker の DATE_KANJI_*_RE で使われる。
pub(super) const DATE_NUM_PAT: &str = r"(?:[0-9０-９]{1,4}|[一二三四五六七八九十〇零]{1,3})";

// ─── 静的 regex ───────────────────────────────────────────────────────────

pub(super) static URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?xi)(?:(?:https?://|ftp://|file://|www\.)[^\s<>"'\(\)\{\}\[\]]+|(?:[A-Za-z0-9\-]+\.)+[A-Za-z]{2,}(?::\d+)?(?:/[^\s<>"'\(\)\{\}\[\]]*)?|\d{1,3}(?:\.\d{1,3}){3}(?::\d+)?(?:/[^\s<>"'\(\)\{\}\[\]]*)?)"#).unwrap()
});

pub(super) static EMAIL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap());

pub(super) static TIME_COLON_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([0-9０-９]{1,2})[:：]([0-9０-９]{2})(?:[:：]([0-9０-９]{2}))?").unwrap()
});

pub(super) static TIME_JP_FULL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([0-9０-９]{1,2})時(?:([0-9０-９]{1,2})分)?(?:([0-9０-９]{1,2})秒)?").unwrap()
});

pub(super) static DATE_KANJI_FULL_RE: Lazy<Regex> = Lazy::new(|| {
    let pat = format!(r"({DATE_NUM_PAT})年({DATE_NUM_PAT})月({DATE_NUM_PAT})日");
    Regex::new(&pat).unwrap()
});

pub(super) static DATE_KANJI_MD_RE: Lazy<Regex> = Lazy::new(|| {
    let pat = format!(r"({DATE_NUM_PAT})月({DATE_NUM_PAT})日");
    Regex::new(&pat).unwrap()
});

pub(super) static DIGIT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(NUM_PAT).unwrap());

// ─── 動的 builders (data 依存) ─────────────────────────────────────────────

/// 助数詞リストから regex を構築 (長い順でソート、prefix 衝突回避)。
/// 入力が空なら `None` を返す (split 側で if let Some で skip 可能に)。
pub(super) fn build_counter_regex(counters: &CountersData) -> Option<Regex> {
    let keys: Vec<String> = counters
        .simple
        .keys()
        .chain(counters.counter.keys())
        .cloned()
        .collect();
    build_alt_regex_opt(&keys, "counter")
}

/// 大数スケール regex (空なら `None`)
///
/// `units` を渡すと、scale 末尾に optional で漢字 1 文字 unit (円 / % など) を
/// 連結したパターンになる。これにより「1万円」のような scale + unit 連結が
/// 1 chunk として処理できる (Lindera が「1万」+「円」に切ると「円」単独の
/// reading が Lindera の訓読みに倒れる問題を回避)。
pub(super) fn build_scale_regex(scales: &ScalesData, units: &UnitsData) -> Option<Regex> {
    let kanjis: Vec<String> = scales.entries.iter().map(|e| e.kanji.clone()).collect();
    if kanjis.is_empty() {
        return None;
    }
    let mut sorted_scales = kanjis;
    sorted_scales.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
    let scale_alts: Vec<String> = sorted_scales.iter().map(|s| regex::escape(s)).collect();

    // units の中で「1 文字の漢字 (ASCII 以外)」を抽出。これらが scale 末尾に
    // 続く場合に optional で連結マッチさせる。「km / kg」のような ASCII unit は
    // scale 末尾に来ないので除外。
    let kanji_units: Vec<String> = units
        .entries
        .keys()
        .filter(|s| {
            s.chars().count() == 1 && s.chars().next().is_some_and(|c| !c.is_ascii_alphanumeric())
        })
        .map(|s| regex::escape(s))
        .collect();

    let pat = if kanji_units.is_empty() {
        format!(r"({NUM_PAT})({})", scale_alts.join("|"))
    } else {
        format!(
            r"({NUM_PAT})({})({})?",
            scale_alts.join("|"),
            kanji_units.join("|")
        )
    };

    Some(Regex::new(&pat).unwrap_or_else(|_| panic!("scale regex build failed")))
}

/// SI 単位 regex (case-insensitive: `1km` `1KM` `1Km` `1kM` 全て chunk 化)。
/// 空なら `None`。
///
/// 個別 entry の case 区別は [`UnitsData::lookup`] 側で `ci = false` を尊重
/// するため、ここでは regex 段で広めに拾って後段で絞る方針。
pub(super) fn build_si_unit_regex(units: &UnitsData) -> Option<Regex> {
    let symbols: Vec<String> = units.entries.keys().cloned().collect();
    if symbols.is_empty() {
        return None;
    }
    let mut sorted = symbols;
    sorted.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
    let alts: Vec<String> = sorted.iter().map(|s| regex::escape(s)).collect();
    let pat = format!(r"(?i)({NUM_PAT})({})", alts.join("|"));
    Some(Regex::new(&pat).unwrap_or_else(|_| panic!("si_unit regex build failed")))
}

/// `(NUM_PAT)(alt1|alt2|...)` 形式の regex を構築する。空 list なら `None` (split で skip)。
///
/// 旧実装は `r"(?P<n>\A\B)(?P<x>\A\B)"` という never-match pattern を返していたが、
/// release ビルドの `cargo test` harness で巨大 alloc 暴走を引き起こす shadowy bug
/// を伴ったため、空時は Option で表現することで根本回避する。
fn build_alt_regex_opt(items: &[String], label: &str) -> Option<Regex> {
    if items.is_empty() {
        return None;
    }
    let mut sorted = items.to_vec();
    sorted.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
    let alts: Vec<String> = sorted.iter().map(|s| regex::escape(s)).collect();
    let pat = format!(r"({NUM_PAT})({})", alts.join("|"));
    Some(Regex::new(&pat).unwrap_or_else(|_| panic!("{label} regex build failed")))
}

// ─── 共通ヘルパ ───────────────────────────────────────────────────────────

/// 開始位置 (start == 0) でマッチする場合のみ Captures を返す
pub(super) fn at_start<'h>(re: &Regex, hay: &'h str) -> Option<Captures<'h>> {
    re.captures(hay)
        .filter(|c| c.get(0).is_some_and(|m| m.start() == 0))
}

/// 隣接する読みなし (None) チャンクを連結
pub(super) fn merge_non_numeric(
    parts: Vec<(String, Option<String>)>,
) -> Vec<(String, Option<String>)> {
    let mut merged: Vec<(String, Option<String>)> = Vec::new();
    let mut buf = String::new();

    for (s, y) in parts {
        if y.is_none() {
            buf.push_str(&s);
        } else {
            if !buf.is_empty() {
                merged.push((std::mem::take(&mut buf), None));
            }
            merged.push((s, y));
        }
    }
    if !buf.is_empty() {
        merged.push((buf, None));
    }
    merged
}
