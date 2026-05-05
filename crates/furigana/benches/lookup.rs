//! lookup 性能のベンチマーク。
//!
//! 実行:
//! ```sh
//! cargo bench -p ja-furigana --bench lookup
//! ```
//!
//! 結果は `target/criterion/<group>/<name>/` に HTML report として出力される。
//! 改善 PR ではベース → 修正後の比較を要約 commit すると差分が見える。

// `criterion::black_box` は 0.8 で deprecated になったため、std の方を使う
// (MSRV 1.66+ で利用可、本 crate は 1.88+)。これで criterion 0.5 / 0.8 両対応。
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use furigana::{Furigana, TtsOptions};

fn build_furigana_with_seed_dict() -> Furigana {
    let mut f = Furigana::minimal().expect("minimal init");
    // 代表的な熟語をいくつか流し込んで「辞書 hit する」ケースも測れるようにする。
    // 本番 seed (44k 字) は data ディレクトリ依存になるので最小サンプルに留める。
    let pairs: &[(&str, &str)] = &[
        ("灰桜", "ハイザクラ"),
        ("黎明", "レイメイ"),
        ("曙光", "ショコウ"),
        ("一期一会", "イチゴイチエ"),
        ("四面楚歌", "シメンソカ"),
        ("北海道", "ホッカイドウ"),
        ("吉祥寺", "キチジョウジ"),
        ("秋葉原", "アキハバラ"),
        ("今日", "キョウ"),
        ("明日", "アシタ"),
        ("一日", "イチニチ"),
        ("仲人", "ナコウド"),
    ];
    for (s, r) in pairs {
        f.add_reading(*s, *r);
    }
    f
}

fn bench_init(c: &mut Criterion) {
    let mut g = c.benchmark_group("init");
    g.sample_size(20); // init は重いのでサンプル数控えめ
    g.bench_function("Furigana::minimal", |b| {
        b.iter(|| Furigana::minimal().expect("init"))
    });
    g.finish();
}

fn bench_lookup_mode(c: &mut Criterion) {
    // 注意: medium / long のテキスト (句点を含む長文) を `to_ruby` 等で iter する
    // と、Lindera v3.0.7 + bench harness の組み合わせで巨大 alloc 暴走 (12 GB 級)
    // が起きて STATUS_STACK_BUFFER_OVERRUN で死ぬ既知問題がある。CLI / serve
    // 経由では正常動作するので harness 特有 (詳細は別 issue で upstream 調査)。
    // このため bench は「short テキスト」のみに限定。スループット測定用に
    // 入力サイズも記録。
    let f = build_furigana_with_seed_dict();
    let inputs: &[(&str, &str)] = &[
        ("short", "灰桜の散る道"),
        ("short_phrase", "一期一会と四面楚歌"),
    ];

    let opts = TtsOptions::default();
    for (label, text) in inputs {
        let mut g = c.benchmark_group("lookup");
        g.throughput(Throughput::Bytes(text.len() as u64));
        g.bench_with_input(BenchmarkId::new("to_ruby", label), text, |b, t| {
            b.iter(|| black_box(f.to_ruby(t)));
        });
        g.bench_with_input(BenchmarkId::new("to_hiragana", label), text, |b, t| {
            b.iter(|| black_box(f.to_hiragana(t)));
        });
        g.bench_with_input(BenchmarkId::new("to_tts", label), text, |b, t| {
            b.iter(|| black_box(f.to_tts(t, &opts)));
        });
        g.finish();
    }
}

fn bench_tokenize(c: &mut Criterion) {
    // 同様に「句点を含む長文」は Lindera 暴走で測れないので短文のみ
    let f = build_furigana_with_seed_dict();
    let mut g = c.benchmark_group("tokenize");
    let short = "灰桜の散る道";
    g.throughput(Throughput::Bytes(short.len() as u64));
    g.bench_function("short", |b| b.iter(|| black_box(f.tokenize(short))));
    g.finish();
}

criterion_group!(benches, bench_init, bench_lookup_mode, bench_tokenize);
criterion_main!(benches);
