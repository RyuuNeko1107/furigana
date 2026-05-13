//! HTTP ハンドラ + 変換ロジック

use super::types::{
    default_mode, error, ApiError, AppState, FuriganaParams, FuriganaResponse, MAX_TEXT_LEN,
};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use furigana::{Furigana, RomajiStyle, TtsOptions};
use serde_json::{json, Value};
use std::time::Instant;

/// `GET /healthz`
pub(super) async fn healthz(State(state): State<AppState>) -> Json<Value> {
    let f = state.furigana.read().await;
    Json(json!({
        "status": "ok",
        "dict_size": f.dict_size(),
    }))
}

/// `GET /furigana?text=...`
pub(super) async fn furigana_get(
    State(state): State<AppState>,
    Query(params): Query<FuriganaParams>,
) -> Result<Json<FuriganaResponse>, ApiError> {
    let f = state.furigana.read().await.clone();
    process(f.as_ref(), &params)
}

/// `POST /furigana` (JSON body)
pub(super) async fn furigana_post(
    State(state): State<AppState>,
    Json(params): Json<FuriganaParams>,
) -> Result<Json<FuriganaResponse>, ApiError> {
    let f = state.furigana.read().await.clone();
    process(f.as_ref(), &params)
}

/// `POST /admin/reload` — `<data_dir>` から辞書を再ロードして state を swap
pub(super) async fn admin_reload(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let dict_size = do_reload(&state).await.map_err(|e| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("reload failed: {e}"),
        )
    })?;
    Ok(Json(json!({
        "status": "reloaded",
        "dict_size": dict_size,
    })))
}

/// 辞書を再ビルド → state.furigana を差し替え。`POST /admin/reload` と SIGHUP の共通実装。
///
/// build 自体は CPU bound + I/O 込みなので `spawn_blocking` で逃がす。
/// 戻り値は新 dict のサイズ。
pub(super) async fn do_reload(state: &AppState) -> Result<usize, String> {
    let paths = state.paths.clone();
    let new = tokio::task::spawn_blocking(move || crate::commands::build_furigana(&paths))
        .await
        .map_err(|e| format!("reload task join error: {e}"))?
        .map_err(|e| format!("build_furigana failed: {e}"))?;
    let new_arc = std::sync::Arc::new(new);
    let dict_size = new_arc.dict_size();
    *state.furigana.write().await = new_arc;
    tracing::info!("辞書を reload しました (dict_size={dict_size})");
    Ok(dict_size)
}

/// パラメータをデコード → モード別変換 → JSON レスポンス組み立て
fn process(f: &Furigana, params: &FuriganaParams) -> Result<Json<FuriganaResponse>, ApiError> {
    let text = decode_text(params)?;
    validate_length(&text)?;
    let mode = normalize_mode(&params.mode);

    tracing::debug!(text = %text, mode = %mode, "request");

    let t_start = Instant::now();

    // analyze mode は tokenize 経路ではなく Smart engine analyze() を直接呼ぶ。
    // 既存 mode (tts/ruby/...) は従来通り tokenize → 変換。
    if mode == "analyze" {
        let analyze_start = Instant::now();
        let analyze_result = f.analyze(&text);
        let t_convert_ms = analyze_start.elapsed().as_secs_f64() * 1000.0;
        let t_total_ms = t_start.elapsed().as_secs_f64() * 1000.0;

        // result には採択 path の reading を連結 (= Smart engine が決めた reading sequence)、
        // 詳細 candidate / boundary は analyze field 経由で参照。
        let result: String = analyze_result
            .tokens
            .iter()
            .map(|t| t.reading.as_str())
            .collect();

        let timings_ms = if params.debug {
            Some(json!({
                "total": round1(t_total_ms),
                "tokenize": 0.0, // analyze は tokenize 経由しないため 0
                "convert": round1(t_convert_ms),
            }))
        } else {
            None
        };

        let token_dump = format_analyze_tokens(&analyze_result);
        tracing::debug!(
            result = %result,
            tokens = %token_dump,
            n_tokens = analyze_result.tokens.len(),
            total_ms = round1(t_total_ms),
            convert_ms = round1(t_convert_ms),
            "response (analyze)"
        );

        return Ok(Json(FuriganaResponse {
            result,
            mode,
            segments: None,
            timings_ms,
            analyze: Some(analyze_result),
        }));
    }

    let tokens_start = Instant::now();
    let tokens = f.tokenize(&text);
    let t_tokenize_ms = tokens_start.elapsed().as_secs_f64() * 1000.0;

    let convert_start = Instant::now();
    let result = match mode.as_str() {
        "kanji" => text.clone(),
        "ruby" => furigana::tokens_to_ruby(&tokens),
        "hiragana" => furigana::tokens_to_hiragana(&tokens),
        "romaji" => {
            let hira = furigana::tokens_to_hiragana(&tokens);
            furigana::hiragana_to_romaji(&hira, RomajiStyle::Hepburn)
        }
        "romaji-kunrei" => {
            let hira = furigana::tokens_to_hiragana(&tokens);
            furigana::hiragana_to_romaji(&hira, RomajiStyle::Kunrei)
        }
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

    let token_dump = format_tokens(&tokens);
    let n_segments = segments.as_ref().map(|s| s.len()).unwrap_or(0);
    tracing::debug!(
        result = %result,
        tokens = %token_dump,
        n_tokens = tokens.len(),
        n_segments,
        total_ms = round1(t_total_ms),
        tokenize_ms = round1(t_tokenize_ms),
        convert_ms = round1(t_convert_ms),
        "response"
    );

    Ok(Json(FuriganaResponse {
        result,
        mode,
        segments,
        timings_ms,
        analyze: None,
    }))
}

/// debug log 用に tokens を `surface[reading]|surface[reading]|...` 形式に整形
fn format_tokens(tokens: &[furigana::ReadingToken]) -> String {
    tokens
        .iter()
        .map(|t| {
            format!(
                "{}[{}]",
                t.surface,
                t.reading.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// debug log 用に analyze tokens を `surface[reading]|...` 形式に整形
fn format_analyze_tokens(result: &furigana::AnalyzeResult) -> String {
    result
        .tokens
        .iter()
        .map(|t| format!("{}[{}]", t.surface, t.reading))
        .collect::<Vec<_>>()
        .join("|")
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

/// 不正な mode は静かに `tts` (= default) に fallback
fn normalize_mode(mode: &str) -> String {
    match mode {
        "tts" | "hiragana" | "ruby" | "kanji" | "romaji" | "romaji-kunrei" | "analyze" => {
            mode.to_string()
        }
        _ => default_mode(),
    }
}

fn round1(ms: f64) -> f64 {
    (ms * 10.0).round() / 10.0
}
