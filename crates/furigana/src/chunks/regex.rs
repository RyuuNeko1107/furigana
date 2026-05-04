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

pub(super) static DATE_KANJI_FULL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([0-9０-９]{1,4})年([0-9０-９]{1,2})月([0-9０-９]{1,2})日").unwrap());

pub(super) static DATE_KANJI_MD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([0-9０-９]{1,2})月([0-9０-９]{1,2})日").unwrap());

pub(super) static DIGIT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(NUM_PAT).unwrap());

// ─── 動的 builders (data 依存) ─────────────────────────────────────────────

/// 助数詞リストから regex を構築 (長い順でソート、prefix 衝突回避)
pub(super) fn build_counter_regex(counters: &CountersData) -> Regex {
    let keys: Vec<String> = counters
        .simple
        .keys()
        .chain(counters.counter.keys())
        .cloned()
        .collect();
    build_alt_regex(&keys, "counter")
}

/// 大数スケール regex
pub(super) fn build_scale_regex(scales: &ScalesData) -> Regex {
    let kanjis: Vec<String> = scales.entries.iter().map(|e| e.kanji.clone()).collect();
    build_alt_regex(&kanjis, "scale")
}

/// SI 単位 regex
pub(super) fn build_si_unit_regex(units: &UnitsData) -> Regex {
    let symbols: Vec<String> = units.entries.keys().cloned().collect();
    build_alt_regex(&symbols, "si_unit")
}

/// `(NUM_PAT)(alt1|alt2|...)` 形式の regex を構築する。
/// 空 list の場合は意図的にマッチしないパターンを返す (空 alternation は invalid)。
fn build_alt_regex(items: &[String], label: &str) -> Regex {
    let mut sorted = items.to_vec();
    sorted.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
    let alts: Vec<String> = sorted.iter().map(|s| regex::escape(s)).collect();
    let pat = if alts.is_empty() {
        // 絶対マッチしない (空 alternation 回避)
        r"(?P<n>\A\B)(?P<x>\A\B)".to_string()
    } else {
        format!(r"({NUM_PAT})({})", alts.join("|"))
    };
    Regex::new(&pat).unwrap_or_else(|_| panic!("{label} regex build failed"))
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
