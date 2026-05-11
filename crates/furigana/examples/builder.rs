//! `FuriganaBuilder` のフル機能例: `cargo run --example builder`
//!
//! 1. 一時ディレクトリに core / user / overrides の TOML を生成
//! 2. `FuriganaBuilder` で 3 階層 + add_entry を mount
//! 3. 優先順位が overrides > user > core で適用されることを観察
//!
//! 実運用では `furigana dict pull` で取得した `<data_dir>/data/` を `core_dict_dir`
//! に渡せば良い (本サンプルは TOML が手書きでも mount できることを示す)。

use furigana::Furigana;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = std::env::temp_dir().join("furigana_builder_example");
    let core = tmp.join("core");
    let user = tmp.join("user");
    let overrides = tmp.join("overrides.toml");
    fs::create_dir_all(&core)?;
    fs::create_dir_all(&user)?;

    // ---- core 辞書: 「灰桜」→「ハイザクラ」 / 「黎明」→「レイメイ」 ----
    fs::write(
        core.join("ja.toml"),
        r#"
[meta]
schema_version = "2"
role = "jukugo"

[entries]
"灰桜" = "ハイザクラ"
"黎明" = "レイメイ"
"#,
    )?;

    // ---- user 辞書: 灰桜 を「カイオウ」に上書き (core を override) ----
    fs::write(
        user.join("custom.toml"),
        r#"
[meta]
schema_version = "2"
role = "jukugo"

[entries]
"灰桜" = "カイオウ"
"#,
    )?;

    // ---- overrides: 黎明 を「クライセンス」に上書き (user/core より強い) ----
    fs::write(
        &overrides,
        r#"
[meta]
schema_version = "2"
role = "jukugo"

[entries]
"黎明" = "クライセンス"
"#,
    )?;

    let f = Furigana::builder()
        .core_dict_dir(&core)
        .user_dict_dir(&user)
        .overrides_file(&overrides)
        .add_entry("追加語", "ツイカゴ") // 最優先 (override より強い)
        .build()?;

    println!("辞書サイズ: {}", f.dict_size());
    println!();

    println!("== 優先度の検証 ==");
    for text in ["灰桜", "黎明", "追加語"] {
        println!("{:>5} → {}", text, f.to_ruby(text));
    }

    println!();
    println!(
        "優先度 (高→低): add_entry > overrides_file > user_dict_dir > core_dict_dir > Lindera"
    );
    println!("  灰桜:  core=ハイザクラ      → user で カイオウ に上書き");
    println!("  黎明:  core=レイメイ         → overrides で クライセンス に上書き");
    println!("  追加語: add_entry で直接追加");

    // 後始末 (失敗しても CI には影響しないので ok)
    fs::remove_dir_all(&tmp).ok();

    Ok(())
}
