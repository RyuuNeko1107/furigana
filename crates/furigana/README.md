# furigana

Japanese furigana (ruby) lookup library.

⚠️ Pre-alpha — public API will change.

```rust
use furigana::Furigana;

let f = Furigana::minimal();
let ruby = f.to_ruby("灰桜の散る道");
// → "{灰桜|はいざくら}の{散|ち}る{道|みち}"
```

See the [project README](https://github.com/RyuuNeko1107/ja-furigana) for full docs.

## License

MIT License.
