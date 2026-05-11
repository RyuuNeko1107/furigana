//! テスト用 fixture (tests/fixtures/rules/) のロード検証
//!
//! 本体バイナリには rules を embed しないため、テスト用に保持している
//! fixture ファイルが loader でパースできることを確認する。
//! 実データは [`furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict)
//! 側でメンテナンスされる。

use furigana::loader::load_rules_dir;
use std::path::PathBuf;

fn data_rules_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rules")
}

#[test]
fn load_all_rules_succeeds() {
    let dir = data_rules_dir();
    assert!(
        dir.exists(),
        "data/rules dir not found at {}",
        dir.display()
    );

    let data = load_rules_dir(&dir).expect("loading rules failed");

    // ─── Counters ──────────────────────────────────────────────────────
    assert!(!data.counters.simple.is_empty(), "simple counters empty");
    assert!(!data.counters.counter.is_empty(), "complex counters empty");
    assert_eq!(
        data.counters.simple.get("円").map(String::as_str),
        Some("エン")
    );

    let hon = data.counters.counter.get("本").expect("本 not found");
    assert_eq!(hon.default.as_deref(), Some("ホン"));
    assert!(!hon.rules.is_empty());

    let toki = data.counters.counter.get("時").expect("時 not found");
    assert_eq!(toki.replacements.len(), 3);
    assert_eq!(toki.specials.get("0").map(String::as_str), Some("レイジ"));

    let tsuki = data.counters.counter.get("月").expect("月 not found");
    assert_eq!(tsuki.specials.get("4").map(String::as_str), Some("シガツ"));

    let me = data.counters.counter.get("目").expect("目 not found");
    assert!(me.mode.is_some());
    assert_eq!(me.suffix.as_deref(), Some("メ"));

    // ─── Context ───────────────────────────────────────────────────────
    // alpha.15 で rules::context は削除済 (= 文脈分岐 reading は dict 側の
    // [entries."X".match] / [[kanji]] block で表現)。 旧 context.toml が読み
    //込まれても silent skip される。

    // ─── Days ──────────────────────────────────────────────────────────
    assert_eq!(data.days.get(1), Some("ツイタチ"));
    assert_eq!(data.days.get(4), Some("ヨッカ"));
    assert_eq!(data.days.get(20), Some("ハツカ"));
    assert_eq!(data.days.get(31), Some("サンジュウイチニチ"));
    assert_eq!(data.days.len(), 31);

    // ─── Scales ────────────────────────────────────────────────────────
    assert_eq!(data.scales.lookup("万"), Some("マン"));
    assert_eq!(data.scales.lookup("無量大数"), Some("ムリョウタイスウ"));
    assert_eq!(data.scales.lookup("𥝱"), Some("シ"));

    // ─── Units (ci フラグ込み) ─────────────────────────────────────────
    assert_eq!(data.units.lookup("km"), Some("キロメートル"));
    assert_eq!(data.units.lookup("L"), Some("リットル"));
    assert_eq!(data.units.lookup("l"), Some("リットル"));
    assert_eq!(data.units.lookup("mL"), Some("ミリリットル"));
    assert_eq!(data.units.lookup("ML"), Some("ミリリットル"));

    // ─── Symbols ───────────────────────────────────────────────────────
    assert_eq!(data.symbols.lookup("+"), Some("プラス"));
    assert_eq!(data.symbols.lookup("‰"), Some("パーミル"));

    // ─── Numeric phrases ───────────────────────────────────────────────
    assert_eq!(data.numeric_phrases.lookup("二十歳"), Some("ハタチ"));
    assert_eq!(data.numeric_phrases.lookup("明後日"), Some("アサッテ"));

    // ─── Compat ────────────────────────────────────────────────────────
    // 異体字データは furigana-dict 側で管理するため本リポジトリの
    // data/rules には存在しない (役割分離)。空である事を確認するに留める。
    assert!(data.compat.is_empty());
}
