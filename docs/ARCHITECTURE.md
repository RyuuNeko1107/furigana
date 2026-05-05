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
│   ├── dict.rs               # surface→reading 辞書 (jukugo ≥2 文字 / unihan 1 文字 を内部分離)
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
│   │   └── postprocess.rs    #   postprocess.toml (本番 Step 7 互換 regex 置換)
│   ├── numbers/              # 数値処理 (data-driven)
│   │   ├── helpers.rs        #   zen2han / norm_num / sokuonize_last / kansuji_to_arabic 等
│   │   ├── digit.rs          #   number_to_katakana
│   │   ├── counter.rs        #   euphonic_counter_read
│   │   ├── phrase.rs         #   NumericPhraseMatcher (慣用語句)
│   │   └── extras.rs         #   scale / si_unit / symbol 単発読み
│   ├── chunks/               # テキスト全体の数値チャンク分割
│   │   ├── mod.rs            #   NumberChunker + split() + read_counter / read_counter_in_date
│   │   └── regex.rs          #   静的 / 動的 regex builder (DATE_NUM_PAT は漢数字も対応)
│   └── reading/              # 読み解決パイプライン
│       ├── mod.rs            #   ReadingToken + tokenize_text (top-level)
│       ├── pipeline.rs       #   tokenize_chunk + resolve_reading (本番互換 5 段階優先順位)
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

## 8 段階パイプライン (本番 ryuuneko.com の公開フリガナ API と互換)

`tokenize_text` + `Furigana::to_*` の流れは本番のフリガナ API パイプラインに揃えてある:

| # | 工程 | 実装場所 |
|---|---|---|
| 1 | テキスト正規化 (NFKC + 互換マップ) | `kana::normalize_text` + `rules::compat` |
| 2 | 数値・日付・時刻・カウンタ先行確定 | `numbers::NumericPhraseMatcher` + `chunks::NumberChunker::split` |
| 3 | 形態素解析 (Lindera + IPADIC) | `analyzer::Analyzer::tokenize` |
| 4 | 辞書照合 + 文脈ルール | `reading::merge::merge_with_dict` + `reading::context::apply_context_rules` |
| 5 | 漢字検出: 辞書 → 形態素 → unihan 順 | `reading::pipeline::resolve_reading` (下記) |
| 6 | カナ→ひらがな統一 | `reading::output::tokens_to_hiragana` (`kana::katakana_to_hiragana` 経由) |
| 7 | 後処理 regex 置換 (mode 別) | `rules::postprocess::PostProcessData::apply` |
| 8 | TTS 正規化 (`tts` mode のみ) | `tts::normalize_for_tts` |

### Step 5 の詳細: `resolve_reading` の 5 段階優先順位

各 token に対して以下の順序で読みを決定:

1. **漢字を含まない** → `None` (surface のまま)
2. **context rule** ([`reading::context::apply_context_rules`]) — 同形異音語 (一日 / 上手 / 市場 等) の動的読み分け
3. **熟語辞書** (`Dict::lookup_jukugo`) — surface ≥ 2 文字の固定読み (灰桜=ハイザクラ 等)
4. **Lindera reading** — `details[7]` のカタカナ読み (動詞活用形などの自然な読み)
5. **単漢字辞書** (`Dict::lookup_unihan`) — surface = 1 文字の最終 fallback

**過去の落とし穴 (現在は解決)**: 旧 0.1.0-alpha.2 までは「dict.lookup → context rule → Lindera」の順だった結果、`unihan` に登録された単漢字読み (能=あたう、本=もと等の動詞活用形 / 訓読み) が `context rule` の `default` を遮断していた。0.1.0-alpha.3 で `Dict` を `jukugo` (≥2 文字) と `unihan` (1 文字) に分離 + 上記の優先順位に変更して根本解決。

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
- **`Dict` の jukugo / unihan 分離**: 内部 HashMap を 2 つに分けて `lookup_jukugo` / `lookup_unihan` で別 lookup。本番 ryuuneko.com の Step 5 「辞書 → 形態素 → unihan」の 3 段階優先順位に揃えるため (0.1.0-alpha.3 で導入)。
- **`postprocess.toml` (Step 7)**: 辞書 / context rule で表現しづらい文字列レベルの最終調整 (例: 「ジュウパー → ジュッパー」の促音化補正)。mode 別 (`hiragana` / `ruby` / `tts` / `romaji`) フィルタ + regex pattern + capture group 参照可。
- **`NumberChunker` の漢数字対応**: `kansuji_to_arabic` (`numbers::helpers`) で「一」「二十一」等を Arabic に変換し、`DATE_KANJI_MD_RE` 等の日付 regex でマッチ。「6月一日」が正しく日付 chunk として認識される。
- **counter「日」の単独 = 期間扱い**: `chunks::NumberChunker` で `read_counter` (単独) と `read_counter_in_date` (日付内) を分離。`days.toml` の特殊読み (1=ツイタチ等) は日付 chunk 内でのみ採用。「1日に 2〜3回」は単独経由で「イチニチに」、「6月1日に」は日付経由で「ツイタチに」になる。
- **scale + 漢字単位連結**: `build_scale_regex(scales, units)` で末尾に「漢字 1 文字 unit」(円 / %) を optional capture (3) として注入。「1万円」のような scale + unit 連結が 1 chunk として処理される。
- **WASM は無し**: 一度実装したが `.wasm` が 57 MB と重いため削除。Web からは `furigana serve` (HTTP API) を推奨

## 関連リンク

- 公開 rustdoc: [docs.rs/ja-furigana](https://docs.rs/ja-furigana)
- contributors 向けの編集ガイド: [CONTRIBUTING.md](../CONTRIBUTING.md)
- ロードマップ: [ROADMAP.md](./ROADMAP.md)
