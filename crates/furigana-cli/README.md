# ja-furigana-cli

`furigana` CLI バイナリ + ローカル HTTP サーバー。
[`ja-furigana`](https://crates.io/crates/ja-furigana) lib crate のフロントエンド。

> Status: v0.1.x (alpha) — 公開 API はまだ変更され得る。

## インストール

```sh
cargo install ja-furigana-cli
# → ~/.cargo/bin/furigana がインストールされる
```

## 使い方

```sh
# 1 ショット変換
furigana lookup '灰桜の散る道'                # → tts モード (default)
furigana lookup '灰桜の散る道' --mode ruby    # → {灰桜|はいざくら}...
furigana lookup '灰桜の散る道' --mode hiragana

# ローカルサーバー
furigana serve                                 # http://127.0.0.1:8000

# 辞書管理
furigana dict add 灰桜 ハイザクラ
furigana dict list
furigana dict remove 灰桜
furigana dict import path/to/extra.toml
```

詳細は [プロジェクト README](https://github.com/RyuuNeko1107/ja-furigana) を参照。

## ライセンス

MIT License.
