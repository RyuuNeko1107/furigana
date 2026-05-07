# ja-furigana

Japanese furigana (ruby) lookup library — Lindera + IPADIC ベースの形態素解析、
語彙辞書とルールはすべて TOML データ駆動。

> **Status**: alpha (0.1.x) — `context rule → jukugo → Lindera → unihan` の 5 段階
> 優先順位で読み解決パイプラインを実装。
> 公開 API は 0.1.x の間は変更されうる。MSRV: Rust 1.89+。

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
    .core_loanwords_dir("/path/to/data/loanwords")  // IT 用語等の英単語辞書
    .build()?;
# Ok::<_, furigana::FuriganaError>(())
```

**外来語 (loanwords) サポート**: `core_loanwords_dir` 経由で `[entries]` 形式の TOML
を recursive load。 chunks 段階で英単語 chunk を 1 unit として丸ごと切り出し +
完全一致 lookup (case-fold + 全角→半角) → IT 用語等を確実に hit させる
([データ層の形式](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/loanwords/it.toml) 参照)。

**出力ルール**: `to_hiragana` は surface の文字種で reading 表記を切替えます:
漢字を含む surface はひらがな化、 ASCII / カタカナ / 数字 / 記号のみの surface は
カタカナ統一。 例: `to_hiragana("Kubernetesが安定")` → `"クバネティスがあんてい"`。

CLI / HTTP server / 詳細は [project README](https://github.com/RyuuNeko1107/ja-furigana) を参照。

## License

MIT License.
