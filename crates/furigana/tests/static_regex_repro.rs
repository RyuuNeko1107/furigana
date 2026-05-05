//! NumberChunker::split 内で使われる静的 regex を 1 つずつ呼んで暴走特定。
//! 空 rules でもこれらは構築されているので、ここに犯人がいる仮説。

const SAMPLE: &str = "こんにちは。さようなら。";

/// URL_RE は private なので、同じ pattern をローカルに定義してテスト。
/// (実 prod のものと同一文字列をコピー)
fn url_re() -> regex::Regex {
    regex::Regex::new(r#"(?xi)(?:(?:https?://|ftp://|file://|www\.)[^\s<>"'\(\)\{\}\[\]]+|(?:[A-Za-z0-9\-]+\.)+[A-Za-z]{2,}(?::\d+)?(?:/[^\s<>"'\(\)\{\}\[\]]*)?|\d{1,3}(?:\.\d{1,3}){3}(?::\d+)?(?:/[^\s<>"'\(\)\{\}\[\]]*)?)"#).unwrap()
}

fn email_re() -> regex::Regex {
    regex::Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap()
}

fn time_colon_re() -> regex::Regex {
    regex::Regex::new(r"([0-9０-９]{1,2})[:：]([0-9０-９]{2})(?:[:：]([0-9０-９]{2}))?").unwrap()
}

fn time_jp_full_re() -> regex::Regex {
    regex::Regex::new(r"([0-9０-９]{1,2})時(?:([0-9０-９]{1,2})分)?(?:([0-9０-９]{1,2})秒)?")
        .unwrap()
}

fn date_kanji_full_re() -> regex::Regex {
    regex::Regex::new(r"([0-9０-９]{1,4})年([0-9０-９]{1,2})月([0-9０-９]{1,2})日").unwrap()
}

fn date_kanji_md_re() -> regex::Regex {
    regex::Regex::new(r"([0-9０-９]{1,2})月([0-9０-９]{1,2})日").unwrap()
}

fn digit_re() -> regex::Regex {
    regex::Regex::new(
        r"[+\-\u{2212}\u{FF0D}\u{FF0B}]?[0-9０-９]+(?:,[0-9０-９]{3})*(?:\.[0-9０-９]+)?",
    )
    .unwrap()
}

#[test]
#[ignore = "regex repro"]
fn url_find() {
    let re = url_re();
    let _ = re.find(SAMPLE);
}

#[test]
#[ignore = "regex repro"]
fn email_find() {
    let re = email_re();
    let _ = re.find(SAMPLE);
}

#[test]
#[ignore = "regex repro"]
fn time_colon_find() {
    let re = time_colon_re();
    let _ = re.find(SAMPLE);
}

#[test]
#[ignore = "regex repro"]
fn time_jp_find() {
    let re = time_jp_full_re();
    let _ = re.find(SAMPLE);
}

#[test]
#[ignore = "regex repro"]
fn date_full_find() {
    let re = date_kanji_full_re();
    let _ = re.find(SAMPLE);
}

#[test]
#[ignore = "regex repro"]
fn date_md_find() {
    let re = date_kanji_md_re();
    let _ = re.find(SAMPLE);
}

#[test]
#[ignore = "regex repro"]
fn digit_find() {
    let re = digit_re();
    let _ = re.find(SAMPLE);
}

// ─── Dynamic regex (空 rules の never-match pattern) ─────────────────

/// `build_alt_regex` の空 list 分岐で生成される pattern。
/// 「絶対 match しない」意図で `\A\B` zero-width pair を使ってる。
fn never_match_re() -> regex::Regex {
    regex::Regex::new(r"(?P<n>\A\B)(?P<x>\A\B)").unwrap()
}

#[test]
#[ignore = "regex repro"]
fn never_match_compile_only() {
    let _ = never_match_re();
}

#[test]
#[ignore = "regex repro"]
fn never_match_find() {
    let re = never_match_re();
    let _ = re.find(SAMPLE);
}

#[test]
#[ignore = "regex repro"]
fn never_match_captures() {
    let re = never_match_re();
    let _ = re.captures(SAMPLE);
}

/// 「at_start」と同じ仕方で呼ぶ (chunks::regex::at_start のロジック)
#[test]
#[ignore = "regex repro"]
fn never_match_at_start() {
    let re = never_match_re();
    let _ = re
        .captures(SAMPLE)
        .filter(|c| c.get(0).is_some_and(|m| m.start() == 0));
}

/// NumberChunker::new だけ (split を呼ばない)。
#[test]
#[ignore = "regex repro"]
fn chunker_new_only() {
    use furigana::chunks::NumberChunker;
    use furigana::rules::RulesData;
    let r = RulesData::default();
    let _c = NumberChunker::new(&r);
}
