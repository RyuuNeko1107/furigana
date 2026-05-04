//! HTTP ハンドラ + 変換ロジック

use super::types::{
    default_mode, error, ApiError, AppState, FuriganaParams, FuriganaResponse, MAX_TEXT_LEN,
};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use furigana::{Furigana, TtsOptions};
use serde_json::{json, Value};
use std::time::Instant;

/// `GET /healthz`
pub(super) async fn healthz(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "dict_size": state.furigana.dict_size(),
    }))
}

/// `GET /furigana?text=...`
pub(super) async fn furigana_get(
    State(state): State<AppState>,
    Query(params): Query<FuriganaParams>,
) -> Result<Json<FuriganaResponse>, ApiError> {
    process(&state.furigana, &params)
}

/// `POST /furigana` (JSON body)
pub(super) async fn furigana_post(
    State(state): State<AppState>,
    Json(params): Json<FuriganaParams>,
) -> Result<Json<FuriganaResponse>, ApiError> {
    process(&state.furigana, &params)
}

/// パラメータをデコード → モード別変換 → JSON レスポンス組み立て
fn process(f: &Furigana, params: &FuriganaParams) -> Result<Json<FuriganaResponse>, ApiError> {
    let text = decode_text(params)?;
    validate_length(&text)?;
    let mode = normalize_mode(&params.mode);

    let t_start = Instant::now();
    let tokens_start = Instant::now();
    let tokens = f.tokenize(&text);
    let t_tokenize_ms = tokens_start.elapsed().as_secs_f64() * 1000.0;

    let convert_start = Instant::now();
    let result = match mode.as_str() {
        "kanji" => text.clone(),
        "ruby" => furigana::tokens_to_ruby(&tokens),
        "hiragana" => furigana::tokens_to_hiragana(&tokens),
        _ => {
            // tts (default)
            let opts = TtsOptions {
                short_pause: params.short_pause.clone(),
                long_pause: params.long_pause.clone(),
                keep_period: params.keep_period,
            };
            let hira = furigana::tokens_to_hiragana(&tokens);
            furigana::tts::normalize_for_tts(&hira, &opts)
        }
    };
    let t_convert_ms = convert_start.elapsed().as_secs_f64() * 1000.0;
    let t_total_ms = t_start.elapsed().as_secs_f64() * 1000.0;

    let segments = if params.segmented && (mode == "tts" || mode == "hiragana") {
        Some(furigana::tts::segment_for_tts(
            &result,
            params.max_segment_len,
        ))
    } else {
        None
    };

    let timings_ms = if params.debug {
        Some(json!({
            "total": round1(t_total_ms),
            "tokenize": round1(t_tokenize_ms),
            "convert": round1(t_convert_ms),
        }))
    } else {
        None
    };

    Ok(Json(FuriganaResponse {
        result,
        mode,
        segments,
        timings_ms,
    }))
}

/// `text` または `text_b64` から本文を取り出す。両方無ければ 400。
fn decode_text(params: &FuriganaParams) -> Result<String, ApiError> {
    if let Some(b64) = params.text_b64.as_ref() {
        let decoded = URL_SAFE_NO_PAD
            .decode(b64.trim_end_matches('='))
            .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid base64 in text_b64"))?;
        String::from_utf8(decoded).map_err(|_| {
            error(
                StatusCode::BAD_REQUEST,
                "text_b64 decoded bytes are not valid UTF-8",
            )
        })
    } else if let Some(t) = params.text.as_ref() {
        Ok(t.clone())
    } else {
        Err(error(StatusCode::BAD_REQUEST, "no text provided"))
    }
}

/// 入力長制限を確認
fn validate_length(text: &str) -> Result<(), ApiError> {
    if text.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "no text provided"));
    }
    let nchars = text.chars().count();
    if nchars > MAX_TEXT_LEN {
        return Err(error(
            StatusCode::BAD_REQUEST,
            format!("text too long: {nchars} chars (max {MAX_TEXT_LEN})"),
        ));
    }
    Ok(())
}

/// 不正な mode は静かに `tts` (= default) に fallback (本番 API と同挙動)
fn normalize_mode(mode: &str) -> String {
    match mode {
        "tts" | "hiragana" | "ruby" | "kanji" => mode.to_string(),
        _ => default_mode(),
    }
}

fn round1(ms: f64) -> f64 {
    (ms * 10.0).round() / 10.0
}
