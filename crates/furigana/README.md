# ja-furigana

Japanese furigana (ruby) lookup library — Lindera + IPADIC ベースの形態素解析、
語彙辞書とルールはすべて TOML データ駆動。

> **Status**: v0.1.x (alpha) — 公開 API は変更され得ます。MSRV: Rust 1.88+。

> **import 名に注意**: crate 名は `ja-furigana` ですが、Rust 上の `use` は
> `use furigana::Furigana;` (アンダースコアではなくそのまま `furigana`) です
> ([lib] name 設定により)。

```rust
use furigana::Furigana;

let mut f = Furigana::minimal()?;
f.add_reading("灰桜", "ハイザクラ");

println!("{}", f.to_ruby("灰桜の散る道"));
// → "{灰桜|はいざくら}の{散る|ちる}{道|みち}"

println!("{}", f.to_hiragana("灰桜の散る道"));
// → "はいざくらのちるみち"
# Ok::<_, furigana::FuriganaError>(())
```

辞書 / ルールを mount する場合は builder API を使います:

```rust
use furigana::Furigana;

let f = Furigana::builder()
    .core_dict_dir("/path/to/data")
    .rules_dir("/path/to/data")
    .user_dict_dir("/path/to/data/user")
    .overrides_file("/path/to/data/overrides.toml")
    .build()?;
# Ok::<_, furigana::FuriganaError>(())
```

CLI / HTTP server / 詳細は [project README](https://github.com/RyuuNeko1107/ja-furigana) を参照。

## License

MIT License.
