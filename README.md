# furigana

> Japanese furigana (ruby) lookup library and HTTP server in Rust — fully data-driven rules, no DB required.

日本語テキストに **フリガナ (読み仮名 / ルビ)** を付けるための Rust 製ライブラリ + ローカル HTTP サーバー。

> ⚠️ **Status**: Pre-alpha — 開発中。API・データ形式は変更されます。

## なにこれ

- **ライブラリ** (`furigana` crate): `cargo add furigana` で組み込める。DB 不要、async 不要、Pure Rust。
- **CLI / ローカル サーバー** (`furigana-cli` → `furigana` バイナリ): ローカル HTTP API + 辞書管理コマンド。

**ローカル利用 / 組み込み用途** を前提に設計しています。デフォルト bind は `127.0.0.1`、認証なし、レート制限なし。公開サービスではなく、自分のアプリ・社内ツール・スクリプトに組み込む使い方を想定。

## なぜ作るのか

既存のフリガナツールはだいたい次のどちらか:

- (a) ルール (助数詞・連濁・文脈読み等) を全部 Rust ソースに直書きしているため、辞書追加・修正に **再コンパイル必須**、Rust 知識ない人は PR を投げられない
- (b) Postgres / Java など重量級依存があり、「ちょっと試す」ハードルが高い

`furigana` は、助数詞ルール・スケール・SI 単位・文脈依存読み・例外語句といった **すべてのルールを編集可能な TOML / TSV データファイル** として外出ししています。読みを追加したい? TSV を編集。助数詞ルールを直したい? TOML を編集。再コンパイル不要。

## クイックスタート

### ライブラリとして使う

```rust
use furigana::Furigana;

let f = Furigana::minimal();
let ruby = f.to_ruby("灰桜の散る道");
// → "{灰桜|はいざくら}の{散|ち}る{道|みち}"
```

### ローカルサーバーとして使う

```sh
$ cargo install furigana-cli         # または Releases から binary を取得
$ furigana dict pull                 # 辞書本体を GitHub Release から DL
$ furigana serve                     # http://127.0.0.1:8000 で起動
$ curl 'http://127.0.0.1:8000/furigana?text=灰桜'
```

## 辞書の置き場所

```
~/.local/share/furigana/dict/   (Windows: %LOCALAPPDATA%\furigana\dict\)
├── core/         <- `furigana dict pull` で配布版を取得 (read-only 扱い)
├── user/         <- 自由に *.tsv を置く (起動時に全 scan)
└── overrides.tsv <- 強制上書き用 (最優先)
```

優先順位 (高→低):

1. `overrides.tsv`
2. `user/*.tsv`
3. `core/*.tsv`
4. embed されている異体字マップ
5. Lindera (形態素解析) のフォールバック

## ルール一覧 (全部データ)

| ファイル | 内容 |
|---|---|
| `data/rules/counters.toml` | 助数詞 (本/匹/個/年/月/日…) の連濁・促音化ルール |
| `data/rules/days.toml` | 1〜31 日の特殊読み (1→ツイタチ 等) |
| `data/rules/scales.tsv` | 万 / 億 / 兆 / 京 / 垓… 大数スケール |
| `data/rules/units.tsv` | SI 単位 (km / kg / mL …) |
| `data/rules/symbols.tsv` | 記号読み (+ / − / % / ‰ …) |
| `data/rules/latin.tsv` | ラテン文字読み (A→エー…) |
| `data/rules/numeric_phrases.tsv` | 例外語句 (二十歳→ハタチ 等) |
| `data/rules/context.toml` | 前後トークンを見る文脈ルール (一日→ツイタチ/イチニチ) |
| `data/compat_map.tsv` | 異体字 → 標準字 の正規化 |

ルール編集 → サーバー再起動 (or `POST /admin/reload`) だけで反映。

## ライセンス

MIT License もしくは Apache License 2.0 のいずれか好きな方を選べます (デュアルライセンス):

- [LICENSE-APACHE](LICENSE-APACHE)
- [LICENSE-MIT](LICENSE-MIT)

## コントリビュート

新しい読みやルール修正は、ほとんどの場合 `data/rules/` 配下の TSV / TOML を編集するだけです。Rust を書く必要はありません。詳細は [CONTRIBUTING.md](CONTRIBUTING.md) (準備中) を参照してください。
