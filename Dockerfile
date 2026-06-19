# ── Stage 1: Build ────────────────────────────────────────────────────
FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev pkgconf

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/

RUN cargo build --release

# ── Stage 2: Runtime ──────────────────────────────────────────────────
FROM alpine:3.20

RUN apk add --no-cache ca-certificates tzdata gcompat libstdc++ util-linux

WORKDIR /app

COPY --from=builder /app/target/release/gravitylancacheui /usr/local/bin/gravitylancacheui
COPY static/ /app/static/

# Create data directories
RUN mkdir -p /data/gravitylancacheui

ENV LANCACHE_LOGS_DIR=/data/logs \
    LANCACHE_CACHE_DIR=/data/cache \
    PREFILL_DIR=/data/gravitylancacheui \
    DB_PATH=/data/gravitylancacheui/db.sqlite \
    CONFIG_FILE=/data/gravitylancacheui/config.json \
    LISTEN_PORT=8080 \
    CACHE_SCAN_INTERVAL_SECS=300 \
    LOG_RETENTION_DAYS=90 \
    RUST_LOG=gravitylancacheui=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
  CMD wget -q --spider http://localhost:8080/api/v1/health || exit 1

ENTRYPOINT ["gravitylancacheui"]
