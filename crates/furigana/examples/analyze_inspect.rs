//! `analyze()` + inspect API の例: `cargo run --example analyze_inspect`
//!
//! Smart engine の path 採択 trace と、 dict 改善候補 (= dict 未登録疑いの surface)
//! 抽出を実演する。 production logging で 「未登録 surface ranking」 を作る用途
//! (= production traffic → log → 集計 → dict PR) の base implementation。
//!
//! 詳細:
//! - [`Furigana::analyze`] で AnalyzeResult を取得 (= 採択 path + 全 candidate + boundary)
//! - [`extract_dict_gap_candidates`] で band threshold 以下の漢字 token を context 込みで抽出

use furigana::{extract_dict_gap_candidates, Furigana};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut f = Furigana::minimal()?;

    // dict に登録された surface (= 採択 candidate band 1000)
    f.add_reading("灰桜", "ハイザクラ");
    // 「散る」 「道」 等は dict 未登録 → Lindera fallback (band 50) で reading 取得
    // → dict 改善候補として extract される想定

    let input = "灰桜の散る道";
    let result = f.analyze(input);

    println!("== 採択 path (= tokens) ==");
    for (i, token) in result.tokens.iter().enumerate() {
        println!(
            "  #{i}: surface={:?} reading={:?} range={}..{}",
            token.surface, token.reading, token.range.start, token.range.end
        );
    }

    println!();
    println!("== 各 token 位置の全 candidate (= 競合候補) ==");
    for (i, cands) in result.candidates.iter().enumerate() {
        println!("  pos {i}:");
        for c in cands {
            println!(
                "    surface={:?} reading={:?} band={} length={}",
                c.surface, c.reading, c.score.band, c.score.length
            );
        }
    }

    println!();
    println!("== 漢字連続 boundary region ==");
    for region in &result.boundary_regions {
        println!(
            "  bytes {}..{} → {:?}",
            region.start,
            region.end,
            &input[region.clone()]
        );
    }

    println!();
    println!("== dict 改善候補 (band ≤ 100 + 漢字含む surface) ==");
    // production logging の input: production traffic に対してこの呼び出しで
    // 「dict 未登録疑い」 surface を抽出、 caller が log に流して頻度集計する
    let gaps = extract_dict_gap_candidates(&result, input, 3, 100);
    if gaps.is_empty() {
        println!("  (該当なし、 全 surface が dict / 高 band candidate で覆われている)");
    } else {
        for gap in &gaps {
            println!(
                "  surface={:?} reading={:?} band={} context=[{}|{}|{}]",
                gap.surface,
                gap.reading,
                gap.band,
                gap.context.before,
                gap.context.surface,
                gap.context.after,
            );
        }
        println!();
        println!("→ これらを production log で頻度ランキングすると、 dict 改善 PR の input になる");
    }

    Ok(())
}
