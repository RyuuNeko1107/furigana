# データ配置

辞書 / ルール TOML の置き場所 / 探索順 / 優先順位の解説。

> 戻る: [README](../README.md) / 関連: [RULES.md](./RULES.md) (各ファイルの中身) / [CONFIG.md](./CONFIG.md)

## 既定の場所 (portable 配置)

default では **実行ファイルと同じディレクトリ**:

```
<furigana.exe と同じフォルダ>/
├── furigana.exe                   # 本体 (Windows なら .exe / 他は単に furigana)
├── config.toml                    # 設定 (任意)
├── repl_history                   # REPL の入力履歴 (自動生成)
└── data/                          # `furigana dict pull` で展開、ユーザー追加もここに集約
    ├── unihan.toml                # 単漢字フォールバック (43k+ 字)
    ├── compat.toml                # 異体字マップ (髙→高 等)
    ├── jukugo/                    # 熟語 / 固有名詞 / 文化系 (24 ファイル、カテゴリ別分割)
    │   ├── general.toml / four_char.toml / personal_names.toml / place_names.toml
    │   ├── proper_nouns.toml / animals.toml / foods.toml / specialized.toml
    │   ├── body_parts.toml / weather.toml / colors.toml / arts.toml / abstracts.toml
    │   ├── vehicles.toml / clothes.toml / architecture.toml / literature.toml
    │   ├── science.toml / emotions.toml / idioms.toml / politics.toml
    │   └── religions.toml / music.toml / sports.toml
    ├── works/                     # 作品単位辞書 (0.1.0-alpha.6+、無制限階層)
    │   └── game/touhou.toml       #   例: 東方Project (公式読みのみ採録、出典コメント必須)
    ├── days.toml                  # 1〜31 日特殊読み
    ├── scales.toml                # 万 / 億 / 兆 / 京 / 垓 ...
    ├── units.toml                 # SI 単位 + 「円」「%」(N+漢字単位 連結用、0.1.2)
    ├── symbols.toml               # 記号読み (〜→から、・→ナカグロ 等)
    ├── latin.toml                 # ラテン文字読み (A→エー …)
    ├── numeric_phrases.toml       # 慣用語句 (二十歳→ハタチ 等) + 百個 / 千個 等
    ├── counters/*.toml            # 助数詞ルール (年度 / 時間半 含む 7+ ファイル)
    ├── context/*.toml             # 文脈ルール (3 ファイル: numbers / homonyms / special)
    ├── postprocess.toml           # 後処理 regex 置換 (Step 7、0.1.2 新設)
    ├── user/                      # ユーザー追加 (`furigana dict add` で自動生成)
    │   └── cli-added.toml         #   `furigana dict add` 経由のエントリ
    └── overrides.toml              # 強制上書き用 (最優先、任意)
```

zip / tar.gz を解凍したフォルダごとコピーすれば持ち運べる **portable 配置**。

> ⚠️ 旧バージョン (alpha.1 以前) には `data/core/` と `data/rules/` の 2 階層分けがあったが、alpha.2 で **`data/` 1 階層に統合** された。lib loader が「`[entries]` 持つ TOML だけ拾う dict scan」「特定ファイル名だけ拾う rules scan」を排他的に実行するため、同じ `data/` を両方に渡しても干渉しない。

## カスタム場所 + 環境変数

CLI flag および環境変数で `<data_dir>` を変えられる:

```sh
# CLI flag
furigana lookup '灰桜' --data-dir /var/lib/furigana

# 環境変数 (優先度: CLI flag > env)
export FURIGANA_DATA_DIR=/var/lib/furigana
furigana lookup '灰桜'
```

`current_exe()` の解決に失敗した稀なケースのみ XDG fallback:
- Linux: `~/.local/share/furigana/`
- Windows: `%LOCALAPPDATA%\furigana\furigana\`
- macOS: `~/Library/Application Support/com.furigana.furigana/`

## 優先順位 (、0.1.0-alpha.3 で整備)

辞書ソースの **merge 順** (後勝ち、`Furigana::builder` で組立):

1. **`core_dict_dir`** ← 配布版 (`furigana dict pull`)、最弱
2. **`user_dict_dir`** ← `furigana dict add` の保存先 (`cli-added.toml`)
3. **`overrides_file`** ← `data/overrides.toml`
4. **`add_entry`** ← API で直接追加 (最強)

その上で **token 単位での読み解決優先順位** は (`reading::pipeline::resolve_reading`):

1. 漢字なし → `None`
2. **context rule** (`data/context/*.toml`) — 同形異音語 (一日 / 上手 / 市場) の動的読み分け
3. **熟語辞書 jukugo** (`Dict::lookup_jukugo`、surface ≥ 2 文字) — 灰桜=ハイザクラ等
4. **Lindera reading** — IPADIC `details[7]` のカタカナ (動詞活用形などの自然な読み)
5. **単漢字 unihan** (`Dict::lookup_unihan`、surface = 1 文字) — 最終 fallback
6. fallback `None`

context rule が **辞書より先** に評価されるため、`一日` を `general.toml` に登録していても、context rule の `prev_ends_with_month` で「6月一日」が「ロクガツツイタチ」になる。逆に `一日` を登録しないと Lindera が「一」+「日」に分解した結果がそのまま使われるため、登録は依然必要。

詳しい設計は [`docs/ARCHITECTURE.md`](./ARCHITECTURE.md#step-5-の詳細-resolve_reading-の-5-段階優先順位) を参照。

`furigana dict add 灰桜 ハイザクラ` で `data/user/cli-added.toml` に追加 → 次回起動時 (or `:reload`) で反映。

## `furigana dict pull` の動き

GitHub Releases から `ja-furigana-dict` の tarball を取得 + SHA-256 検証 + 展開する。

```sh
furigana dict pull                       # 最新 release
furigana dict pull --version v0.1.3      # version pin
```

サーバ運用 (alpha.5+) では、これを **起動時に自動実行** する `--auto-pull` フラグや、
**起動中に定期 polling** する `[auto_update]` config もある (admin_tokens 不要)。
詳細は [HTTP_API.md#ホットリロード--自動更新](./HTTP_API.md#ホットリロード--自動更新) を参照。

詳細フロー:

1. `--version` 指定 or GitHub API `/releases/latest` で tag 解決
2. `furigana-dict-{tag}.tar.gz` をダウンロード
3. `furigana-dict-{tag}.tar.gz.sha256` (sidecar) を取得して **SHA-256 検証** (sidecar 無い古い release は warn skip)
4. `<data_dir>/data/` 配下の旧配布ファイルを削除 (**`user/` と `overrides.toml` は保持**)
5. tarball 展開 + path rebase: archive 内の `core/X` / `rules/X` を `data/X` に flat 化

archive 側 (`ja-furigana-dict` repo) は `core/` `rules/` の 2 階層で PR レビュー上の分類のために維持されている。配布物だけ flat に変換される (展開後は `data/` 1 階層、ただし jukugo / works / counters / context のサブディレクトリは保持)。

```
archive 内 (PR レビュー用)        →   利用者の <data_dir>/data/ (flat-ish)
core/jukugo/general.toml         →   data/jukugo/general.toml
core/works/game/touhou.toml      →   data/works/game/touhou.toml
core/unihan.toml                 →   data/unihan.toml
rules/days.toml                  →   data/days.toml
rules/counters/objects.toml      →   data/counters/objects.toml
rules/context/numbers.toml       →   data/context/numbers.toml
```

> 0.1.0-alpha.6 以降の lib loader (`Dict::from_toml_dir`) は **無制限階層を再帰** で走査する。`data/jukugo/` (1 階層) と `data/works/<medium>/<title>.toml` (任意深度) が同じ loader で処理され、後者は作品単位 1 ファイルの細分化を許容する (公式読みのみ採録、出典コメント必須のサブポリシーは [`ja-furigana-dict/core/works/README.md`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/works/README.md) を参照)。

path traversal 防御として、archive entry の展開先が `<data_dir>/data/` 配下に収まることを canonicalize で確認している。

## REPL からの操作

```
all> :pull              # 同名コマンド (`furigana dict pull` 相当)
all> :pull v0.1.3       # version pin
all> :reload            # data_dir から in-memory 辞書を再 build
all> :size              # dict_size 表示
```

詳細は `furigana repl` 起動後 `:help` で。

## 関連リンク

- 各ファイルの中身 / フォーマット: [RULES.md](./RULES.md)
- 設定ファイル全項目: [CONFIG.md](./CONFIG.md)
- 辞書 PR の出し方: [`ja-furigana-dict/CONTRIBUTING.md`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/CONTRIBUTING.md)
