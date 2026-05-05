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
│   ├── dict.rs               # 単純 surface→reading 辞書 (HashMap)
│   ├── tts.rs                # TTS 整形 (normalize_for_tts) + segment_for_tts
│   ├── romaji.rs             # ひらがな → ローマ字 (ヘボン式 / 訓令式)
│   ├── error.rs              # FuriganaError / Result
│   ├── loader.rs             # TOML 汎用 parser (parse_toml<T> + load_or_default<T>)
│   ├── embedded.rs           # 空 default RulesData (rules は furigana-dict 側)
│   ├── rules/                # データスキーマ (counters / context / scales / units / ...)
│   ├── numbers/              # 数値処理 (data-driven)
│   │   ├── helpers.rs        #   zen2han / norm_num / sokuonize_last 等
│   │   ├── digit.rs          #   number_to_katakana
│   │   ├── counter.rs        #   euphonic_counter_read
│   │   ├── phrase.rs         #   NumericPhraseMatcher (慣用語句)
│   │   └── extras.rs         #   scale / si_unit / symbol 単発読み
│   ├── chunks/               # テキスト全体の数値チャンク分割
│   │   ├── mod.rs            #   NumberChunker + split()
│   │   └── regex.rs          #   静的 / 動的 regex builder (Option<Regex> で空時 None)
│   └── reading/              # 読み解決パイプライン
│       ├── mod.rs            #   ReadingToken + tokenize_text (top-level)
│       ├── pipeline.rs       #   tokenize_chunk + resolve_reading
│       ├── merge.rs          #   merge_with_dict (最長一致結合)
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

## 形態素解析の流れ

1. `Furigana::tokenize(text)` 呼び出し
2. `chunks::NumberChunker::split` で数値チャンク (3冊 / 5KM / 10:30 / 2026-04-26 等) を切り出し
3. 数値チャンクは `numbers::*` のルールで読み解決
4. 残りは `analyzer::Analyzer::tokenize` で Lindera に流す
5. `reading::merge::merge_with_dict` で辞書 (`overrides → user → core`) を **最長一致** で適用
6. `reading::context::apply_context_rules` で前後トークンを見て読み確定 (一日→ツイタチ等)
7. `reading::output::tokens_to_hiragana` / `tokens_to_ruby` で目的形式に変換

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
- **WASM は無し**: 一度実装したが `.wasm` が 57 MB と重いため削除。Web からは `furigana serve` (HTTP API) を推奨

## 関連リンク

- 公開 rustdoc: [docs.rs/ja-furigana](https://docs.rs/ja-furigana)
- contributors 向けの編集ガイド: [CONTRIBUTING.md](../CONTRIBUTING.md)
- ロードマップ: [ROADMAP.md](./ROADMAP.md)
