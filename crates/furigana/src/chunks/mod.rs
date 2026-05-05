//! 数値テキスト全体のチャンク分割
//!
//! テキストを左から右に走査し、URL / 日付 / 時刻 / 数値 + 助数詞 /
//! 数値 + スケール / SI 単位 / 記号 / 素の数字 を**読み確定済みチャンク**として
//! 切り出す。読みが付かない部分は `None` のまま返し、呼び出し側で
//! 形態素解析パイプラインに委ねる。
//!
//! ## v0.1 でサポートするパターン (優先度順)
//! 1. URL / メール (skip)
//! 2. 和式日付 `YYYY年MM月DD日` / `MM月DD日`
//! 3. 和式時刻 `H時M分S秒` / `H時M分` / `H時`
//! 4. 時刻 `HH:MM(:SS)`
//! 5. 数値 + 大数スケール (+ 末尾助数詞) `3万円`
//! 6. SI 単位 `100km`
//! 7. 単一助数詞 `3本` / `1日` / `12月`
//! 8. 記号 1 文字
//! 9. 素の数字
//!
//! Phase 2 で追加予定: AM/PM 英語、第n四半期、温度、レンジ、分数、ヶ月、
//! ヶ所、n年(間|生|目|半|代)、人前/人分 等。

mod regex;

use self::regex::{
    at_start, build_counter_regex, build_scale_regex, build_si_unit_regex, merge_non_numeric,
    DATE_KANJI_FULL_RE, DATE_KANJI_MD_RE, DIGIT_RE, EMAIL_RE, TIME_COLON_RE, TIME_JP_FULL_RE,
    URL_RE,
};
use crate::numbers::{
    euphonic_counter_read, number_to_katakana, scale_reading, si_unit_reading, symbol_char_reading,
};
use crate::rules::{CountersData, DaysData, RulesData, ScalesData, SymbolsData, UnitsData};

/// 数値テキストのオーケストレーション (regex pre-compiled)
#[derive(Debug, Clone)]
pub struct NumberChunker {
    counters: CountersData,
    scales: ScalesData,
    units: UnitsData,
    symbols: SymbolsData,
    days: DaysData,

    /// 助数詞末尾を含むパターン (`(NUM)(月|日|時|分|...|本|匹|...)`)。
    /// `RulesData` が空 (counter エントリゼロ) なら `None`。
    counter_re: Option<::regex::Regex>,
    /// 大数スケール (`(NUM)(万|億|兆|...)`)。空なら `None`。
    scale_re: Option<::regex::Regex>,
    /// SI 単位 (`(NUM)(km|kg|...)`)。空なら `None`。
    si_unit_re: Option<::regex::Regex>,
}

impl NumberChunker {
    /// `RulesData` から regex を pre-compile
    #[must_use]
    pub fn new(rules: &RulesData) -> Self {
        let counter_re = build_counter_regex(&rules.counters);
        let scale_re = build_scale_regex(&rules.scales);
        let si_unit_re = build_si_unit_regex(&rules.units);

        Self {
            counters: rules.counters.clone(),
            scales: rules.scales.clone(),
            units: rules.units.clone(),
            symbols: rules.symbols.clone(),
            days: rules.days.clone(),
            counter_re,
            scale_re,
            si_unit_re,
        }
    }

    /// テキストを (表層, Option<読み>) のチャンク列に分割
    ///
    /// 読みが付いた部分は形態素解析に渡さない (確定済み)。
    /// 読みなしの隣接チャンクは `merge_non_numeric` で連結される。
    #[must_use]
    pub fn split(&self, text: &str) -> Vec<(String, Option<String>)> {
        let mut parts: Vec<(String, Option<String>)> = Vec::new();
        let len = text.len();
        let mut i = 0;

        while i < len {
            let rest = &text[i..];

            // ─── 1. URL / メール (skip、読みなし) ────────────────────────────
            if let Some(m) = URL_RE.find(rest) {
                if m.start() == 0 {
                    parts.push((m.as_str().to_string(), None));
                    i += m.end();
                    continue;
                }
            }
            if let Some(m) = EMAIL_RE.find(rest) {
                if m.start() == 0 {
                    parts.push((m.as_str().to_string(), None));
                    i += m.end();
                    continue;
                }
            }

            // ─── 2. 和式日付 ─────────────────────────────────────────────────
            if let Some(caps) = at_start(&DATE_KANJI_FULL_RE, rest) {
                let m_end = caps.get(0).unwrap().end();
                let y = caps.get(1).unwrap().as_str();
                let mo = caps.get(2).unwrap().as_str();
                let d = caps.get(3).unwrap().as_str();
                let surface = rest[..m_end].to_string();
                let reading = format!(
                    "{}{}{}",
                    self.read_counter(y, "年"),
                    self.read_counter(mo, "月"),
                    self.read_counter(d, "日")
                );
                parts.push((surface, Some(reading)));
                i += m_end;
                continue;
            }
            if let Some(caps) = at_start(&DATE_KANJI_MD_RE, rest) {
                let m_end = caps.get(0).unwrap().end();
                let mo = caps.get(1).unwrap().as_str();
                let d = caps.get(2).unwrap().as_str();
                let surface = rest[..m_end].to_string();
                let reading = format!(
                    "{}{}",
                    self.read_counter(mo, "月"),
                    self.read_counter(d, "日")
                );
                parts.push((surface, Some(reading)));
                i += m_end;
                continue;
            }

            // ─── 3. 和式時刻 (H時M分S秒 / H時M分 / H時) ─────────────────────
            if let Some(caps) = at_start(&TIME_JP_FULL_RE, rest) {
                let m_end = caps.get(0).unwrap().end();
                let h = caps.get(1).unwrap().as_str();
                let mo = caps.get(2).map(|m| m.as_str());
                let se = caps.get(3).map(|m| m.as_str());
                let surface = rest[..m_end].to_string();
                let mut reading = self.read_counter(h, "時");
                if let Some(m_str) = mo {
                    reading.push_str(&self.read_counter(m_str, "分"));
                }
                if let Some(s_str) = se {
                    reading.push_str(&self.read_counter(s_str, "秒"));
                }
                parts.push((surface, Some(reading)));
                i += m_end;
                continue;
            }

            // ─── 4. 時刻 HH:MM(:SS) ─────────────────────────────────────────
            if let Some(caps) = at_start(&TIME_COLON_RE, rest) {
                let m_end = caps.get(0).unwrap().end();
                let h = caps.get(1).unwrap().as_str();
                let mo = caps.get(2).unwrap().as_str();
                let se = caps.get(3).map(|m| m.as_str());
                let surface = rest[..m_end].to_string();
                let mut reading = self.read_counter(h, "時");
                reading.push_str(&self.read_counter(mo, "分"));
                if let Some(s_str) = se {
                    reading.push_str(&self.read_counter(s_str, "秒"));
                }
                parts.push((surface, Some(reading)));
                i += m_end;
                continue;
            }

            // ─── 5. 数値 + 大数スケール (+ 末尾助数詞) ─────────────────────
            if let Some(re) = &self.scale_re {
                if let Some(caps) = at_start(re, rest) {
                    let m_end = caps.get(0).unwrap().end();
                    let num = caps.get(1).unwrap().as_str();
                    let scale = caps.get(2).unwrap().as_str();
                    let surface = rest[..m_end].to_string();
                    let reading = scale_reading(num, scale, &self.scales);
                    parts.push((surface, Some(reading)));
                    i += m_end;
                    continue;
                }
            }

            // ─── 6. SI 単位 ─────────────────────────────────────────────────
            if let Some(re) = &self.si_unit_re {
                if let Some(caps) = at_start(re, rest) {
                    let m_end = caps.get(0).unwrap().end();
                    let num = caps.get(1).unwrap().as_str();
                    let unit = caps.get(2).unwrap().as_str();
                    let surface = rest[..m_end].to_string();
                    let reading = si_unit_reading(num, unit, &self.units);
                    parts.push((surface, Some(reading)));
                    i += m_end;
                    continue;
                }
            }

            // ─── 7. 単一助数詞 (3本, 5匹, 12月, 1日…) ─────────────────────
            if let Some(re) = &self.counter_re {
                if let Some(caps) = at_start(re, rest) {
                    let m_end = caps.get(0).unwrap().end();
                    let num = caps.get(1).unwrap().as_str();
                    let counter = caps.get(2).unwrap().as_str();
                    let surface = rest[..m_end].to_string();
                    let reading = self.read_counter(num, counter);
                    parts.push((surface, Some(reading)));
                    i += m_end;
                    continue;
                }
            }

            // ─── 8. 記号 1 文字 ─────────────────────────────────────────────
            let ch = rest.chars().next().expect("non-empty rest");
            if let Some(read) = symbol_char_reading(ch, &self.symbols) {
                parts.push((ch.to_string(), Some(read)));
                i += ch.len_utf8();
                continue;
            }

            // ─── 9. 素の数字 ────────────────────────────────────────────────
            if let Some(m) = at_start(&DIGIT_RE, rest) {
                let m_end = m.get(0).unwrap().end();
                let num = m.get(0).unwrap().as_str();
                parts.push((num.to_string(), Some(number_to_katakana(num))));
                i += m_end;
                continue;
            }

            // ─── 10. その他 (1 文字進める、読みなし) ───────────────────────
            parts.push((ch.to_string(), None));
            i += ch.len_utf8();
        }

        merge_non_numeric(parts)
    }

    /// 数値 + 助数詞 を読みに変換する内部ヘルパ
    fn read_counter(&self, raw_num: &str, counter: &str) -> String {
        let nk = number_to_katakana(raw_num);
        euphonic_counter_read(&nk, counter, raw_num, &self.counters, &self.days)
    }
}

/// `numbers::apply_numeric_overrides` を再エクスポート (chunker と同階層から呼びやすく)
pub use crate::numbers::apply_numeric_overrides;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load_rules_dir;
    use std::path::PathBuf;

    fn rules() -> RulesData {
        // 本体に rules を embed しないため、テスト用 fixture を使う。
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("rules");
        load_rules_dir(&dir).expect("load rules failed")
    }

    fn chunker() -> NumberChunker {
        NumberChunker::new(&rules())
    }

    fn find<'a>(
        parts: &'a [(String, Option<String>)],
        surface: &str,
    ) -> Option<&'a (String, Option<String>)> {
        parts.iter().find(|(s, _)| s == surface)
    }

    #[test]
    fn split_single_counter() {
        let c = chunker();
        let r = c.split("3本のバナナ");
        let m = find(&r, "3本").expect("missing 3本");
        assert_eq!(m.1.as_deref(), Some("サンボン"));
    }

    #[test]
    fn split_month_day_separately() {
        let c = chunker();
        let r = c.split("1月1日に集合");
        let m = find(&r, "1月1日").expect("missing 1月1日");
        let reading = m.1.as_deref().expect("no reading");
        assert!(reading.contains("イチガツ"), "reading: {reading}");
        assert!(reading.contains("ツイタチ"), "reading: {reading}");
    }

    #[test]
    fn split_date_full() {
        let c = chunker();
        let r = c.split("2025年10月30日");
        let m = find(&r, "2025年10月30日").expect("missing date");
        let reading = m.1.as_deref().expect("no reading");
        assert!(reading.contains("ジュウガツ"), "reading: {reading}");
    }

    #[test]
    fn split_time_colon() {
        let c = chunker();
        let r = c.split("9:30に集合");
        let m = find(&r, "9:30").expect("missing 9:30");
        let reading = m.1.as_deref().expect("no reading");
        assert!(reading.contains("クジ"), "reading: {reading}");
        assert!(reading.contains("サンジュッフン") || reading.contains("サンジュップン"));
    }

    #[test]
    fn split_time_jp() {
        let c = chunker();
        let r = c.split("9時30分");
        let m = find(&r, "9時30分").expect("missing 9時30分");
        let reading = m.1.as_deref().expect("no reading");
        assert!(reading.contains("クジ"), "reading: {reading}");
    }

    #[test]
    fn split_scale() {
        let c = chunker();
        let r = c.split("3万円のもの");
        let has_scale = r
            .iter()
            .any(|(s, read)| (s == "3万" || s == "3万円") && read.is_some());
        assert!(has_scale, "no scale match: {r:?}");
    }

    #[test]
    fn split_si_unit() {
        let c = chunker();
        let r = c.split("100km先");
        let m = find(&r, "100km").expect("missing 100km");
        let reading = m.1.as_deref().expect("no reading");
        assert!(reading.contains("ヒャク"), "reading: {reading}");
        assert!(reading.contains("キロメートル"), "reading: {reading}");
    }

    #[test]
    fn split_skips_url() {
        let c = chunker();
        let r = c.split("詳しくは https://example.com/100 を");
        let url_chunk = r.iter().find(|(s, _)| s.contains("example.com"));
        assert!(url_chunk.is_some());
    }

    #[test]
    fn split_symbol() {
        let c = chunker();
        let r = c.split("3+5");
        let plus = find(&r, "+").expect("missing +");
        assert_eq!(plus.1.as_deref(), Some("プラス"));
    }

    #[test]
    fn split_bare_digit() {
        let c = chunker();
        let r = c.split("番号は12345です");
        let m = find(&r, "12345").expect("missing 12345");
        assert!(m.1.is_some());
    }

    #[test]
    fn split_mixed() {
        let c = chunker();
        let r = c.split("3本と5匹");
        assert!(find(&r, "3本").is_some());
        assert!(find(&r, "5匹").is_some());
    }

    #[test]
    fn split_handles_full_width_digits() {
        let c = chunker();
        let r = c.split("３本");
        let m = find(&r, "３本").expect("missing ３本");
        assert!(m.1.is_some());
    }

    #[test]
    fn split_empty() {
        let c = chunker();
        let r = c.split("");
        assert!(r.is_empty());
    }
}
