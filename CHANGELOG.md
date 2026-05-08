# Changelog

このプロジェクト (`ja-furigana` lib + `ja-furigana-cli` bin) の変更履歴。
[Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) 形式に概ね従い、
バージョニングは [Semantic Versioning](https://semver.org/lang/ja/) を採用。

## [Unreleased]

## [0.1.0-alpha.9] - 2026-05-08

alpha.8 から積み上がっていた累積変更をまとめてリリース。 主な軸は
**security 全 8 軸の補強** + **sanitize layer 新設** + **`[meta] role` 駆動 loader
の rules + dict 統一** + **rules 3 sub-dir 階層化** + **inline test の append-only
CI 強制** + **dict TOML format の DSL 化 (triple-quoted string)** + **days.toml の
`[entries]` block 化** 等。

公開 API は backwards compat を維持 — 既存 dict release tar (alpha.5+) は新 lib で
何も触らずに `furigana dict pull` で動作する。 `[meta] role` 無しの旧 file は
path-based 推定で fallback、 `DaysData` の旧 flat 形式も custom Deserialize で受け
入れる。 dict 側 PR (ja-furigana-dict#9) も新形式に migration 済 — alpha.9 lib +
新 dict release で最新形式の恩恵 (role tag 駆動 / triple-quoted DSL / 等)。

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

### Changed (dict loader: role 駆動 dispatch)

- **`Dict::from_toml_dir` / `Loanwords::from_toml_dir` を role 駆動に refactor**:
  従来 file 名 / dir 名 hardcoded skip (`single_overrides.toml` skip /
  `loanwords/` subdir skip) で識別していたが、 各 dict file の `[meta] role`
  tag を見て dispatch する形に変更。
  - `Dict` に load: role ∈ `{"jukugo", "unihan", "works"}` または role 不明
    (backwards compat: 古い release で `[meta]` 無い file を救う)
  - `Dict` から skip: role ∈ `{"loanwords", "single_overrides", "compat"}` /
    rules 系 role (`"counters"` / `"context"` / 等)
  - `Loanwords` に load: role = `"loanwords"` のみ
- **新規 helper `crate::loader::resolve_role`**: `[meta] role` → path 推定
  (rules / dict 両方) → None の優先順位で role を解決。 同 helper を rules /
  dict 両 loader が共有する。
- **dir 構造の自由化**: 同じ dir に jukugo file と loanwords file が混在しても
  正しく分離 load できるようになった。 `core_dict_dir(p)` と `core_loanwords_dir(p)`
  に同じ path を渡しても重複 load しない。
- 公開 API (`Dict::from_toml_dir` / `Loanwords::from_toml_dir`) のシグネチャ
  変更なし、 既存の dict release tar (alpha.5+) は path-based fallback で
  そのまま動作する。
- 関連 test 5 件追加 (dict 3 / loanwords 2): role tag 駆動 + path-based
  back-compat の両経路を validate。

### Changed (context rule: triple-quoted string で string list を受ける)

- **`prev_ends` / `next_starts_any` / `next2_starts` field に triple-quoted
  string 形式を追加**: 従来の TOML array (`["a", "b", "c"]`) に加えて、
  triple-quoted string (`"""\na\nb\nc\n"""`) でも書けるようになる。 後者は
  newline split + trim + 空行 filter で `Vec<String>` に変換される。
- **目的**: contributor が array で各行末に `,` を付ける friction を削減。
  特に多行 array (10+ entry) で merge conflict 耐性も向上 (1 行 1 entry)。
- 旧形式 (TOML array) は引き続きサポート (`#[serde(untagged)]` enum で両受け)、
  既存 dict release tar との backwards compat は維持。
- 関連 test 4 件追加: triple-quoted / blank line filter / array back-compat
  / empty string の各経路を validate。

### Changed (days.toml 構造: `[entries]` block 推奨、 旧形式互換維持)

- **`DaysData` を `[entries]` block 形式に migration、 旧 flat 形式も引き続き
  サポート**: 従来 transparent HashMap (top-level に `"1" = "ツイタチ"` 直書き)
  だったため `[meta] role` block を併置できず、 role 駆動 loader の対象外
  だった。 alpha.9 から `[entries]` table 内に entries を移し、
  `[meta] role = "days"` + `description` を併置可能に。 これで days.toml も
  他 rule file と同じく role tag 駆動で識別できる。
- 推奨形式 (alpha.9+):
  ```toml
  [meta]
  role = "days"
  description = "1〜31 日の特殊読み (1→ツイタチ 等)"

  [entries]
  "1" = "ツイタチ"
  "2" = "フツカ"
  ```
- 旧形式 (flat top-level、 alpha.5 〜 alpha.8 互換) も引き続き受け入れる:
  ```toml
  "1" = "ツイタチ"
  "2" = "フツカ"
  ```
  → custom Deserialize impl で `[entries]` key を見つければ Table 配下、
  無ければ top-level table 直下を採用する。 既存 dict release tar
  (alpha.5+) で `furigana dict pull` した user は何もせずに alpha.9 に
  upgrade できる。
- `DaysData` struct: `pub struct DaysData(pub HashMap<String,String>)` →
  `pub struct DaysData { pub entries: HashMap<String,String> }`。 `get` /
  `len` / `is_empty` の API は据え置き、 内部実装のみ `self.0` →
  `self.entries`。 derive(Deserialize) → 手書き impl に変更 (両形式 dispatch
  のため)。

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

## [0.1.0-alpha.1] 〜 [0.1.0-alpha.5] - 2026-05-05〜2026-05-06 (要約)

初回 crates.io publish (alpha.1) から、 本番互換の読み解決優先順位整備 (alpha.3) /
依存 major bump 一気取り込み (alpha.4) / 辞書自動更新 admin_tokens 不要化 (alpha.5)
までを集約。 各 alpha tag の verbose な entry は
[GitHub Releases](https://github.com/RyuuNeko1107/ja-furigana/releases) を参照。

主な達成点:

- **Phase 2 機能完成** (alpha.1): `furigana repl` (対話モード) / `furigana dict pull`
  (GitHub Releases + SHA-256 検証 + tarball 展開) / ホットリロード
  (`POST /admin/reload` + Unix `SIGHUP`) / portable 配置 (`<exe>/data/` 1 階層集約) /
  SI 単位 case-insensitive lookup / `cargo about` ベースの NOTICE.md 自動生成 /
  GitHub Releases 経由の 5 platform binary + Docker image 配布
- **crate 名統一** (alpha.2): `ja-furigana` (lib) / `ja-furigana-cli` (bin) に rename、
  GitHub repo も `RyuuNeko1107/ja-furigana` / `ja-furigana-dict` に統一
- **本番互換 5 段階優先順位** (alpha.3): `resolve_reading` を
  `context rule → jukugo → Lindera → unihan` で再構成、 `Dict` を jukugo (≥2 字) /
  unihan (1 字) に内部分離。 `postprocess.toml` (Step 7 mode 別 regex 置換) 新設。
  `NumberChunker` に漢数字日付 + scale+unit 連結 + counter context (期間 vs 日付)
  対応。 検証ループ 75/75 (100%)。 CI に `audit` (cargo-audit) + `corpus` (回帰検証) job 追加
- **依存 major bump 取り込み** (alpha.4): `toml` 0.8 → 1.x / `directories` 5 → 6 /
  `criterion` 0.5 → 0.8 / `sha2` 0.10 → 0.11
- **辞書自動更新 admin_tokens 不要** (alpha.5): `furigana serve --auto-pull` フラグ +
  `[auto_update]` config (background polling、 `enabled` / `interval` / `pin`) 新設。
  `/admin/reload` HTTP は外部から同期 reload を打ちたい運用者向けに残置

その他:

- alpha.4 で **`Furigana` の Lindera 初期化を lazy 化** (`Furigana::minimal()`
  単体で 5.97 ms → 27.3 µs)、 `Furigana::merge_dict_toml` / `Furigana::preload` /
  ローマ字出力モード (ヘボン式 / 訓令式) 追加
- `crates/furigana-wasm/` (WebAssembly bindings) は alpha.4 で削除
  (`.wasm` が Lindera + IPADIC 込みで 57 MB と重く、 Web からは `furigana serve` で
  十分という判断)
- alpha.3 で `cargo test --release` harness の **51 GB alloc 暴走** を修正
  (`chunks::regex::build_alt_regex` 空 list 時の never-match pattern が release DFA
  を暴発させていた、 Lindera は無罪)

## [Pre-history (Phase 1)] - ~2026-05-04

- workspace 構成 (`furigana` lib + `furigana-cli` bin) と Lindera + IPADIC ベースの
  形態素解析パイプライン
- `Furigana` / `FuriganaBuilder` 公開 API、 `tokens_to_ruby` / `tokens_to_hiragana`、
  TTS 整形 (`TtsOptions` + `normalize_for_tts`)
- `furigana lookup` / `furigana serve` (Axum HTTP、 本番 API 互換) /
  `furigana dict {add,list,remove,import}` サブコマンド
- 数値テキスト全体オーケストレーション (`NumberChunker` で時刻・日付・URL・スケール・
  助数詞・SI 単位を 1 パイプラインで処理)
- データ駆動ルール: 全ルールを `ja-furigana-dict` 側 TOML で外部化
- 本番 ryuuneko.com から seed 投入 (unihan 43,749 / jukugo 605 / compat 436)

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
