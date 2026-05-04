# syntax=docker/dockerfile:1.6
#
# Multi-stage build for furigana-cli
#
# Build stage: rust:1.88-slim (matches Cargo.toml MSRV)
# Runtime stage: distroless/cc-debian12 (small, no shell, libgcc あり)

FROM rust:1.88-slim AS builder

WORKDIR /build

# 依存だけ先に build してキャッシュを効かせる手もあるが、
# pre-alpha では単純に全 build。最適化は後で。
COPY . .

# release build (lto + codegen-units=1 で size 最適化済み、Cargo.toml 設定参照)
RUN cargo build --release --workspace

# ─── runtime ───────────────────────────────────────────────────────────────
FROM gcr.io/distroless/cc-debian12

LABEL org.opencontainers.image.source="https://github.com/RyuuNeko1107/furigana"
LABEL org.opencontainers.image.description="日本語フリガナ (ルビ) 解決 HTTP サーバー"
LABEL org.opencontainers.image.licenses="MIT"

# ビルド成果物の配置 (data/rules/* は include_str! で binary に embed 済)
COPY --from=builder /build/target/release/furigana /usr/local/bin/furigana

# 既定の bind: 0.0.0.0:8000 (コンテナ内なので外部公開前提)
EXPOSE 8000

# user dict / config を mount するなら /data 推奨 (FURIGANA_DATA_DIR で指定)
ENV FURIGANA_DATA_DIR=/data
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/furigana"]
CMD ["serve", "--bind", "0.0.0.0:8000"]
