# Changelog

このプロジェクト (`ja-furigana` lib + `ja-furigana-cli` bin) の変更履歴。
[Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) 形式に概ね従い、
バージョニングは [Semantic Versioning](https://semver.org/lang/ja/) を採用。

## [Unreleased]

### Security (攻撃面: 辞書 / HTTP 入力)

- **`furigana dict pull` の archive 展開強化**:
  - download 合計サイズ上限 `MAX_DOWNLOAD_BYTES = 50 MB` (Content-Length と
    実 body の両方で post-check、 帯域 / disk DoS 防御)
  - 展開合計サイズ上限 `MAX_UNCOMPRESSED_TOTAL = 200 MB` (archive bomb 防御)
  - 1 entry サイズ上限 `MAX_PER_ENTRY_BYTES = 10 MB`
  - entry 数上限 `MAX_ENTRY_COUNT = 50,000` (大量小ファイル zip bomb 防御)
  - **entry type 制限**: Regular file / Directory のみ許可。 symlink /
    hardlink / char device / block device / fifo は **絶対 reject**
    (path traversal + sensitive file 露出の典型攻撃 vector を構造的に潰す)
- **`furigana serve` の HTTP body / rate limit**:
  - `tower_http::limit::RequestBodyLimitLayer` で body 上限 1 MB
    (巨大 POST による memory blow を防御)
  - `tower_governor` で rate limit 1 req/sec + burst 5 per IP
    (request flood / brute-force 攻撃の減速)
- **ReDoS audit**: lib 内 regex (loanword / 数値 / 日付 等) を audit、
  `regex` crate の linear-time 保証 (NFA-based、 catastrophic backtracking
  起きない) で OK と確認、 修正不要。
- **任意コード実行 audit** (辞書 + 入力経由):
  - lib + cli に `unsafe` block **0 件** → memory unsafety 経由 RCE 不可
  - shell exec **無し** → command injection 不可
  - DB / SQL **無し** → SQL injection 不可
  - eval / dynamic exec **無し** (rust に存在しない)
  - TOML deserialize は serde の `HashMap<String, String>` のみ → gadget 攻撃不可
  - 入力 text は data として扱われ regex pattern に流入する経路 **無し**
  - HTTP handler panic は axum default で 500 catch、 process は落ちない
  - 唯一の懸念: **regex bomb** (postprocess.toml / numeric_phrases.toml の
    pattern が巨大 regex として compile されると memory 消費)
    → `RegexBuilder::size_limit(10 MB)` で compile 拒否を追加
- 既存対策 (path traversal canonicalize、 SHA-256 sidecar 検証、 admin
  token 認証、 CORS layer、 `set_preserve_permissions(false)`、 server
  text 文字数 `MAX_TEXT_LEN`) は維持。
- **固定フォーマット以外の入力 audit + 追加対策**:
  - HTTP `mode` パラメータ: 既存の `normalize_mode` whitelist
    (`tts`/`hiragana`/`ruby`/`kanji`/`romaji`/`romaji-kunrei`) で OK
  - **HTTP auth token (X-API-Key / Bearer): timing-safe 比較に変更** —
    `subtle::ConstantTimeEq` で全 byte を見比べた結果に縮約。 単純 `==`
    だと一致 prefix 長が処理時間差に漏れて char-by-char 推測される
  - **GitHub API `tag_name` の strict format validate** — `[A-Za-z0-9.\-]`
    のみ・ 連続 `..` 禁止 ・1〜64 文字に限定。 `..` や `/` `:` 注入で
    別 release / 別 host を pull する攻撃を構造的に防御。 `dict pull`
    の URL 組立て前に `validate_tag_format` を必ず通す
  - **CLI `dict add` の制御文字 reject** — surface / reading に C0 制御文字
    (NULL や U+0001..U+0008 等、 `\t` `\n` `\r` 以外) を含む入力を reject。
    既存 `toml_escape` (`"` `\` `\n` `\r` `\t`) で TOML breaking char は
    既に escape 済み、 残る self-DoS 経路を構造的に塞ぐ
  - HTTP CORS Origin / GitHub JSON parse / 環境変数 path 等の他経路は
    現行 default で安全 (axum / serde / std::path の既存防御で吸収)
- **辞書 load 経路の sanitize layer** (任意コード埋め込み / 詐称防御):
  TOML 自体の deserialize で RCE は起きないが、 entries の **value** に紛れ
  込ませて間接的に害を及ぼす経路を構造的に塞ぐ。 新設 `crate::sanitize::
  sanitize_dict_value` で各 surface / reading を load 時 reject する:
  - **C0 制御文字 / DEL** (`\t` `\n` `\r` 以外) → log injection / display 破壊
    / 書き戻し時の TOML parse 全体破壊 (self-DoS) 防御
  - **Unicode bidi override** (U+202A..U+202E、 U+2066..U+2069) → Trojan Source
    攻撃 (PR review でコード意味と見た目が乖離する) 防御
  - **Zero-width / invisible char** (U+200B..U+200F、 U+FEFF) → homoglyph 詐称
    (一見同じ surface で別 entry を仕込む) 防御
  - **excessive length per entry** (1024 chars 上限) → 1 entry に巨大 string
    で OOM させる経路を塞ぐ
  - 適用先: `Dict::from_toml_str` (jukugo / unihan / works) +
    `Loanwords::from_toml_str` + `SingleOverrides::from_toml_str`
  - 公開 ja-furigana-dict (CJK + kana + ASCII + 通常記号のみ) は影響なし、
    既存 corpus 118/118 pass 確認

## [0.1.0-alpha.8] - 2026-05-07

alpha.7 のリリースをやり直したもの。 binary 内容と機能は alpha.7 と実質同じ
(loanwords / 出力ルール / lookup priority / 踊り字 / SingleOverrides)。
alpha.7 を捨てた理由 2 つ:

1. **Immutable Releases policy** が repo で有効化されたタイミングで `gh release
   create` が即 immutable lock をかけ、 binary upload step が全 platform で
   HTTP 422 を返した。 release.yml を draft → finalize 構造に直したものの、
   alpha.7 tag は GitHub 内部 ledger に「使用済み」 として永久登録され、 同名
   tag の再 create が `Cannot create ref due to creations being restricted`
   で reject された (immutable releases を OFF にしても解除されない)。
2. **個人 email** が SECURITY.md / Cargo.toml workspace authors に直書き
   され alpha.6 と alpha.7 の crates.io author metadata に焼き付いた。 git
   history は filter-repo で全 commit から除去 + force push で消したが、
   crates.io publish 済み metadata は変えられないため、 alpha.6 / alpha.7
   は yank で対処。 alpha.8 から `mail@ryuuneko.com` author で再 publish。

機能差分は **無し** (alpha.7 と同じ)。 ci(release.yml) の draft + finalize 構造
だけが追加で含まれる。 詳細な機能変更点は下記 alpha.7 セクションを参照。

### Changed (release workflow)

- `.github/workflows/release.yml` を Immutable Releases policy 互換に修正:
  - `create-release` で `gh release create --draft` (publish 直後の immutable
    lock を回避)
  - 新 `finalize-release` job で binary 全 platform upload 完了後に
    `gh release edit --draft=false --latest` で publish 化

### Yanked (crates.io)

- `ja-furigana@0.1.0-alpha.6`, `ja-furigana@0.1.0-alpha.7`,
  `ja-furigana-cli@0.1.0-alpha.6`, `ja-furigana-cli@0.1.0-alpha.7` を
  cargo yank 済み。 alpha.5 以前は author 欄に問題があるため新規利用は
  alpha.8+ を推奨 (alpha.5 以前の yank はしない、 古い author 残置)。

## [0.1.0-alpha.7] - 2026-05-07 (yanked, see alpha.8)

下記内容は alpha.8 にも全て含まれる (alpha.7 と alpha.8 は機能同一)。 alpha.7
tag / GitHub Release は immutable ledger 残置 (再 create 不可)、 crates.io
は yank 済み。

外来語 (loanwords) サポート + 出力ルール仕様変更 + lookup priority 強化 +
踊り字「々」 自動展開 + 単漢字 default override + 検証ループで発見した動詞活用
系 bug の dict 側修正 を取り込んだ大型リリース。 alpha.6 で欠けていた Docker
image build もこの release で復旧する (MSRV 1.89 bump 込み)。 まだ alpha 中なので
公開 API は破壊的変更ありえる点に注意。

### Changed (MSRV)

- **MSRV を 1.88 → 1.89 に bump**: rustyline 18 (alpha.4 で取り込み) が
  std::fs::File::lock に依存するようになり、これが 1.89 で安定化した機能のため。
  alpha.6 release の Docker build (rust:1.88-slim ベース) が `file_lock` 不安定
  エラーで失敗していた問題への対応。
  - `Cargo.toml` workspace `rust-version`: 1.88 → 1.89
  - `Dockerfile` builder image: `rust:1.88-slim` → `rust:1.89-slim`
  - `README.md` MSRV badge: 1.88+ → 1.89+
- alpha.6 GitHub release は binary upload (5 platform) は完了済、Docker image のみ
  欠けた状態で残置。Docker image は次の release で復旧予定。

### Added (loanwords / IT 用語の英単語対応)

- **`Loanwords` data type** (`crates/furigana/src/loanwords.rs`):
  - `[entries]` 形式の TOML を recursive load (`core/loanwords/**/*.toml`)
  - **case-fold + 全角→半角 正規化** + **完全一致 lookup** (substring 切断ゼロ)
  - 「Kubernetes」「kubernetes」「Ｋｕｂｅｒｎｅｔｅｓ」 すべて同じ entry に hit
- **`chunks/split()` 階層 4.7** (jukugo prefix-match の後、 scale より前):
  - regex `[A-Za-zＡ-Ｚａ-ｚ][A-Za-z0-9...+#._\-]*` で英単語 chunk を **1 unit
    として丸ごと切り出し** (Lindera/IPADIC が token 単位でぶった切るのを防ぐ)
  - chunk 全体に対して loanwords lookup
    - hit → reading 確定 chunk
    - miss → ASCII surface のまま読みなしで残す (Lindera 経路に渡らないので
      IPADIC 推測誤読も発生しない)
- **`Furigana::builder().core_loanwords_dir(p)`** API 追加
- **`<data_dir>/data/loanwords/`** を CLI auto-load (`furigana lookup` /
  `furigana serve` 等で透過的に使える)
- **`Dict::from_toml_dir` の再帰 walk から `loanwords/` を除外**:
  - これは ASCII surface 専用で `Loanwords` 側で別管理されるため、 jukugo / unihan に
    混入させると jukugo prefix-match で「TypeScript」 等が誤って hit する問題があった
- 関連 GitHub issue: [#19 (closed)](https://github.com/RyuuNeko1107/ja-furigana/issues/19)

### Changed (出力ルール仕様変更: surface 文字種で reading 表記を切替)

`reading::output::tokens_to_hiragana` の出力ルールを surface 文字種で分岐:

- **漢字を含む surface** → reading をひらがな化 (既存挙動)
  - 「灰桜」 + ハイザクラ → 「はいざくら」
- **漢字を含まない surface** (ASCII / 全角英字 / カタカナ / ひらがな / 数字 / 記号) →
  reading を **カタカナに統一** (`hira_to_kata` 適用)
  - 「Kubernetes」 + クバネティス → 「クバネティス」 (ASCII カタカナ維持)
  - 「3」 + サン → 「サン」 (数字 chunk もカタカナ)
  - 「〜」 + から → 「カラ」 (symbols.toml の ひらがな登録もカタカナに揃える)
  - 「3本」 (漢字「本」 含む) → 「さんぼん」 (既存通りひらがな化)

これにより 「Anthropic の Claude を使う」 → 「アンソロピックのクロードをつかう」 の
ような自然な日本語混在表記が出るようになった。 ja-furigana-dict 側 corpus でも
ASCII / 数字 / 記号 を含む 4 件の expected を追従更新。

### Added (本体側 issue 起票 — 検証ループ R12-R17 で副産物として発見)

- [#13](https://github.com/RyuuNeko1107/ja-furigana/issues/13) bug: 「淹れる」 → 「いれるれる」 (送り仮名二重出力)
- [#14](https://github.com/RyuuNeko1107/ja-furigana/issues/14) bug: 「点ける」 → 「てんける」 (単漢字 unihan default が動詞活用を上書き)
- [#15](https://github.com/RyuuNeko1107/ja-furigana/issues/15) bug: unihan default が Lindera reading に override される (鋸 / 土 等)
- [#16](https://github.com/RyuuNeko1107/ja-furigana/issues/16) feat: 踊り字 「々」 の自動展開 (神々 → かみがみ)
- [#17](https://github.com/RyuuNeko1107/ja-furigana/issues/17) bug: 動詞 default reading 選択ズレ (摘む → つまむ)
- [#18](https://github.com/RyuuNeko1107/ja-furigana/issues/18) (closed) perf/lookup priority: 助数詞 / numeric_phrases が jukugo 最大マッチングを阻害 — 修正済み

### Changed (lookup priority — issue #18 解決)

- **`NumericPhraseMatcher` と `NumberChunker` に jukugo Aho-Corasick automaton を Arc 共有**:
  - phrase / counter / scale が jukugo entry の真部分集合を切り出してしまう問題を解決
  - 例: 「千本桜」 で「千本」 を numeric_phrases (千本=センボン) が先取りしていた
    → jukugo「千本桜」 を super-set check で優先採用 → 「センボンザクラ」 (連濁ザ) で出力
  - 副作用ゼロを担保:
    - **homonyms (`rules/context/*.toml` の `[[rule]] surface` 51 件) を AC patterns
      から除外** → reading pipeline の context rule (例: 「翡翠+が+水辺」 → カワセミ)
      は無傷
    - **≥3 字 jukugo のみ AC に登録** → IPADIC が一語で返す長い複合語
      (「烏賊墨」 → イカスミ、 「金平糖」 → コンペイトウ) を 2 字 jukugo
      (烏賊 / 金平) で先取り regression が出ない
- aho-corasick 1.x を依存に追加 (workspace 共有)

### Added (踊り字「々」 の自動展開 — closes #16)

- `reading::expand_odoriji_inplace` を tokenize_text の最終段に挿入。
  Lindera が 「神々」 を 神 + 々 にぶった切るのに対し、 後段で 々 token の
  reading を直前 token の reading で置き換える。
- **簡易連濁判定 `voice_first_kana`**: 直前 reading の先頭が `カ/サ/タ/ハ` 行
  なら `ガ/ザ/ダ/バ` に濁音化 (神々 → カミガミ、 国々 → クニグニ、 木々 → キギ
  等)。 `ナ/マ/ヤ/ラ/ワ/ア` 行は濁らないルール。
- 出力例:
  - 「神々」 → かみがみ
  - 「人々」 → ひとびと (ヒト + ビト)
  - 「日々」 → ひび
- 関連: [#16 (closed)](https://github.com/RyuuNeko1107/ja-furigana/issues/16)

### Added (単漢字 default override — issue #15 の限定解)

- **`SingleOverrides` data type** (`crates/furigana/src/single_overrides.rs`):
  - `[entries]` 形式 1 ファイル (`core/single_overrides.toml`) で 1 字 surface
    に対する明示的 default 上書きを管理
  - `lookup()` は内部で「surface が 1 字」 制約を課し、 ≥2 字 surface には影響
    しない (jukugo 分担を侵食しない)
- **`resolve_reading` 6 段階優先順位** に Step 4 として割り込み:
  1. 漢字なし → None
  2. context rule
  3. 熟語辞書
  4. **SingleOverrides** ← NEW
  5. Lindera reading
  6. unihan
- 「全 unihan を Lindera より先にすると副作用大」 (R20 の 6 件 corpus regression)
  が分かったので、 priority 全体を倒すのではなく **明示的に override したい
  単漢字だけ** を別 data file で管理する設計に着地。
- seed: `"土" = "ツチ"` 1 件 (ja-furigana-dict 側 `core/single_overrides.toml`)。
- 関連: [#15 (open、 限定解)](https://github.com/RyuuNeko1107/ja-furigana/issues/15)

### Security (CodeQL 起票)

- **GitHub Actions workflow に `permissions: contents: read` を明示**
  ([PR #20](https://github.com/RyuuNeko1107/ja-furigana/pull/20)、 Copilot
  Autofix 経由) — CodeQL alert "Workflow does not contain permissions" の修正。
  default token permission を最小化することで supply chain リスク低減。
- `SECURITY.md` 追加 (脆弱性報告手順、 サポートバージョンポリシー)。

### Chore

- `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings`
  を pass する状態に整流 (機能変更なし、 doc_overindented_list_items /
  doc_lazy_continuation 等の lint fix)。

## [0.1.0-alpha.6] - 2026-05-07

辞書ディレクトリの再帰スキャンを **無制限階層** に拡張。これにより
ja-furigana-dict 側で `core/works/game/touhou.toml`、`core/works/anime/<title>.toml`
のような作品単位 1 ファイルの細分化構造が利用可能になる。

### Changed (`Dict::from_toml_dir`)

- 旧: 直下 + サブディレクトリ 1 階層のみスキャン (`core/jukugo/general.toml` は OK、
  `core/jukugo/works/X.toml` は読まれなかった)
- 新: `collect_toml_files_recursive` で **任意の深さ** を再帰、絶対パス順に sort、
  後勝ちで merge。配布 tar.gz の展開結果を想定するため symlink ループ対策は持たない
  (静的データ + 配布側で混入し得ない前提)

### Added (test)

- `from_toml_dir_recurses_arbitrary_depth`: `works/game/series/touhou.toml` および
  `works/anime/placeholder.toml` の 3 階層構造でロード成功と lookup ヒットを確認

### Verification

171 lib unit test + 4 doctest + 1 integration (`load_real_data`) + 2 CLI unit
全 pass、clippy clean、fmt clean。ja-furigana-dict 側 v0.1.2 (24 ファイル /
`core/jukugo/*.toml` 1 階層構造) は新 loader でも完全互換 (旧 1 階層構造は新 loader の subset)。

## [0.1.0-alpha.5] - 2026-05-06

辞書の自動取得 / 自動更新を **admin_tokens 設定不要** で使えるようにした。
`/admin/reload` の認証は引き続き「外部から同期トリガーしたい運用者向け」に残置。

### Added (`furigana serve`)

- **`--auto-pull` フラグ**: 起動時に GitHub Releases から最新 `ja-furigana-dict`
  を自動取得してから listen 開始。失敗時は warn を出して既存辞書で起動を続行
  (network なし / GitHub 一時障害でも壊れない)。
  `[auto_update].pin` が空でなければそれを使う、空なら latest。
- **`[auto_update]` config セクション** (`config.toml`):
  - `enabled` (bool, default false): `furigana serve` 起動中の **定期 polling**
    background task を spawn する
  - `interval` (string, default `"24h"`): polling 周期。`"30m" / "1h" / "6h" / "1d"` 等
  - `pin` (string, default 空): 特定 tag に固定。空なら最新追従
- **`crate::commands::dict_pull::resolve_latest_tag_async`**: 既存の sync 版を
  spawn_blocking でラップ、auto_update polling 経路から呼びやすく

  これらは **admin_tokens 設定なしで動く** (内部呼び出しで HTTP 経由しない)。
  個人 / 小規模ホビー運用なら admin_tokens は不要、運用者向けに残置。

### CI

なし (lib のみ patch、CI 設定変更なし)

## [0.1.0-alpha.4] - 2026-05-06

依存ライブラリの major bump を一気に取り込む。「基本最新」方針を一貫させた。

### Changed (依存)

- **`toml` 0.8 → 1.x** (spec 1.1 対応の major release)
  - `loader.rs::parse_toml` / `dict.rs::Dict::from_toml_str` /
    `rules/postprocess.rs::PostProcessSpec` deserialize 経路を確認、互換動作。
  - 利用者影響なし (公開 API シグネチャ不変)。
- **`directories` 5 → 6** (`ProjectDirs` API)
  - `paths.rs` の `ProjectDirs::from("com", "furigana", "furigana")` 経路を確認、
    XDG fallback の挙動同じ。

### Added (Dependabot 自動 merge 経由)

- **`criterion` 0.5 → 0.8** (bench、`std::hint::black_box` 移行は 0.1.0-alpha.2 で
  対応済)。
- **`sha2` 0.10.9 → 0.11.0** (`furigana dict pull` の SHA-256 検証経路、互換動作)。

### CI

- `.github/dependabot.yml` の major bump ignore (toml / directories) を削除。
  今後の major bump も Dependabot が PR 提示する。

### Verification

170 unit test + 4 doctest + 3 integration test 全 pass、clippy clean、fmt clean。
NOTICE.md も `cargo about generate` で再生成済。

## [0.1.0-alpha.3] - 2026-05-06

本番 ryuuneko.com の公開フリガナ API パイプラインに揃える形で読み解決の
優先順位を整備。検証ループ (実例文 75 件回帰) で 75/75 (100%) を達成。

### Changed (本番 Step 5 互換: resolve_reading の優先順位を 5 段階に)

`crates/furigana/src/reading/pipeline.rs` の `resolve_reading` を:

1. 漢字なし → None
2. **context rule** (動的読み分け、同形異音語が効くように)
3. **熟語辞書 (jukugo)** (≥2 文字 surface の固定読み)
4. **Lindera reading** (動詞活用形等の自然な読み)
5. **単漢字辞書 (unihan)** (1 文字 surface の最終 fallback)
6. fallback None

旧版では「dict.lookup → context rule → Lindera」の順で、unihan に登録された
動詞活用形 / 訓読み (能=あたう、本=もと等) が context rule の default を遮断
していた問題を根本解決。

### Changed (Dict struct を jukugo / unihan に分離)

`crates/furigana/src/dict.rs` の `Dict` を内部で 2 つの HashMap に分割:
- `jukugo`: surface ≥ 2 文字 (熟語 / 固有名詞 / 複合語)
- `unihan`: surface = 1 文字 (単漢字 fallback)

新規 lookup API: `lookup_jukugo()` / `lookup_unihan()` / 互換 `lookup()` (jukugo 優先)。
`insert()` で文字数を自動振り分け。`merge()` / `len()` は両 HashMap を合算。
`merge_with_dict` も最長一致照合を `lookup_jukugo` のみに変更
(単漢字 unihan を熟語結合トリガにしない)。

### Added (本番 Step 7 互換: postprocess module)

`crates/furigana/src/rules/postprocess.rs` 新設:
- `PostProcessData`: コンパイル済み regex リスト + mode 別フィルタ
- `PostProcessSpec`: TOML deserialize 用 (`[[rule]]` array)
- `apply(text, mode)`: 順次置換、空なら no-op、`$1` 等のキャプチャ参照可

`Furigana::to_{hiragana,ruby,tts,romaji}` の出力直前に該当 mode で apply。
`loader.rs` で `postprocess.toml` を読み込み (不在なら default 空)。

ja-furigana-dict v0.1.2 で `rules/postprocess.toml` に「ジュウパー → ジュッパー」
(50% 促音化) を投入し、検証ループ #70 を解決。

### Added (NumberChunker の漢数字日付 + scale+unit 連結 + counter context)

- `numbers/helpers.rs` に `kansuji_to_arabic()`: 漢数字 (一〜二十一) → Arabic
  数字文字列。`chunks::NumberChunker::read_counter` で漢数字混在パターンを処理。
- `chunks/regex.rs` の `DATE_NUM_PAT` を Arabic + 全角数字 + 漢数字に拡張。
  `DATE_KANJI_FULL_RE` / `DATE_KANJI_MD_RE` が「6月一日」のような漢数字日も
  日付 chunk として認識。
- `build_scale_regex(scales, units)` に変更: scale 末尾に「漢字 1 文字 unit」
  (円 / %) を optional capture (3) として注入。「1万円」のような scale + unit
  パターンが 1 chunk として処理される。
- `chunks/mod.rs` に `read_counter_in_date` を新設: 日付内の「N日」は days.toml の
  特殊読み (1=ツイタチ等) を採用。単独 counter としての「N日」は期間文脈とみなし
  default ニチ にする。「6月一日」=ツイタチ / 「1日に2〜3回」=イチニチ を両立。

### Fixed

- `Furigana::add_reading` 経由で追加した単漢字エントリが、前優先順位 (旧) で
  Lindera reading より先に hit する問題を解決 (Dict 分離 + 新優先順位)。
- 検証ループで判明した動詞活用形 surface (差/能/約/見) の Lindera 出力に対し、
  jukugo 熟語登録 + unihan 音読み正規化で解決可能に。

### CI

- `.github/workflows/ci.yml`: macOS test を `schedule` のみ (週次) に移動。
  push / PR では `ubuntu-latest` + `windows-latest` の 2 OS で走らせる。
  GitHub macOS runner queue が常に混雑し PR が 10+ 分 macOS 待ちで詰まる問題の対策。
- 新規 `audit` job: `cargo-audit` で RustSec advisory DB を毎週 schedule + push / PR
  でチェック。`continue-on-error: true` (advisory DB 更新による偶発失敗を blocking にしない)。
- 新規 `corpus` job: ja-furigana-dict の master を checkout → release binary build →
  `tools/run_corpus.py` で `should_read.toml` の各 case を `expected` と diff 検証。

### Removed
- **役目を終えた reproducer test ファイル** 4 件を削除:
  - `crates/furigana/tests/lindera_minimal_repro.rs`
  - `crates/furigana/tests/furigana_layer_repro.rs`
  - `crates/furigana/tests/components_repro.rs`
  - `crates/furigana/tests/static_regex_repro.rs`

  これらは「`cargo test --release` 経由の 51 GB alloc 暴走」の原因切り分け用に
  作った一時的なファイル。原因 (`build_alt_regex` の never-match pattern) が
  特定されて修正済みなので、用途を終えた。詳細経緯は本 CHANGELOG の Fixed
  セクションに残してあり、必要なら git history で復元可能 (`git log -- ...`)。
  新しめの clippy 新規 lint (`explicit_iter_loop` / `unused_imports`) で
  繰り返し fail する原因にもなっていたため整理。

### Fixed
- **`cargo test --release` / `cargo bench` harness で `NumberChunker::split` が
  巨大 alloc (51 GB 級) で `STATUS_STACK_BUFFER_OVERRUN` を起こしていた問題を修正**。
  原因は `chunks::regex::build_alt_regex` で空 list 時に返していた never-match
  pattern `r"(?P<n>\A\B)(?P<x>\A\B)"` が、release ビルドの test/bench harness
  特有のメモリ allocator 状況下で内部 DFA 構築を暴発させていたこと。
  - `build_*_regex` を `Option<Regex>` 返却に変更し、空時は `None`
  - `NumberChunker.{counter_re,scale_re,si_unit_re}` を `Option<Regex>` に変更
  - `split` の各 dynamic regex 呼び出しを `if let Some(re) = ...` で gate
  - 過去 `#[ignore]` していた `api::tests::to_tts_*` 3 件を解禁、bench も
    medium / long テキスト復活。bench 実測: medium 51 µs (2.2 MiB/s) / long 263 µs。
  - **Lindera は最後まで無罪** (途中で疑った issue #326 とは無関係)。

### Added
- **ローマ字出力モード** (`--mode romaji` / `--mode romaji-kunrei`):
  - lib: `Furigana::to_romaji(text, RomajiStyle)` + 公開 `hiragana_to_romaji` 関数
  - CLI: `furigana lookup --mode romaji` / `--mode romaji-kunrei`
  - REPL: `mode romaji` / `mode romaji-kunrei`
  - HTTP API: `mode=romaji` / `mode=romaji-kunrei`
  - ヘボン式 (default): し→shi、ち→chi、つ→tsu、b/m/p 前 ん→m、母音/y 前 `'` 区切り
  - 訓令式: し→si、ち→ti、つ→tu、ん→常に n
  - 促音 (っ) は次の子音を重ねる、ヘボン式 ち系の前は t (motchi)
  - 長音 (ー) は直前の母音を repeat
- `Furigana::merge_dict_toml(content)` — TOML 文字列を辞書に一括 merge する API。
  ファイルシステムベースの `core_dict_dir` が使えない環境向け。
- `Furigana::preload()` — Lindera 形態素解析器を eager に初期化する API
  (server 起動時の preload 用)。
- `examples/clients/{python,nodejs,curl}/` — `furigana serve` HTTP API を他言語から
  叩く最小サンプル (TTS パイプライン / Discord bot / shell パイプ用途)。
- `crates/furigana/benches/lookup.rs` — criterion ベンチ (init / mode 別 / tokenize)。

### Changed
- **`Furigana` の Lindera 初期化を lazy に**。`Furigana::minimal()` /
  `FuriganaBuilder::build()` の時点では Analyzer を init せず、最初の
  `tokenize` / `to_*` 呼び出し時に [`OnceLock`] で 1 度だけ init。
  `Furigana::minimal()` 単体の bench で **5.97 ms → 27.3 µs (-99.5%)**。
  CLI レベルでは `--version` / `--help` 等の Lindera 不要経路が
  ~80 ms → ~10 ms に高速化。`furigana serve` は preload を起動時に呼んで
  最初のリクエストレイテンシを保つ。

### Removed
- `crates/furigana-wasm/` (WebAssembly bindings) を削除。`.wasm` が Lindera + IPADIC
  込みで 57 MB と重く、Web からは `furigana serve` (HTTP API) で十分という判断。
  Pages workflow (`.github/workflows/pages.yml`) も合わせて削除。
  `lib::merge_dict_toml` API は WASM 用に追加したが、サーバ無し環境からの利用にも
  汎用的に役立つので lib 側に残してある。

## [0.1.0-alpha.2] - 2026-05-06

### Changed
- crate と GitHub repo の名前統一: `furigana` (取得済) は別 crate に取られていたため
  `ja-furigana` (lib) / `ja-furigana-cli` (bin) に rename。
- GitHub repo を `RyuuNeko1107/furigana` → `RyuuNeko1107/ja-furigana` /
  `RyuuNeko1107/furigana-dict` → `RyuuNeko1107/ja-furigana-dict` に rename。
  GitHub redirect が効くため旧 URL も互換。
- `crates/furigana-cli/src/commands/dict_pull.rs` の REPO 定数を
  `RyuuNeko1107/ja-furigana-dict` に更新 (alpha.1 は redirect 経由)。

### Removed
- 旧 `furigana-cli@0.1.0-alpha.1` を yank (rename 前の crate name、利用者ほぼゼロ前提)。

## [0.1.0-alpha.1] - 2026-05-05

初回 crates.io publish。Phase 2 機能ほぼ完成版。

### Added (Phase 2)
- **`furigana repl`**: 対話モード (rustyline、Tab 補完、↑↓ 履歴、`:` optional)。
  引数なしで起動すれば REPL に入る (Windows ダブルクリック対応)。
- **`furigana dict pull`**: GitHub Releases から `ja-furigana-dict` の tarball を fetch、
  SHA-256 検証、`<data_dir>/data/` 配下に flat 展開。
- **ホットリロード**: `POST /admin/reload` (`[auth].admin_tokens` 認証) と Unix 上の
  `SIGHUP` で `<data_dir>` から辞書を再 build。
- **portable 配置**: 既定では `<exe>/data/` に展開。フォルダごとコピーで持ち運べる。
- **SI 単位の case-insensitive lookup**: `1km` / `1KM` / `1Km` どれも「いちきろめーとる」。
  個別 entry で `ci = false` opt-out 可能。
- **依存ライセンスの自動収集**: `cargo about` で `NOTICE.md` を生成。CI で license
  drift を検知 (GPL/AGPL の混入を防止)。
- **GitHub Releases 自動配布**: `release.yml` で 5 platform の binary +
  `ghcr.io/ryuuneko1107/furigana` Docker image を tag push で配布。

### Changed
- 配布物 layout を `<data_dir>/{core,rules}/` の 2 階層から `<data_dir>/data/`
  1 階層に統合。`Dict::from_toml_str` を defensive に修正し、rules 系
  inline-table TOML を silent skip するように。

## [Pre-history (Phase 1)] - ~2026-05-04

- workspace 構成 (`furigana` lib + `furigana-cli` bin) と Lindera + IPADIC ベースの
  形態素解析パイプライン。
- `Furigana` / `FuriganaBuilder` 公開 API、`tokens_to_ruby` / `tokens_to_hiragana`、
  TTS 整形 (`TtsOptions` + `normalize_for_tts`)。
- `furigana lookup` / `furigana serve` (Axum HTTP、本番 API 互換) /
  `furigana dict {add,list,remove,import}` サブコマンド。
- 数値テキスト全体オーケストレーション (`NumberChunker` で時刻・日付・URL・スケール・
  助数詞・SI 単位を 1 パイプラインで処理)。
- データ駆動ルール: 全ルールを `ja-furigana-dict` 側 TOML で外部化。
- 本番 ryuuneko.com から seed 投入 (unihan 43,749 / jukugo 605 / compat 436)。

## [一覧]

[Unreleased]: https://github.com/RyuuNeko1107/ja-furigana/compare/v0.1.0-alpha.8...HEAD
[0.1.0-alpha.8]: https://github.com/RyuuNeko1107/ja-furigana/releases/tag/v0.1.0-alpha.8
[0.1.0-alpha.7]: https://github.com/RyuuNeko1107/ja-furigana/releases/tag/v0.1.0-alpha.7
[0.1.0-alpha.6]: https://github.com/RyuuNeko1107/ja-furigana/releases/tag/v0.1.0-alpha.6
[0.1.0-alpha.5]: https://github.com/RyuuNeko1107/ja-furigana/releases/tag/v0.1.0-alpha.5
[0.1.0-alpha.4]: https://github.com/RyuuNeko1107/ja-furigana/releases/tag/v0.1.0-alpha.4
[0.1.0-alpha.3]: https://github.com/RyuuNeko1107/ja-furigana/releases/tag/v0.1.0-alpha.3
[0.1.0-alpha.2]: https://github.com/RyuuNeko1107/ja-furigana/releases/tag/v0.1.0-alpha.2
[0.1.0-alpha.1]: https://github.com/RyuuNeko1107/ja-furigana/releases/tag/v0.1.0-alpha.1
