# --- builder ---
FROM rust:1-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p flock

# --- runtime ---
FROM debian:bookworm-slim
# ffmpeg is only needed for the live SRT preview (crates/web/src/preview.rs) -
# everything else in flock works fine without it. IMPORTANT: this must be an
# ffmpeg build with SRT input support (`ffmpeg -protocols | grep srt`);
# Debian bookworm's own `ffmpeg` package has historically NOT linked libsrt
# (unconfirmed either way for the current bookworm build - verify before
# relying on it). Preview requests just get an "ffmpeg not found" style
# error if this doesn't pan out; nothing else in flock is affected.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl ffmpeg \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/flock /usr/local/bin/flock
COPY config/example.toml /app/config/flock.toml

EXPOSE 8080
VOLUME ["/app/data"]

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["flock", "config/flock.toml"]
