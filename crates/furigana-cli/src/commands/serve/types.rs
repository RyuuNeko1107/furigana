//! `serve` の HTTP 型 + AppState + エラーヘルパ

use axum::http::StatusCode;
use axum::Json;
use furigana::Furigana;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

/// 1 リクエストあたりの最大入力文字数 (公開 API 仕様)
pub(super) const MAX_TEXT_LEN: usize = 10_000;

/// HTTP サーバーの共有状態
#[derive(Clone)]
pub(super) struct AppState {
    pub(super) furigana: Arc<Furigana>,
    pub(super) tokens: Arc<Vec<String>>,
}

/// `/furigana` のクエリ / body パラメータ
#[derive(Debug, Deserialize)]
pub(super) struct FuriganaParams {
    #[serde(default)]
    pub(super) text: Option<String>,
    #[serde(default)]
    pub(super) text_b64: Option<String>,
    #[serde(default = "default_mode")]
    pub(super) mode: String,
    #[serde(default = "default_short_pause")]
    pub(super) short_pause: String,
    #[serde(default = "default_long_pause")]
    pub(super) long_pause: String,
    #[serde(default = "default_true")]
    pub(super) keep_period: bool,
    #[serde(default)]
    pub(super) segmented: bool,
    #[serde(default = "default_max_seg")]
    pub(super) max_segment_len: usize,
    #[serde(default)]
    pub(super) debug: bool,
}

pub(super) fn default_mode() -> String {
    "tts".to_string()
}
pub(super) fn default_short_pause() -> String {
    " ".to_string()
}
pub(super) fn default_long_pause() -> String {
    "   ".to_string()
}
pub(super) fn default_true() -> bool {
    true
}
pub(super) fn default_max_seg() -> usize {
    60
}

/// `/furigana` の正常レスポンス
#[derive(Debug, Serialize)]
pub(super) struct FuriganaResponse {
    pub(super) result: String,
    pub(super) mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) segments: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) timings_ms: Option<Value>,
}

/// `/furigana` のエラーレスポンス (`{"error": "..."}` + HTTP ステータス)
pub(super) type ApiError = (StatusCode, Json<Value>);

/// エラーレスポンスを生成するショートカット
pub(super) fn error(code: StatusCode, msg: impl Into<String>) -> ApiError {
    (code, Json(json!({ "error": msg.into() })))
}
