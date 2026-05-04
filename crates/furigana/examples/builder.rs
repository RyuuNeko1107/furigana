//! builder API のフル機能例: `cargo run --example builder`
//!
//! このサンプルは:
//! 1. 一時ディレクトリに core / user / overrides の TSV を生成
//! 2. FuriganaBuilder で 3 階層の辞書を mount
//! 3. 優先度が overrides > user > core であることを示す

use furigana::Furigana;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = std::env::temp_dir().join("furigana_builder_example");
    let core = tmp.join("dict").join("core");
    let user = tmp.join("dict").join("user");
    let overrides = tmp.join("dict").join("overrides.tsv");
    fs::create_dir_all(&core)?;
    fs::create_dir_all(&user)?;

    // core 辞書: 灰桜=ハイザクラ
    fs::write(core.join("ja.tsv"), "灰桜\tハイザクラ\n黎明\tレイメイ\n")?;

    // user 辞書: 灰桜=カイオウ (core を上書き)
    fs::write(user.join("custom.tsv"), "灰桜\tカイオウ\n")?;

    // overrides: 黎明=クライセンス (user の core を上書き)
    fs::write(&overrides, "黎明\tクライセンス\n")?;

    let f = Furigana::builder()
        .core_dict_dir(&core)
        .user_dict_dir(&user)
        .overrides_file(&overrides)
        .add_entry("追加語", "ツイカゴ") // builder で直接追加 (最優先)
        .build()?;

    println!("辞書サイズ: {}", f.dict_size());

    for text in ["灰桜", "黎明", "追加語"] {
        println!("{:>5} → {}", text, f.to_ruby(text));
    }
    println!();
    println!("優先度: overrides_file > user_dict_dir > core_dict_dir");
    println!("  灰桜: core=ハイザクラ → user で カイオウ に上書き");
    println!("  黎明: core=レイメイ   → overrides で クライセンス に上書き");

    // 後始末
    fs::remove_dir_all(&tmp).ok();

    Ok(())
}
