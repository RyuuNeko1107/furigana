# ja-furigana-cli

`furigana` CLI バイナリ + ローカル HTTP サーバー。
[`ja-furigana`](https://crates.io/crates/ja-furigana) lib crate のフロントエンド。

> Status: v0.1.x (alpha) — 公開 API はまだ変更され得る。

## インストール

```sh
cargo install ja-furigana-cli
# → ~/.cargo/bin/furigana がインストールされる
```

GitHub Releases から OS 別の binary をダウンロードする方法もあります
([RyuuNeko1107/ja-furigana/releases](https://github.com/RyuuNeko1107/ja-furigana/releases))。

## 使い方

```sh
# 1 ショット変換 (--mode は tts | hiragana | ruby | kanji | romaji | romaji-kunrei)
furigana lookup '灰桜の散る道'                       # → tts (default)
furigana lookup '灰桜の散る道' --mode ruby           # → {灰桜|はいざくら}...
furigana lookup '灰桜の散る道' --mode hiragana       # → はいざくらのちるみち
furigana lookup '灰桜の散る道' --mode romaji         # → haizakura no chiru michi (ヘボン式)
furigana lookup '灰桜の散る道' --mode romaji-kunrei  # → 訓令式

# 対話モード (REPL) — 引数なしで起動 = REPL (Windows なら exe ダブルクリック相当)
furigana
furigana repl --mode hiragana

# 辞書管理
furigana dict pull                       # GitHub Release から最新 furigana-dict を取得
furigana dict pull --version v2026.05.08      # version pin
furigana dict add 灰桜 ハイザクラ        # ユーザー辞書に追加
furigana dict list                       # 現状サマリ
furigana dict remove 灰桜
furigana dict import path/to/extra.toml  # 既存 TOML を user 配下に取り込み

# ローカル HTTP サーバー (`/furigana` エンドポイント)
furigana serve                                 # http://127.0.0.1:8000
furigana serve --bind 0.0.0.0:8000             # 外部からも叩く
furigana serve --auto-pull                     # 起動時に最新 dict を自動取得
FURIGANA_TOKEN=<secret> furigana serve         # 認証有効
```

辞書を最新化する一番シンプルな方法は **`furigana dict pull` してから process を再起動**。
無停止運用したい場合は `--auto-pull` (起動時 1 回) や `[auto_update]` 定期 polling
(config.toml に 1 セクション、admin_tokens 不要) が選択肢。詳細は
[`docs/HTTP_API.md`](https://github.com/RyuuNeko1107/ja-furigana/blob/master/docs/HTTP_API.md#ホットリロード--自動更新) を参照。

データディレクトリの default は **実行ファイルと同じフォルダ** (portable 配置)。
`--data-dir <path>` または `FURIGANA_DATA_DIR` で上書き可能。

詳細は [プロジェクト README](https://github.com/RyuuNeko1107/ja-furigana) を参照。

## ライセンス

MIT License.
