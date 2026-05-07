# アーキテクチャ概要

ja-furigana の crate 構成と内部モジュールの役割。

> 戻る: [README](../README.md) / 関連: contributors 向けの編集ガイドは [CONTRIBUTING.md](../CONTRIBUTING.md)

## crate 構成

```
crates/
├── furigana/                 # lib crate (crates.io 名: ja-furigana / [lib] name = "furigana")
│   ├── lib.rs                # module 宣言 + 公開 API re-export (ファサード)
│   ├── api.rs                # Furigana 構造体 + FuriganaBuilder (公開 API のエントリ)
│   ├── analyzer.rs           # Lindera + IPADIC のラッパ (形態素解析)
│   ├── kana.rs               # ひら⇄カタ + Unicode 正規化
│   ├── dict.rs               # surface→reading 辞書 (jukugo ≥2 文字 / unihan 1 文字 を内部分離、 loanwords/ subdir は再帰 walk から除外)
│   ├── loanwords.rs          # 外来語 (IT 用語等の英単語) 辞書、 case-fold + 全角→半角 + 完全一致 lookup
│   ├── single_overrides.rs   # 1 字 surface に対する明示的 default 上書き (issue #15 限定解、 Step 4)
│   ├── sanitize.rs           # 辞書 load 経路の sanitize layer (任意コード埋め込み防御 — 制御文字 / bidi override / zero-width / 過大長 reject)
│   ├── tts.rs                # TTS 整形 (normalize_for_tts) + segment_for_tts
│   ├── romaji.rs             # ひらがな → ローマ字 (ヘボン式 / 訓令式)
│   ├── error.rs              # FuriganaError / Result
│   ├── loader.rs             # TOML 汎用 parser (parse_toml<T> + load_or_default<T>)
│   ├── embedded.rs           # 空 default RulesData (rules は furigana-dict 側)
│   ├── rules/                # データスキーマ
│   │   ├── counters.rs       #   counters/*.toml
│   │   ├── context.rs        #   context/*.toml (rules + matches)
│   │   ├── days.rs           #   days.toml (1〜31 日特殊読み)
│   │   ├── scales.rs         #   scales.toml (大数: 万 / 億 / 兆…)
│   │   ├── units.rs          #   units.toml (km / kg / 円 / % …)
│   │   ├── symbols.rs        #   symbols.toml (+/-/% etc)
│   │   ├── latin.rs          #   latin.toml (A→エー…)
│   │   ├── numeric_phrases.rs #  慣用句 (二十歳→ハタチ等)
│   │   ├── compat.rs         #   compat.toml (異体字)
│   │   └── postprocess.rs    #   postprocess.toml (Step 7 (mode 別後処理 regex) regex 置換)
│   ├── numbers/              # 数値処理 (data-driven)
│   │   ├── helpers.rs        #   zen2han / norm_num / sokuonize_last / kansuji_to_arabic 等
│   │   ├── digit.rs          #   number_to_katakana
│   │   ├── counter.rs        #   euphonic_counter_read
│   │   ├── phrase.rs         #   NumericPhraseMatcher (慣用語句)
│   │   └── extras.rs         #   scale / si_unit / symbol 単発読み
│   ├── chunks/               # テキスト全体の数値・固有語チャンク分割
│   │   ├── mod.rs            #   NumberChunker + split() (URL/日付/jukugo prefix/loanword/scale/SI/counter/symbols/digit の階層的優先確定) + Aho-Corasick 共有
│   │   └── regex.rs          #   静的 / 動的 regex builder (DATE_NUM_PAT は漢数字も対応 / LOANWORD_RE は ASCII + 全角英字 + 記号)
│   └── reading/              # 読み解決パイプライン
│       ├── mod.rs            #   ReadingToken + tokenize_text (top-level)
│       ├── pipeline.rs       #   tokenize_chunk + resolve_reading (5 段階優先順位)
│       ├── merge.rs          #   merge_with_dict (最長一致結合、jukugo のみ参照)
│       ├── context.rs        #   apply_context_rules (data-driven)
│       └── output.rs         #   tokens_to_hiragana / tokens_to_ruby
│
└── furigana-cli/             # bin crate (crates.io 名: ja-furigana-cli / バイナリ名: furigana)
    └── src/
        ├── main.rs           # clap dispatch (引数なしなら repl にフォールバック)
        ├── paths.rs          # 実行ファイル横を default、--data-dir / FURIGANA_DATA_DIR で上書き
        ├── config.rs         # config.toml ロード ([server] / [auth].tokens / .admin_tokens)
        └── commands/
            ├── mod.rs        # build_furigana (各コマンドが共有する Furigana 組み立て)
            ├── lookup.rs     # furigana lookup
            ├── repl.rs       # furigana repl (rustyline + Tab 補完 + 履歴)
            ├── dict.rs       # furigana dict {add,list,remove,import,pull}
            ├── dict_pull.rs  #   pull 実装 (GitHub Releases + SHA-256 検証 + tar 展開)
            └── serve/        # furigana serve (Axum HTTP)
                ├── mod.rs    #   run() + Args + shutdown_signal + SIGHUP reload
                ├── handlers.rs # /furigana / /healthz / /admin/reload + do_reload
                ├── auth.rs   #   X-API-Key / Bearer middleware (一般 + admin) + CORS
                └── types.rs  #   FuriganaParams / FuriganaResponse / AppState
```

## 公開 API (lib)

```rust
use furigana::{Furigana, FuriganaBuilder, RomajiStyle, TtsOptions};

let mut f = Furigana::minimal()?;          // Lindera は lazy init
f.add_reading("灰桜", "ハイザクラ");        // 動的に辞書追加
f.preload()?;                              // (option) Lindera を eager init

let _ = f.tokenize("灰桜の道");
let _ = f.to_ruby("灰桜の道");
let _ = f.to_hiragana("灰桜の道");
let _ = f.to_tts("灰桜の道", &TtsOptions::default());
let _ = f.to_romaji("灰桜の道", RomajiStyle::Hepburn);
let _ = f.segment_tts("...", &TtsOptions::default(), 60);
let _ = f.dict_size();
f.merge_dict_toml(r#"[entries]
"黎明" = "レイメイ""#)?;
```

`FuriganaBuilder`:

```rust
let f = Furigana::builder()
    .rules_dir("/path/to/data")           // 単一上書き
    .core_dict_dir("/path/to/data")       // 複数追加可
    .user_dict_dir("/path/to/data/user")  // 複数追加可
    .overrides_file("/path/to/data/overrides.toml")  // 複数追加可
    .add_entry("追加語", "ツイカゴ")      // 最優先
    .build()?;
```

## 8 段階パイプライン (HTTP API と互換)

`tokenize_text` + `Furigana::to_*` の流れは本書のパイプラインに揃えてある:

| # | 工程 | 実装場所 |
|---|---|---|
| 1 | テキスト正規化 (NFKC + 互換マップ) | `kana::normalize_text` + `rules::compat` |
| 2 | 慣用語句 + 固有語 + 数値テキスト先行確定 | `numbers::NumericPhraseMatcher` (jukugo super-set check 付き) + `chunks::NumberChunker::split` (下記の階層) |
| 3 | 形態素解析 (Lindera + IPADIC) | `analyzer::Analyzer::tokenize` |
| 4 | 辞書照合 + 文脈ルール | `reading::merge::merge_with_dict` + `reading::context::apply_context_rules` |
| 5 | 漢字検出: 辞書 → 形態素 → unihan 順 | `reading::pipeline::resolve_reading` (下記) |
| 6 | surface 文字種で reading 表記を分岐 | `reading::output::tokens_to_hiragana` (下記の出力ルール) |
| 7 | 後処理 regex 置換 (mode 別) | `rules::postprocess::PostProcessData::apply` |
| 8 | TTS 正規化 (`tts` mode のみ) | `tts::normalize_for_tts` |

### Step 2 の詳細: `NumberChunker::split` の階層的優先確定

文頭から左→右にスキャンし、 各位置で以下の階層順に「読み確定済み chunk」 を切り出す
(早い階層で確定したらそこで切り出して次の位置へ、 後段は呼ばれない):

| 階層 | 役割 | 例 |
|---|---|---|
| 1 | URL / メール (skip、 読みなし) | `https://example.com` / `a@b.jp` |
| 2 | 和式日付 / 時刻 | `2025年10月30日` / `9時30分` / `9:30` |
| 4.5 | **jukugo prefix-match** (Aho-Corasick、 ≥3 字 jukugo + homonyms 除外) | 「千本桜」「義経千本桜」 を 1 chunk に固定して連濁 (センボンザクラ) を救う |
| 4.7 | **loanwords prefix-match** (英単語 chunk 全体を完全一致 lookup) | 「Kubernetes」「PostgreSQL」「TypeScript-config」 を 1 chunk として丸ごと切り出し |
| 5 | 数値 + 大数スケール (+ 末尾漢字単位) | 「1万円」「3億ドル」 |
| 6 | SI 単位 | 「100km」「3GB」 |
| 7 | 単一助数詞 (jukugo super-set check 付き) | 「3本」「12月」「1日」、 ただし jukugo entry が counter range を真に含む場合は jukugo 優先 |
| 8 | 記号 1 文字 | `+` / `-` / `%` / `〜` |
| 9 | 素の数字 | `12345` |

**設計ポイント**:

- 階層 4.5 / 4.7 / phrase_matcher の各層で jukugo Aho-Corasick automaton を **`Arc` 共有**
  し、 同 surface に対して複数経路が競合する場合は **「真上位集合となる jukugo entry が
  あれば jukugo を優先」** の super-set check で解決する。 「千本桜」 が
  `numeric_phrases.toml` の「千本=センボン」 に先取りされる問題 ([issue #18](https://github.com/RyuuNeko1107/ja-furigana/issues/18)) はこれで解決。
- 階層 4.7 の loanwords は **完全一致のみ** (substring 切断ゼロ)。 「PostgreSQL」 chunk が
  「Post」 entry に部分一致しても採用しない。 hit / miss どちらでも 1 chunk として確定し、
  miss でも Lindera 経路に渡らないので IPADIC 推測誤読を回避できる。
- jukugo AC の patterns 構築では:
  - **homonyms (`rules/context/*.toml` の `[[rule]] surface` 51 件) を除外** →
    reading pipeline の context rule (例: 「翡翠+が+水辺」 → カワセミ) を bypass しない
  - **≥3 字 jukugo のみ** を登録 → IPADIC が一語として返す長い複合語 (「烏賊墨」 →
    イカスミ、 「金平糖」 → コンペイトウ) を 2 字 jukugo (烏賊 / 金平) で先取りして
    分断する regression を回避

### Step 5 の詳細: `resolve_reading` の 6 段階優先順位

各 token に対して以下の順序で読みを決定:

1. **漢字を含まない** → `None` (surface のまま)
2. **context rule** ([`reading::context::apply_context_rules`]) — 同形異音語 (一日 / 上手 / 市場 等) の動的読み分け
3. **熟語辞書** (`Dict::lookup_jukugo`) — surface ≥ 2 文字の固定読み (灰桜=ハイザクラ 等)
4. **単漢字 default override** (`SingleOverrides::lookup`) — 1 字 surface に対する明示的 default 上書き (例: 「土 = ツチ」)。 全 unihan を Lindera より先にすると副作用大 ([issue #15](https://github.com/RyuuNeko1107/ja-furigana/issues/15) の R20 で 6 件 corpus regression 確認済み) のため、 明示的に override したい単漢字だけ別 data file で管理する限定解
5. **Lindera reading** — `details[7]` のカタカナ読み (動詞活用形などの自然な読み)
6. **単漢字辞書** (`Dict::lookup_unihan`) — surface = 1 文字の最終 fallback (5 水準別 file `core/unihan/joyo.toml` 等を再帰 walk で merge)

**過去の落とし穴 (現在は解決)**: 旧 0.1.0-alpha.2 までは「dict.lookup → context rule → Lindera」の順だった結果、`unihan` に登録された単漢字読み (能=あたう、本=もと等の動詞活用形 / 訓読み) が `context rule` の `default` を遮断していた。0.1.0-alpha.3 で `Dict` を `jukugo` (≥2 文字) と `unihan` (1 文字) に分離 + Step 4 (Lindera) を Step 5 (unihan) より先に評価する形に変更して根本解決。 alpha.8 で SingleOverrides を Step 4 に挟み、 unihan 一律優先化の副作用を避けつつ個別 override を可能に。

### Step 6 の詳細: `tokens_to_hiragana` の出力ルール (surface 文字種で分岐)

`tokens_to_hiragana` は token の **surface 文字種** で reading 表記を切り替える:

- **漢字を含む surface** → reading をひらがな化 (`kata_to_hira`)
  - 例: 「灰桜」 + ハイザクラ → 「はいざくら」 / 「3本」 + サンボン → 「さんぼん」
- **漢字を含まない surface** (ASCII / 全角英字 / カタカナ / ひらがな / 数字 / 記号) →
  reading を **カタカナに統一** (`hira_to_kata`)
  - 例: 「Kubernetes」 + クバネティス → 「クバネティス」 (ASCII カタカナ維持)
  - 例: 「3」 + サン → 「サン」 / 「〜」 + から → 「カラ」
    (`symbols.toml` の ひらがな登録もカタカナに揃える)

これにより 「Anthropic の Claude を使う」 のような ASCII + 漢字混在文では
「アンソロピックのクロードをつかう」 のように **アルファベット部分だけがカタカナ
維持** され、 自然な日本語混在表記になる。

## Lindera の lazy init

`Furigana::minimal()` / `FuriganaBuilder::build()` の時点では Analyzer を init せず、最初の `tokenize` / `to_*` 呼び出し時に [`OnceLock`] で 1 度だけ init される。

`Furigana::minimal()` 単体の bench で **5.97 ms → 27.3 µs (-99.5%)**。CLI レベルでは `--version` / `--help` 等の Lindera 不要経路が ~80 ms → ~10 ms に高速化。`furigana serve` は `Furigana::preload()` を listen 前に呼んでいるので、最初のリクエストレイテンシは劣化しない。

## hot reload (server)

```
client                                    server (Arc<RwLock<Arc<Furigana>>>)
  |                                              |
  | POST /admin/reload (admin token)             |
  |--------------------------------------------->|
  |                                              |  spawn_blocking { build_furigana(paths) }
  |                                              |   →  let new = Arc::new(...)
  |                                              |   →  *state.furigana.write() = new
  |  {"status":"reloaded","dict_size":N} <-------|
  |                                              |
  | GET /furigana?... (一般 token)                |
  |--------------------------------------------->|
  |                                              |  let f = state.furigana.read().clone();
  |                                              |  // RwLock 解放
  |                                              |  process(f.as_ref(), &params)
  |  ...                                         |
```

`RwLock<Arc<Furigana>>` で reload 中も既存リクエストが使い切る前の Arc を握り続けられるため、ダウンタイムなし。

## 設計判断のメモ

- **decisions.md にしない**: ADR (Architecture Decision Records) は今のところ書くほどのスコープではないので、本書で軽くメモする方針
- **データ駆動 (TOML)**: ルール変更で再ビルド不要、PR が contributors からも入りやすい
- **Lindera + IPADIC 固定**: `embed-ipadic` で配布物に同梱。NEologd は opt-in feature flag で対応する案 (Phase 3 候補、[Issue #9](https://github.com/RyuuNeko1107/ja-furigana/issues/9))
- **`Dict` の jukugo / unihan 分離**: 内部 HashMap を 2 つに分けて `lookup_jukugo` / `lookup_unihan` で別 lookup。Step 5 の 5 段階優先順位 (`context rule → jukugo → Lindera → unihan`、上記参照) に揃えるため。
- **`Dict::from_toml_dir` の `loanwords/` skip**: `Dict::from_toml_dir` は `core/` 配下を全階層再帰するが、 `loanwords/` サブディレクトリは **意図的に skip** する。 ASCII surface (「TypeScript」 等) を jukugo に混入させると階層 4.5 の jukugo prefix-match で誤って hit してしまうため、 `Loanwords::from_toml_dir` で別管理する。
- **`Dict::from_toml_dir` 全階層再帰** (loanwords/ 以外): `core/works/<medium>/<title>.toml` のような任意深度のサブディレクトリを許容。配布 tar.gz の展開結果を想定するため symlink ループ対策は持たない (静的データ前提)。works/ の運用ルールは [`ja-furigana-dict/core/works/README.md`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/works/README.md) (公式読みのみ採録、出典コメント必須)。
- **`Loanwords` (loanwords.rs) の case-fold + 全角→半角 + 完全一致**: IT 用語の英単語等を扱う独立 data type。 dict 登録 surface は canonical form (大文字始まり) で書くが、 lookup 時に正規化することで「Kubernetes」 / 「kubernetes」 / 「Ｋｕｂｅｒｎｅｔｅｓ」 すべて同 entry に hit する。 完全一致のみ (substring 切断ゼロ) で、 chunk 全体に対して lookup する。
- **`postprocess.toml` (Step 7)**: 辞書 / context rule で表現しづらい文字列レベルの最終調整 (例: 「ジュウパー → ジュッパー」の促音化補正)。mode 別 (`hiragana` / `ruby` / `tts` / `romaji`) フィルタ + regex pattern + capture group 参照可。
- **`NumberChunker` の漢数字対応**: `kansuji_to_arabic` (`numbers::helpers`) で「一」「二十一」等を Arabic に変換し、`DATE_KANJI_MD_RE` 等の日付 regex でマッチ。「6月一日」が正しく日付 chunk として認識される。
- **counter「日」の単独 = 期間扱い**: `chunks::NumberChunker` で `read_counter` (単独) と `read_counter_in_date` (日付内) を分離。`days.toml` の特殊読み (1=ツイタチ等) は日付 chunk 内でのみ採用。「1日に 2〜3回」は単独経由で「イチニチに」、「6月1日に」は日付経由で「ツイタチに」になる。
- **scale + 漢字単位連結**: `build_scale_regex(scales, units)` で末尾に「漢字 1 文字 unit」(円 / %) を optional capture (3) として注入。「1万円」のような scale + unit 連結が 1 chunk として処理される。
- **WASM は無し**: 一度実装したが `.wasm` が 57 MB と重いため削除。Web からは `furigana serve` (HTTP API) を推奨

## 関連リンク

- 公開 rustdoc: [docs.rs/ja-furigana](https://docs.rs/ja-furigana)
- contributors 向けの編集ガイド: [CONTRIBUTING.md](../CONTRIBUTING.md)
- ロードマップ: [ROADMAP.md](./ROADMAP.md)
