//! ja-furigana の WebAssembly バインディング
//!
//! ブラウザ / Node.js から `Furigana::minimal()` ベースの動的辞書を使ってフリガナを
//! 付けられるようにする。形態素解析は Lindera + IPADIC を embed するため
//! 出力 .wasm は 数十 MB 級になる (`wasm-pack build --release` 必須)。
//!
//! ## 使い方 (web target)
//!
//! ```sh
//! wasm-pack build crates/furigana-wasm --target web --release
//! ```
//!
//! ```html
//! <script type="module">
//!   import init, { WasmFurigana } from "./pkg/ja_furigana_wasm.js";
//!   await init();
//!   const f = new WasmFurigana();
//!   f.addReading("灰桜", "ハイザクラ");
//!   document.body.textContent = f.toRuby("灰桜の散る道");
//! </script>
//! ```

use furigana::Furigana;
use wasm_bindgen::prelude::*;

/// JS から呼べる Furigana ラッパ。
#[wasm_bindgen(js_name = WasmFurigana)]
pub struct WasmFurigana {
    inner: Furigana,
}

#[wasm_bindgen(js_class = WasmFurigana)]
impl WasmFurigana {
    /// 空 default で初期化 (Lindera + IPADIC のみ、辞書は別途 `addReading` で投入)。
    ///
    /// # Errors
    /// 形態素解析器の初期化に失敗した場合 (リソース不足等)。
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmFurigana, JsError> {
        console_error_panic_hook::set_once();
        Ok(WasmFurigana {
            inner: Furigana::minimal().map_err(|e| JsError::new(&e.to_string()))?,
        })
    }

    /// 1 件追加。surface (漢字含む文字列) → reading (カタカナ or ひらがな)。
    #[wasm_bindgen(js_name = addReading)]
    pub fn add_reading(&mut self, surface: &str, reading: &str) {
        self.inner.add_reading(surface, reading);
    }

    /// `{灰桜|はいざくら}の{散る|ちる}{道|みち}` 形式で出力。
    #[wasm_bindgen(js_name = toRuby)]
    pub fn to_ruby(&self, text: &str) -> String {
        self.inner.to_ruby(text)
    }

    /// 全部ひらがなで出力。`はいざくらのちるみち`
    #[wasm_bindgen(js_name = toHiragana)]
    pub fn to_hiragana(&self, text: &str) -> String {
        self.inner.to_hiragana(text)
    }

    /// 現在登録されている辞書エントリ数。
    #[wasm_bindgen(js_name = dictSize, getter)]
    pub fn dict_size(&self) -> usize {
        self.inner.dict_size()
    }
}
