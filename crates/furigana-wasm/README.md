# ja-furigana-wasm

[`ja-furigana`](https://crates.io/crates/ja-furigana) の WebAssembly バインディング。

ブラウザ / Node.js から `Furigana::minimal()` ベースの動的辞書を使って
フリガナを付けられる。Lindera + IPADIC を embed するため出力 `.wasm` は
数十 MB 級 (release ビルド + brotli/gzip 圧縮で配信推奨)。

> **Status**: v0.1.x (alpha) — API は変更され得る。

## ビルド

```sh
# wasm-pack を未インストールなら
cargo install wasm-pack

# web (ESM) ターゲット
wasm-pack build crates/furigana-wasm --target web --release

# Node.js ターゲット
wasm-pack build crates/furigana-wasm --target nodejs --release

# bundler (webpack / vite) ターゲット
wasm-pack build crates/furigana-wasm --target bundler --release
```

`crates/furigana-wasm/pkg/` に `.wasm` + `.js` + 型定義が出力される。

## 使い方 (Browser ESM)

```html
<script type="module">
  import init, { WasmFurigana } from "./pkg/ja_furigana_wasm.js";
  await init();

  const f = new WasmFurigana();
  f.addReading("灰桜", "ハイザクラ");
  console.log(f.toRuby("灰桜の散る道"));
  // → "{灰桜|はいざくら}の{散る|ちる}{道|みち}"
  console.log(f.toHiragana("灰桜の散る道"));
  // → "はいざくらのちるみち"
  console.log(f.dictSize); // → 1
</script>
```

## 注意

- `Furigana::minimal()` は **空 default** で起動する。本格的に使うには
  `addReading` で大量に辞書を流し込むか、将来追加予定の「辞書 TOML を fetch して
  食わせる API」(現状未実装) を待つ必要がある。
- ファイルシステムベースの `core_dict_dir` / `rules_dir` は WASM では使えない
  (Wasm sandbox にファイルシステムがないため)。
- 数値処理 (`NumberChunker`)・文脈ルール等は `Furigana::minimal()` 時点では
  空状態。これらを動かすにはルール TOML をブラウザから fetch して
  in-memory で読ませる API が必要 (将来の TODO)。

## ライセンス

MIT License。
