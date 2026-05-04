# furigana

Japanese furigana (ruby) lookup library and HTTP server in Rust.

> ⚠️ **Status**: Pre-alpha — under active development. APIs and data formats *will* change.

## What is this

- **Library** (`furigana`): annotate Japanese text with kana readings. Pure Rust, no DB, no async.
- **CLI / Server** (`furigana-cli` → `furigana` binary): local HTTP API and dictionary management.

Designed for **local development and embedded use** — not as a public-facing service. Default bind is `127.0.0.1`, no auth, no rate limit.

## Why another one

Most existing tools either (a) hardcode all rules in source, making contribution impossible without Rust knowledge, or (b) require heavy dependencies (Postgres, Java, etc.) just to look up readings.

`furigana` keeps every rule — counters, scales, units, context-sensitive readings, exception phrases — as **editable TOML/TSV data files**. Want to add a reading? Edit a TSV. Want to fix a counter rule? Edit a TOML. No recompile.

## Quick start

### As a library

```rust
use furigana::Furigana;

let f = Furigana::minimal();
let ruby = f.to_ruby("灰桜の散る道");
// → "{灰桜|はいざくら}の{散|ち}る{道|みち}"
```

### As a local server

```sh
$ cargo install furigana-cli           # or grab a binary from Releases
$ furigana dict pull                   # download core dictionary
$ furigana serve                       # http://127.0.0.1:8000
$ curl 'http://127.0.0.1:8000/furigana?text=灰桜'
```

## Dictionary layout

```
~/.local/share/furigana/dict/   (Windows: %LOCALAPPDATA%\furigana\dict\)
├── core/         <- managed by `furigana dict pull` (read-only)
├── user/         <- drop your own *.tsv files here
└── overrides.tsv <- highest priority manual overrides
```

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Adding readings or fixing rules typically means editing a TSV/TOML file under `data/rules/` — no Rust required. See [CONTRIBUTING.md](CONTRIBUTING.md) (TBD).
