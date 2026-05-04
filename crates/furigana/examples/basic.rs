//! 最小例: `cargo run --example basic`
//!
//! 埋め込みルールのみで Furigana を初期化し、いくつかのテキストに ruby を付ける。

use furigana::Furigana;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 最小構成 (zero-config)
    let mut f = Furigana::minimal()?;

    // 動的に辞書追加
    f.add_reading("灰桜", "ハイザクラ");
    f.add_reading("黎明", "レイメイ");

    // 異体字 (例: 髙崎) は furigana-dict 側で管理されるため、
    // minimal モードでは正規化されない。動作を見るには
    // `furigana dict pull` 後に `core_dict_dir(...)` を builder で渡す。
    let samples = [
        "灰桜の散る道",
        "黎明の光",
        "1月1日に集合",
        "二十歳の誕生日",
        "3本のバナナ",
    ];

    for text in samples {
        println!("{:>20} → {}", text, f.to_ruby(text));
    }

    println!();
    println!("== ひらがな展開 ==");
    println!("{}", f.to_hiragana("灰桜の散る道"));

    Ok(())
}
