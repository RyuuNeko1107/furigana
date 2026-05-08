# ルール一覧 (全部データ駆動)

ja-furigana のルールはすべて [`ja-furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) の TOML ファイルから読まれます。本体バイナリには embed しない方針 (バイナリ肥大化を避ける + 役割分離)。

> 戻る: [README](../README.md) / 関連: [DATA_LAYOUT.md](./DATA_LAYOUT.md) (ファイルの置き場所)

## カテゴリ概念

ja-furigana-dict は以下の **概念カテゴリ** を持つ。 具体的な dir 階層 / file 名 / 件数 は
[`STATS.md`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/STATS.md)
が auto-generate する (構造変更時に doc が古びるのを避けるため、 ここでは概念のみ説明)。

| カテゴリ | 役割 (lib pipeline 内の位置) | dist path |
|---|---|---|
| 単漢字 (`unihan`) | Step 6 fallback、 surface = 1 文字。 lib `Dict::lookup_unihan` で参照 | `data/unihan/*` |
| 熟語 (`jukugo`) | Step 3、 surface ≥ 2 文字の固定読み。 lib `Dict::lookup_jukugo` で Lindera より優先 | `data/jukugo/<genre>/*` |
| 作品造語 (`works`) | jukugo と同じ Step 3 経路、 媒体別作品単位 (公式読みのみ採録) | `data/works/<medium>/<title>.toml` |
| 異体字 (`compat`) | Step 1 (`kana::normalize_text`) で `[map]` の異体字を標準字に正規化 | `data/compat.toml` |
| 単漢字 default override (`single_overrides`) | Step 4、 1 字 surface に対する明示的 default ([issue #15](https://github.com/RyuuNeko1107/ja-furigana/issues/15) の限定解) | `data/single_overrides.toml` |
| 外来語 (`loanwords`) | chunks 階層 4.7、 ASCII 始まり surface を完全一致 lookup (case-fold + 全角→半角) | `data/loanwords/*.toml` |

### エンジンルール (`rules/`)

| カテゴリ | 役割 |
|---|---|
| `counters/` | 助数詞 (本 / 匹 / 個 / 年 / 月 / 日 + 年度 / 時間半 等) |
| `context/` | 文脈依存読み (一日 → ツイタチ/イチニチ 等の同形異音語) |
| `days.toml` | 1〜31 日の特殊読み (1 → ツイタチ) |
| `scales.toml` | 大数 (万 / 億 / 兆 / 京 / 垓 ...) |
| `units.toml` | SI 単位 (km / kg / mL ...、 case-insensitive) |
| `symbols.toml` | 記号 (+ / − / % / 〜 / ・ / ※ ...) |
| `latin.toml` | ラテン文字 (A → エー、 B → ビー ...) |
| `numeric_phrases.toml` | 例外語句 (二十歳 → ハタチ、 百個、 千個 等) |
| `postprocess.toml` | mode 別 regex 置換 (Step 7) |

具体的な内訳 / file 数 / 件数は STATS.md sub-section に毎回 master push 後 auto-regen される (`tools/regen_stats.py`)。

`furigana dict pull` で取得 → builder の `core_dict_dir(path)` / `rules_dir(path)` で mount される。詳細は [DATA_LAYOUT.md](./DATA_LAYOUT.md) を参照。

## 助数詞ルール (`counters/`)

7 ファイルに細分化。同一サブディレクトリ内の `*.toml` は全て自動 merge される (lib loader 側で対応済み)。

| ファイル | 範囲 |
|---|---|
| `simple.toml` | 単純サフィックス助数詞 |
| `time.toml` | 月 / 日 / 時 / 分 / 分半 / 週間 / 回 |
| `people.toml` | 人 |
| `objects.toml` | 本 / 匹 / 杯 / 個 / 歳 / 冊 |
| `places.toml` | 階 / ヶ所 / 箇所 / か所 |
| `percent.toml` | % / ％ |
| `recursive.toml` | 目 (再帰モード) |

## 文脈ルール (`context/`)

3 ファイルに細分化。各ルールは「対象表層 + 条件 → 読み」のリスト形式:

```toml
[[rule]]
surface = "一日"
default = "イチニチ"          # どの match にも当てはまらないときの読み (任意)

[[rule.match]]
prev_ends_with_month = true   # 前トークンが「1月」「12月」等で終わるなら…
reading = "ツイタチ"          # 「ツイタチ」と読む
```

| ファイル | 範囲 |
|---|---|
| `numbers.toml` | 数字を含む慣用語句 (一日 / 一人 / 一月 / 一杯 等) |
| `homonyms.toml` | 同形異音語 (上手 / 下手 / 人気 / 大人気 / 十分) |
| `special.toml` | 単純な読み固定 (大人 / 仲人 / 今日 / 何日 / 日本 等) |

使える match 条件 (prev / next / next-next / pos) の一覧は [`ja-furigana-dict/CONTRIBUTING.md`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/CONTRIBUTING.md#使える条件一覧) を参照。

## 後処理 regex (`postprocess.toml`、Step 7 (mode 別後処理 regex))

`Furigana::to_{hiragana,ruby,tts,romaji}` の **出力直前** に適用される regex 置換ルール。
辞書 / context rule で表現しづらい文字列レベルの最終調整 (例: 促音化補正、mode 別の整形) に使う。

```toml
[[rule]]
pattern = "ジュウパー"             # regex (Rust の regex crate 構文)
replacement = "ジュッパー"          # $1, $2 で capture group 参照可
modes = ["hiragana", "tts"]      # 適用 mode (空 or 省略 = 全 mode)

[[rule]]
pattern = "(\\d+)\\s*ヶ"
replacement = "$1カ"             # キャプチャグループ参照
modes = []                       # 全 mode
```

実装上の挙動:
- `rules` 配列を上から順に評価 → 該当 mode のルールだけ apply
- 内部で regex を pre-compile (起動時 1 回)
- ルール 0 件 (or `postprocess.toml` 不在) なら no-op

> ⚠️ `ruby` mode は `{漢字|読み}` 構造を含むので、`{` `|` `}` を壊さない pattern にすること。

## TOML 形式 (一般辞書)

熟語 / 単漢字 / 異体字以外のすべては以下の形式:

```toml
[entries]
"灰桜" = "ハイザクラ"
"黎明" = "レイメイ"
"明後日" = "アサッテ"
```

- key: 表層形 (漢字を含む文字列)
- value: ひらがな または 全角カタカナ (慣習: 訓=ひら / 音=カタ)

## 読みなし時の挙動

辞書 / ルールが未配置 (`Furigana::minimal()` のみ) の場合:

- ✅ Lindera (IPADIC) の素朴な読みは出る (動詞・形容詞・既知の名詞)
- ✅ `Furigana::add_reading()` での動的追加は効く
- ❌ 助数詞ルール / 文脈ルール / 大数 / SI 単位 / 異体字正規化は無効
- ❌ 慣用語句 (二十歳→ハタチ 等) は素朴な「ニジュッサイ」になる

`furigana dict pull` で配布版を取得 → 自動で全機能有効化。詳細は [DATA_LAYOUT.md](./DATA_LAYOUT.md)。

## 辞書を増やしたい

新しい読みやルール修正の PR は **[`ja-furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) リポジトリ** にお願いします。Rust 知識不要、TOML を 1 行追加するだけ。詳細は同 repo の [CONTRIBUTING.md](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/CONTRIBUTING.md) を参照。
