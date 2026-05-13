//! HTTP ハンドラ + 変換ロジック

use super::types::{
    default_mode, error, ApiError, AppState, FuriganaParams, FuriganaResponse, MAX_TEXT_LEN,
    SLOW_REQUEST_MS,
};
use axum::extract::{ConnectInfo, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use furigana::{Furigana, RomajiStyle, TtsOptions};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::time::Instant;

/// reload trigger 元 (= log + metrics 用)
///
/// `Startup` は予約 (= 起動時 reload はまだ do_reload 経由しない)、
/// `Sighup` は Unix のみ使われる。 cross-platform 互換のため全 variant を保持。
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(super) enum ReloadSource {
    Startup,
    Admin,
    AutoUpdate,
    Sighup,
}

impl ReloadSource {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Admin => "admin",
            Self::AutoUpdate => "auto_update",
            Self::Sighup => "sighup",
        }
    }
}

/// `GET /healthz`
pub(super) async fn healthz(State(state): State<AppState>) -> Json<Value> {
    let f = state.furigana.read().await;
    Json(json!({
        "status": "ok",
        "dict_size": f.dict_size(),
    }))
}

/// `GET /metrics` — Prometheus 互換 text exposition format で metrics を返す
pub(super) async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let body = state.metrics.render();
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        body,
    )
}

/// `GET /furigana?text=...`
pub(super) async fn furigana_get(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(params): Query<FuriganaParams>,
) -> Result<Json<FuriganaResponse>, ApiError> {
    let f = state.furigana.read().await.clone();
    let user_agent = ua(&headers);
    process(f.as_ref(), &params, &state, peer, user_agent.as_deref())
}

/// `POST /furigana` (JSON body)
pub(super) async fn furigana_post(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(params): Json<FuriganaParams>,
) -> Result<Json<FuriganaResponse>, ApiError> {
    let f = state.furigana.read().await.clone();
    let user_agent = ua(&headers);
    process(f.as_ref(), &params, &state, peer, user_agent.as_deref())
}

/// `POST /admin/reload` — `<data_dir>` から辞書を再ロードして state を swap
pub(super) async fn admin_reload(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let dict_size = do_reload(&state, ReloadSource::Admin).await.map_err(|e| {
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
pub(super) async fn do_reload(state: &AppState, source: ReloadSource) -> Result<usize, String> {
    let old_size = state.furigana.read().await.dict_size();
    let paths = state.paths.clone();
    let new = tokio::task::spawn_blocking(move || crate::commands::build_furigana(&paths))
        .await
        .map_err(|e| format!("reload task join error: {e}"))?
        .map_err(|e| format!("build_furigana failed: {e}"))?;
    let new_arc = std::sync::Arc::new(new);
    let dict_size = new_arc.dict_size();
    *state.furigana.write().await = new_arc;
    state.metrics.record_reload(dict_size);
    tracing::info!(
        source = source.as_str(),
        old_size,
        new_size = dict_size,
        delta = dict_size as i64 - old_size as i64,
        "dict reload"
    );
    Ok(dict_size)
}

/// HeaderMap から User-Agent 取り出し (= debug log 用、 無ければ None)
fn ua(headers: &HeaderMap) -> Option<String> {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// パラメータをデコード → モード別変換 → JSON レスポンス組み立て
fn process(
    f: &Furigana,
    params: &FuriganaParams,
    state: &AppState,
    peer: SocketAddr,
    user_agent: Option<&str>,
) -> Result<Json<FuriganaResponse>, ApiError> {
    let text = decode_text(params)?;
    validate_length(&text)?;
    let mode = normalize_mode(&params.mode);

    tracing::debug!(
        peer = %peer,
        user_agent = user_agent.unwrap_or("-"),
        text = %text,
        mode = %mode,
        "request"
    );

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
        state.metrics.record_request(&mode, t_total_ms);
        let degraded = detect_degraded(&mode, &text, &result);
        if degraded {
            state.metrics.record_failed_resolution();
            tracing::warn!(
                peer = %peer,
                text = %text,
                result = %result,
                mode = %mode,
                "reading resolution degraded (= empty or identity to input)"
            );
        }
        if t_total_ms > SLOW_REQUEST_MS {
            state.metrics.record_slow_request();
            tracing::warn!(
                peer = %peer,
                mode = %mode,
                text_len = text.chars().count(),
                total_ms = round1(t_total_ms),
                "slow request"
            );
        }
        tracing::debug!(
            peer = %peer,
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
    state.metrics.record_request(&mode, t_total_ms);
    let degraded = detect_degraded(&mode, &text, &result);
    if degraded {
        state.metrics.record_failed_resolution();
        tracing::warn!(
            peer = %peer,
            text = %text,
            result = %result,
            mode = %mode,
            "reading resolution degraded (= empty or identity to input)"
        );
    }
    if t_total_ms > SLOW_REQUEST_MS {
        state.metrics.record_slow_request();
        tracing::warn!(
            peer = %peer,
            mode = %mode,
            text_len = text.chars().count(),
            total_ms = round1(t_total_ms),
            tokenize_ms = round1(t_tokenize_ms),
            convert_ms = round1(t_convert_ms),
            "slow request"
        );
    }
    tracing::debug!(
        peer = %peer,
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

/// 読み解決が退化 (= empty / kanji 通過扱い / input = output) しているか判定。
///
/// `mode="kanji"` は input そのまま返すのが仕様なので除外。 そうでない mode で
/// result が空 / input と同一 / 全部 None-reading の状態は dict 未収録 or
/// engine の path 解決失敗を示唆する debug 用 signal。
fn detect_degraded(mode: &str, text: &str, result: &str) -> bool {
    if mode == "kanji" {
        return false;
    }
    if result.is_empty() {
        return true;
    }
    // input 中に漢字を含むのに result が input と完全一致 = reading 解決されてない
    let has_kanji = text.chars().any(|c| {
        // CJK Unified Ideographs の主要範囲のみで近似 (= 詳細は furigana::kana 側)
        ('\u{4E00}'..='\u{9FFF}').contains(&c) || ('\u{3400}'..='\u{4DBF}').contains(&c)
    });
    has_kanji && result == text
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
