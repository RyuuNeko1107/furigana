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
use crate::loanwords::Loanwords;
use crate::numbers::{
    euphonic_counter_read, kansuji_to_arabic, number_to_katakana, scale_reading, si_unit_reading,
    symbol_char_reading,
};
use crate::rules::{CountersData, DaysData, RulesData, ScalesData, SymbolsData, UnitsData};
use aho_corasick::AhoCorasick;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;

/// 英単語 chunk pattern: ASCII / 全角 英字 (大小) で開始し、 英数字 + 記号を許容。
///
/// IT 用語の代表的な綴り (例: 「C++」 「C#」 「.NET」 「TypeScript-config」 「node_modules」)
/// を 1 chunk として丸ごと切り出すための regex。 全角英数字 + 全角記号も拾い、
/// loanwords.normalize() 側で半角化 + case-fold して lookup する。 文頭 anchor は
/// 呼び出し側で `at_start` を通して位置 0 でしかマッチさせないため、 ここでは付けない。
///
/// 注意: 「.NET」 のような pattern 先頭が記号で始まる surface には match しない
/// (英字始まりに限定)。 そういう surface は loanwords entry 側で「DotNET」 等の
/// 先頭文字付き alias を登録する必要がある (今回 scope 外)。
static LOANWORD_RE: Lazy<::regex::Regex> = Lazy::new(|| {
    // character class 内のハイフンは順序事故を避けるため最後尾に置く。
    // 全角ハイフン「－」 も含めるが、 こちらも全角記号群の最後尾に配置。
    ::regex::Regex::new(r"[A-Za-zＡ-Ｚａ-ｚ][A-Za-z0-9Ａ-Ｚａ-ｚ０-９+#._＋＃．＿\-－]*").unwrap()
});

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

    /// 熟語 (jukugo) の Aho-Corasick automaton。
    ///
    /// counter / scale が match した範囲の真上位集合となる jukugo entry が
    /// 存在するか調べ、 あれば counter / scale を破棄して jukugo を採用する
    /// (例: 「千本桜」 で 「千本」 が counter にマッチするが jukugo
    /// 「千本桜」 がある場合、 「千本桜」 全体を 1 chunk に固定)。
    ///
    /// `None` の場合は何もしない (heteronym bypass 等の副作用ゼロで既存挙動)。
    jukugo_ac: Option<Arc<AhoCorasick>>,
    jukugo_map: Option<Arc<HashMap<String, String>>>,

    /// 外来語 (loanwords) 辞書。 IT 用語の英単語等を case-insensitive 完全一致で
    /// lookup する。 `chunks/split()` の階層 4.7 で英単語 chunk を切り出した後、
    /// chunk 全体に対して lookup する (substring 切断ゼロ)。
    ///
    /// `None` の場合は英単語 chunk 自体は切り出すが loanwords lookup を skip し、
    /// ASCII surface のまま読みなしで返す (Lindera 経路に渡らないので IPADIC 誤読も回避)。
    loanwords: Option<Arc<Loanwords>>,
}

impl NumberChunker {
    /// `RulesData` から regex を pre-compile
    #[must_use]
    pub fn new(rules: &RulesData) -> Self {
        let counter_re = build_counter_regex(&rules.counters);
        let scale_re = build_scale_regex(&rules.scales, &rules.units);
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
            jukugo_ac: None,
            jukugo_map: None,
            loanwords: None,
        }
    }

    /// 外来語辞書を注入する (起動時 1 回、 Furigana::build() から)
    ///
    /// `chunks/split()` の階層 4.7 (jukugo prefix-match の後) で
    /// 英単語 chunk を 1 unit として切り出し、 chunk 全体に対して完全一致 lookup する。
    pub fn set_loanwords(&mut self, loanwords: Arc<Loanwords>) {
        self.loanwords = Some(loanwords);
    }

    /// jukugo の Aho-Corasick automaton を注入する (起動時 1 回、 phrase_matcher と Arc 共有想定)
    ///
    /// `Furigana::build()` 側で 1 回 build した AC を chunker と phrase_matcher の両方に
    /// Arc で渡す。 homonyms (context rule を持つ surface) は呼び出し側で予め除外済み。
    pub fn set_jukugo(&mut self, ac: Arc<AhoCorasick>, map: Arc<HashMap<String, String>>) {
        self.jukugo_ac = Some(ac);
        self.jukugo_map = Some(map);
    }

    /// `rest` (テキストの現在位置以降) の文頭から match する jukugo の最長 surface 長と reading を返す
    ///
    /// 文頭からの最長一致が見つからない場合は `None`。
    fn match_jukugo_at_start(&self, rest: &str) -> Option<(usize, String)> {
        let ac = self.jukugo_ac.as_ref()?;
        let map = self.jukugo_map.as_ref()?;
        let mat = ac.find(rest)?;
        if mat.start() != 0 {
            return None;
        }
        let surface = &rest[..mat.end()];
        let reading = map.get(surface)?.clone();
        Some((mat.end(), reading))
    }

    /// `rest` の文頭から match する jukugo を、 `min_end_bytes` より厳密に長い場合のみ返す
    ///
    /// counter / scale が確定した範囲を真に含む (= longer end) jukugo entry がある場合に
    /// jukugo を優先採用するための super-set 判定。 「3千本のバラ」 のような scale 確定 case で
    /// jukugo に「千本」 entry があっても (短いから) 誤って override しないよう strict check。
    fn match_jukugo_strict_super(
        &self,
        rest: &str,
        min_end_bytes: usize,
    ) -> Option<(usize, String)> {
        let (end, reading) = self.match_jukugo_at_start(rest)?;
        if end <= min_end_bytes {
            return None;
        }
        Some((end, reading))
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
            // 日付内では「日」counter は days.toml の特殊読み (1=ツイタチ 等) を採用
            if let Some(caps) = at_start(&DATE_KANJI_FULL_RE, rest) {
                let m_end = caps.get(0).unwrap().end();
                let y = caps.get(1).unwrap().as_str();
                let mo = caps.get(2).unwrap().as_str();
                let d = caps.get(3).unwrap().as_str();
                let surface = rest[..m_end].to_string();
                let reading = format!(
                    "{}{}{}",
                    self.read_counter_in_date(y, "年"),
                    self.read_counter_in_date(mo, "月"),
                    self.read_counter_in_date(d, "日")
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
                    self.read_counter_in_date(mo, "月"),
                    self.read_counter_in_date(d, "日")
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

            // ─── 4.5. jukugo 先取り (counter/scale より前に固有複合語を救う) ──
            // 「千本桜」「義経千本桜」 のように Lindera が token 境界を切ってしまう
            // 結果 jukugo lookup が走らない複合語を、 文頭からの最長 match で先取り
            // して 1 chunk に固定する。 heteronym (homonyms.toml で context rule を
            // 持つ surface) は set_jukugo の exclude_surfaces で除外済みなので、
            // reading pipeline の context rule (例: 「翡翠+が+水辺」 → カワセミ) は
            // ここで bypass されない。
            if let Some((j_end, j_reading)) = self.match_jukugo_at_start(rest) {
                let surface = rest[..j_end].to_string();
                parts.push((surface, Some(j_reading)));
                i += j_end;
                continue;
            }

            // ─── 4.7. 英単語 chunk + loanwords lookup ─────────────────────────
            // ASCII 英字始まりの連続 (英数字 + 記号 +#._-) を 1 chunk として丸ごと
            // 切り出す。 Lindera/IPADIC が英単語を token 単位でぶった切るのを防ぐのが
            // 主目的 (例: 「PostgreSQL」 を 「Post」 + 「greS」 + 「QL」 等に分解されない)。
            //
            // chunk 全体に対して loanwords を **完全一致** で lookup:
            //   - hit → reading 確定 chunk として切り出し
            //   - miss → ASCII surface のまま読みなしで切り出し (Some/None 両方とも
            //     1 chunk として確定 → Lindera 経路に渡さず誤読を防止)
            if let Some(m) = at_start(&LOANWORD_RE, rest) {
                let m_end = m.get(0).unwrap().end();
                let surface = rest[..m_end].to_string();
                let reading = self
                    .loanwords
                    .as_ref()
                    .and_then(|d| d.lookup(&surface).map(String::from));
                parts.push((surface, reading));
                i += m_end;
                continue;
            }

            // ─── 5. 数値 + 大数スケール (+ 末尾漢字単位) ─────────────────────
            // 「1万円」「3億ドル」のような scale + 漢字 1 文字 unit を 1 chunk に。
            // 漢字 unit は build_scale_regex で optional capture (3) として注入済み。
            if let Some(re) = &self.scale_re {
                if let Some(caps) = at_start(re, rest) {
                    let m_end = caps.get(0).unwrap().end();
                    // jukugo super-set check: scale match を真に含む jukugo entry が
                    // あれば jukugo を優先 (例: 「億万長者」 が jukugo にある場合に
                    // scale 「億万」 で分断されるのを回避)
                    if let Some((j_end, j_reading)) = self.match_jukugo_strict_super(rest, m_end) {
                        let surface = rest[..j_end].to_string();
                        parts.push((surface, Some(j_reading)));
                        i += j_end;
                        continue;
                    }
                    let num = caps.get(1).unwrap().as_str();
                    let scale = caps.get(2).unwrap().as_str();
                    let trailing_unit = caps.get(3).map(|m| m.as_str());
                    let surface = rest[..m_end].to_string();
                    let mut reading = scale_reading(num, scale, &self.scales);
                    if let Some(u) = trailing_unit {
                        if let Some(unit_kana) = self.units.lookup(u) {
                            reading.push_str(unit_kana);
                        } else {
                            // units にあるはずだが lookup 失敗時は surface をそのまま
                            reading.push_str(u);
                        }
                    }
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
                    // jukugo super-set check: counter match を真に含む jukugo entry が
                    // あれば jukugo を優先 (例: 「千本桜」 で 「千本」 が counter に
                    // hit するが jukugo 「千本桜 = センボンザクラ」 がある場合に
                    // 全体を 1 chunk に固定して連濁を救う)
                    if let Some((j_end, j_reading)) = self.match_jukugo_strict_super(rest, m_end) {
                        let surface = rest[..j_end].to_string();
                        parts.push((surface, Some(j_reading)));
                        i += j_end;
                        continue;
                    }
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

    /// 数値 + 助数詞 を読みに変換する (**単独 counter 用**、期間文脈)
    ///
    /// raw_num は Arabic 数字 / 全角数字 / 漢数字 (一〜二十一 程度) を許容。
    /// 漢数字は内部で Arabic に変換してから [`euphonic_counter_read`] に渡す。
    ///
    /// **「N日」の特殊扱い**: 単独で出てきた「N日」は **期間文脈** とみなし、
    /// days.toml の特殊読み (1=ツイタチ、2=フツカ等) を bypass して default
    /// 「Nニチ」にする。暦の「N日」(月日付き) は [`Self::read_counter_in_date`]
    /// 経由で日付として処理されるので、こちらでは bypass で OK。
    /// 例: 「1日に2〜3回」の「1日」は単独 counter で chunker.split に来るので
    /// 「イチニチ」になる。「6月1日に集合」の「1日」は DATE_KANJI_MD_RE 経由で
    /// `read_counter_in_date` に行き「ツイタチ」になる。
    fn read_counter(&self, raw_num: &str, counter: &str) -> String {
        let normalized = kansuji_to_arabic(raw_num).unwrap_or_else(|| raw_num.to_string());
        let nk = number_to_katakana(&normalized);

        // 「N日」単独は期間扱い: days.toml 特殊読みを bypass
        if counter == "日" {
            if let Some(rule) = self.counters.counter.get("日") {
                if let Some(default) = &rule.default {
                    return format!("{nk}{default}");
                }
            }
            return format!("{nk}ニチ");
        }

        euphonic_counter_read(&nk, counter, &normalized, &self.counters, &self.days)
    }

    /// 数値 + 助数詞 を読みに変換する (**日付内用**)
    ///
    /// 「N年M月D日」のように日付パターンとしてマッチした内部で呼ばれる。
    /// 「日」counter は days.toml の特殊読み (1=ツイタチ 等) を採用する。
    fn read_counter_in_date(&self, raw_num: &str, counter: &str) -> String {
        let normalized = kansuji_to_arabic(raw_num).unwrap_or_else(|| raw_num.to_string());
        let nk = number_to_katakana(&normalized);
        euphonic_counter_read(&nk, counter, &normalized, &self.counters, &self.days)
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

    fn build_ac(entries: &[(&str, &str)]) -> (Arc<AhoCorasick>, Arc<HashMap<String, String>>) {
        let map: HashMap<String, String> = entries
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        let ac = AhoCorasick::builder()
            .match_kind(aho_corasick::MatchKind::LeftmostLongest)
            .build(map.keys())
            .expect("AC build");
        (Arc::new(ac), Arc::new(map))
    }

    /// jukugo prefix-match: 「千本桜」 (漢数字 prefix で counter_re が走らない) を
    /// 文頭の AC で先取りして 1 chunk に固定する。
    #[test]
    fn split_jukugo_prefix_match() {
        let mut c = chunker();
        let (ac, map) = build_ac(&[("千本桜", "センボンザクラ")]);
        c.set_jukugo(ac, map);
        let r = c.split("千本桜");
        let m = find(&r, "千本桜").expect("expected jukugo chunk for 千本桜");
        assert_eq!(m.1.as_deref(), Some("センボンザクラ"));
    }

    /// 呼び出し側が exclude 済み AC を渡すケース (homonyms surface を含まない AC):
    /// AC 内に entry が無いので、 ここで chunks 段階の確定はされず形態素解析へ流れる。
    #[test]
    fn split_jukugo_excludes_homonym() {
        let mut c = chunker();
        // 「翡翠」 を含まない AC を渡す (api.rs 側で exclude 済みの想定)
        let (ac, map) = build_ac(&[("千本桜", "センボンザクラ")]);
        c.set_jukugo(ac, map);
        let r = c.split("翡翠");
        let m = find(&r, "翡翠").expect("expected raw 翡翠 chunk");
        assert!(m.1.is_none(), "homonym 翡翠 should pass through: {m:?}");
    }

    /// jukugo に該当 entry が無ければ counter は普通に動く (副作用ゼロ確認)。
    #[test]
    fn split_counter_unchanged_when_no_jukugo() {
        let mut c = chunker();
        let (ac, map) = build_ac(&[("無関係", "ムカンケイ")]);
        c.set_jukugo(ac, map);
        let r = c.split("3本のバナナ");
        let m = find(&r, "3本").expect("counter chunk for 3本 should still fire");
        assert_eq!(m.1.as_deref(), Some("サンボン"));
    }

    fn loanwords_with(entries: &[(&str, &str)]) -> Arc<Loanwords> {
        let mut d = Loanwords::default();
        for (k, v) in entries {
            d.insert(*k, *v);
        }
        Arc::new(d)
    }

    /// 階層 4.7: loanwords lookup hit → reading 確定 chunk として 1 unit に切り出し
    #[test]
    fn split_loanword_hit() {
        let mut c = chunker();
        c.set_loanwords(loanwords_with(&[("Kubernetes", "クバネティス")]));
        let r = c.split("Kubernetesが安定");
        let m = find(&r, "Kubernetes").expect("loanword chunk");
        assert_eq!(m.1.as_deref(), Some("クバネティス"));
    }

    /// 階層 4.7: case-fold + 全角→半角 で hit
    #[test]
    fn split_loanword_normalization() {
        let mut c = chunker();
        c.set_loanwords(loanwords_with(&[("Kubernetes", "クバネティス")]));
        // 全角 + 大文字混在
        let r = c.split("ＫＵＢＥＲＮＥＴＥＳ環境");
        let m = find(&r, "ＫＵＢＥＲＮＥＴＥＳ").expect("loanword chunk");
        assert_eq!(m.1.as_deref(), Some("クバネティス"));
    }

    /// 階層 4.7: loanwords miss → ASCII surface のまま読みなしで残す (Lindera 経路に渡らない)
    #[test]
    fn split_loanword_miss_stays_raw() {
        let mut c = chunker();
        c.set_loanwords(loanwords_with(&[("Kubernetes", "クバネティス")]));
        // 単独入力 (周囲に non-loanword chunk が無い): loanword が miss でも
        // ASCII chunk として 1 unit に切り出され、 reading=None で残る
        let r = c.split("UnknownTechName");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, "UnknownTechName");
        assert!(r[0].1.is_none(), "miss should leave reading=None: {r:?}");
    }

    /// 階層 4.7: 完全一致のみ — substring に部分一致しても採用しない
    #[test]
    fn split_loanword_no_substring_match() {
        let mut c = chunker();
        c.set_loanwords(loanwords_with(&[("Post", "ポスト")]));
        let r = c.split("PostgreSQL");
        let m = find(&r, "PostgreSQL").expect("chunk for PostgreSQL");
        // 「Post」 entry はあるが「PostgreSQL」 全体には無いので reading=None で残る
        assert!(m.1.is_none(), "substring match should NOT fire: {m:?}");
    }

    /// 階層 4.7: 記号 (+ # . - _) を含む surface もカバー
    #[test]
    fn split_loanword_with_symbols() {
        let mut c = chunker();
        c.set_loanwords(loanwords_with(&[
            ("C++", "シープラスプラス"),
            ("node_modules", "ノードモジュールス"),
            ("TypeScript-config", "タイプスクリプトコンフィグ"),
        ]));
        for (input, expected) in [
            ("C++", "シープラスプラス"),
            ("node_modules", "ノードモジュールス"),
            ("TypeScript-config", "タイプスクリプトコンフィグ"),
        ] {
            let r = c.split(input);
            let m = find(&r, input).unwrap_or_else(|| panic!("missing {input}: {r:?}"));
            assert_eq!(m.1.as_deref(), Some(expected));
        }
    }
}
