//! 最小例: `cargo run --example basic`
//!
//! `Furigana::minimal()` は **空 default** で起動する (本体に rules / 辞書を embed しないため)。
//! 助数詞や慣用語句などの高度な処理を有効にしたい場合は、
//! [`furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) を pull した後、
//! builder で `rules_dir(...)` / `core_dict_dir(...)` を mount する。
//!
//! このサンプルは zero-config で動く範囲の挙動 (lindera + add_reading) を示す。

use furigana::Furigana;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut f = Furigana::minimal()?;

    // 動的辞書追加 (実運用では furigana dict pull で取得した core_dict_dir を mount)
    f.add_reading("灰桜", "ハイザクラ");
    f.add_reading("黎明", "レイメイ");

    let samples = ["灰桜の散る道", "黎明の光", "二十歳の誕生日", "3本のバナナ"];

    for text in samples {
        println!("{:>20} → {}", text, f.to_ruby(text));
    }

    println!();
    println!("== ひらがな展開 ==");
    println!("{}", f.to_hiragana("灰桜の散る道"));

    Ok(())
}
