//! 最小例: `cargo run --example basic`
//!
//! `Furigana::minimal()` は **空 default** で起動する (本体に rules / 辞書を embed しない方針)。
//! それでも以下は動作する:
//!
//! - 形態素解析 (Lindera + IPADIC) の素朴な読み
//! - `Furigana::add_reading()` で動的に追加した語彙の上書き
//! - すべての出力モード (`to_ruby` / `to_hiragana` / `to_tts` / `to_romaji`)
//!
//! 助数詞 / 文脈ルール / 大数 / SI 単位 / 異体字正規化などは無効。これらを有効化するには
//! [`furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) の TOML を mount する
//! (`builder` example 参照)。

use furigana::{Furigana, RomajiStyle, TtsOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut f = Furigana::minimal()?;

    // 動的辞書追加 (実運用では furigana dict pull で取得した core_dict_dir を mount)
    f.add_reading("灰桜", "ハイザクラ");
    f.add_reading("黎明", "レイメイ");
    f.add_reading("曙光", "ショコウ");

    let samples = ["灰桜の散る道", "黎明の光", "曙光が射す朝"];

    println!("== ruby ==");
    for text in samples {
        println!("  {:>20} → {}", text, f.to_ruby(text));
    }

    println!();
    println!("== hiragana ==");
    for text in samples {
        println!("  {:>20} → {}", text, f.to_hiragana(text));
    }

    println!();
    println!("== TTS (ポーズ込み) ==");
    let tts_opts = TtsOptions::default();
    println!(
        "  {}",
        f.to_tts("こんにちは。灰桜の散る道を歩きます。", &tts_opts)
    );

    println!();
    println!("== romaji ==");
    println!(
        "  Hepburn: {}",
        f.to_romaji("灰桜の散る道", RomajiStyle::Hepburn)
    );
    println!(
        "  Kunrei:  {}",
        f.to_romaji("灰桜の散る道", RomajiStyle::Kunrei)
    );

    Ok(())
}
