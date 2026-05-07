# ルール一覧 (全部データ駆動)

ja-furigana のルールはすべて [`ja-furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) の TOML ファイルから読まれます。本体バイナリには embed しない方針 (バイナリ肥大化を避ける + 役割分離)。

> 戻る: [README](../README.md) / 関連: [DATA_LAYOUT.md](./DATA_LAYOUT.md) (ファイルの置き場所)

## ファイル一覧

> **ja-furigana-dict は dir 階層で整理されており、 lib 側 `Dict::from_toml_dir` は再帰 walk で全 file を読む。**
> 直近の構造変更で **単漢字 (unihan) は 5 水準別**、 **熟語 (jukugo) は 6 genre dir** に分割された。 詳細は
> [`STATS.md`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/STATS.md) で sub-section 別の件数を確認。

### 単漢字 (`core/unihan/*.toml`、 水準別 5 ファイル)

| ファイル | 内容 |
|---|---|
| [`joyo.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/unihan/joyo.toml) | 常用漢字 2,136 字 (review 集中対象) |
| [`jinmeiyou.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/unihan/jinmeiyou.toml) | 人名用 (常用と重複しない 855 字) |
| [`jis_basic.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/unihan/jis_basic.toml) | JIS 第1+第2水準 (CJK Basic Block の残り、 ~13k 字) |
| [`jis_supplement.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/unihan/jis_supplement.toml) | JIS 第3+第4水準 (Extension A + Compatibility、 ~4.8k 字) |
| [`extension.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/unihan/extension.toml) | 拡張漢字 (Extension B 以降、 機械的扱い、 ~22.8k 字) |

`Dict::lookup_unihan` で最終 fallback (Step 6) として呼ばれる。

### 熟語 (`core/jukugo/<genre>/*.toml`、 6 genre / 26 ファイル)

| genre dir | 含む file (代表) | 内容 |
|---|---|---|
| [`basic/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/core/jukugo/basic) | `general.toml` / `four_char.toml` | 一般熟語 / 四字熟語 |
| [`nature/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/core/jukugo/nature) | `animals` / `foods` / `weather` / `body_parts` / `place_names` / `science` | 自然・生命系 |
| [`humanities/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/core/jukugo/humanities) | `literature` / `arts` / `idioms` / `abstracts` / `music` / `religions` / `emotions` | 人文・芸術系 |
| [`society/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/core/jukugo/society) | `politics` / `finance` / `sports` / `specialized` | 社会・制度系 |
| [`proper/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/core/jukugo/proper) | `personal_names` / `proper_nouns` | 固有名詞 |
| [`objects/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/core/jukugo/objects) | `colors` / `clothes` / `vehicles` / `architecture` / `railway` | 物体・工芸系 |

各 genre dir 直下に `_genre.toml` ([genre] name + description + order) があり、 STATS.md の sub-section heading に使われる。 `_genre.toml` は lib では silent skip、 release tar.gz から exclude。

`Dict::lookup_jukugo` で Step 3 (jukugo lookup) として Lindera より優先採用。

### その他 core (`core/`)

| カテゴリ | repo path | 内容 |
|---|---|---|
| 異体字 | [`core/compat.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/compat.toml) | 髙→高、 瀧→滝 等の正規化マップ。 lib Step 1 (`normalize_text`) で適用 |
| 単漢字 default override | [`core/single_overrides.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/single_overrides.toml) | 1 字 surface に対する明示的 default 上書き ([issue #15](https://github.com/RyuuNeko1107/ja-furigana/issues/15) の限定解、 lib Step 4 で Lindera より優先) |
| 外来語 | [`core/loanwords/it.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/loanwords/it.toml) | IT 用語等の英字 surface (Kubernetes / Docker / TypeScript 等)、 chunks 階層 4.7 で完全一致 lookup |
| 作品造語 | [`core/works/<medium>/<title>.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/core/works) | 媒体別作品単位 (game / literature 等)、 公式読みのみ採録 + 出典コメント必須 |

### エンジンルール (`rules/`)

| カテゴリ | repo path | 内容 |
|---|---|---|
| 助数詞 | [`rules/counters/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/rules/counters) (7+ ファイル) | 本 / 匹 / 個 / 年 / 月 / 日 + 年度 / 時間半 等 |
| 文脈ルール | [`rules/context/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/rules/context) (3 ファイル) | 一日→ツイタチ/イチニチ + 単漢字 default 上書き 等 |
| 日付 | [`rules/days.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/days.toml) | 1〜31 日の特殊読み (1→ツイタチ 等) |
| 大数 | [`rules/scales.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/scales.toml) | 万 / 億 / 兆 / 京 / 垓 ... |
| 単位 | [`rules/units.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/units.toml) | km / kg / mL + 円 / % |
| 記号 | [`rules/symbols.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/symbols.toml) | + / − / % / 〜 / ・ / ※ ... |
| ラテン文字 | [`rules/latin.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/latin.toml) | A→エー / B→ビー ... |
| 例外語句 | [`rules/numeric_phrases.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/numeric_phrases.toml) | 二十歳→ハタチ + 百個 / 千個 等 |
| 後処理 | [`rules/postprocess.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/postprocess.toml) | mode 別 regex 置換 (Step 7) |

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
