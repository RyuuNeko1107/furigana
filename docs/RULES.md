# ルール一覧 (全部データ駆動)

ja-furigana のルールはすべて [`ja-furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) の TOML ファイルから読まれます。本体バイナリには embed しない方針 (バイナリ肥大化を避ける + 役割分離)。

> 戻る: [README](../README.md) / 関連: [DATA_LAYOUT.md](./DATA_LAYOUT.md) (ファイルの置き場所)

## ファイル一覧

| カテゴリ | repo path (PR 用) | 配布後 path | 内容 |
|---|---|---|---|
| 単漢字 | [`core/unihan.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/unihan.toml) | `data/unihan.toml` | 43k+ 字。形態素解析 / 辞書でも hit しないときの最終 fallback |
| 異体字 | [`core/compat.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/compat.toml) | `data/compat.toml` | 髙→高、瀧→滝 等の正規化マップ |
| 一般熟語 | [`core/jukugo/general.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/jukugo/general.toml) | `data/jukugo/general.toml` | 二字 / 三字の熟語 (灰桜→ハイザクラ 等) |
| 四字熟語 | [`core/jukugo/four_char.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/jukugo/four_char.toml) | `data/jukugo/four_char.toml` | 4 字 + 全 CJK 漢字 (一期一会、四面楚歌 等) |
| 固有名詞 | [`core/jukugo/proper_nouns.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/jukugo/proper_nouns.toml) | `data/jukugo/proper_nouns.toml` | 会社 / 作品 / ブランド名 |
| 地名 | [`core/jukugo/place_names.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/jukugo/place_names.toml) | `data/jukugo/place_names.toml` | 国 / 都道府県 / 市区町村 / 駅 / スポット |
| 人名 | [`core/jukugo/personal_names.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/jukugo/personal_names.toml) | `data/jukugo/personal_names.toml` | 姓・名・著名人 |
| 助数詞 | [`rules/counters/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/rules/counters) (7 ファイル) | `data/counters/*.toml` | 本/匹/個/年/月/日 ... の連濁・促音化・kana 末尾置換 |
| 文脈ルール | [`rules/context/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/rules/context) (3 ファイル) | `data/context/*.toml` | 一日→ツイタチ/イチニチ 等の前後トークン依存 |
| 日付 | [`rules/days.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/days.toml) | `data/days.toml` | 1〜31 日の特殊読み (1→ツイタチ 等) |
| 大数 | [`rules/scales.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/scales.toml) | `data/scales.toml` | 万 / 億 / 兆 / 京 / 垓 ... |
| SI 単位 | [`rules/units.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/units.toml) | `data/units.toml` | km / kg / mL ... (case-insensitive: km/KM/Km どれも hit) |
| 記号 | [`rules/symbols.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/symbols.toml) | `data/symbols.toml` | + / − / % / ‰ ... |
| ラテン文字 | [`rules/latin.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/latin.toml) | `data/latin.toml` | A→エー / B→ビー ... |
| 例外語句 | [`rules/numeric_phrases.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/numeric_phrases.toml) | `data/numeric_phrases.toml` | 二十歳→ハタチ 等の数字を含む慣用句 |

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
