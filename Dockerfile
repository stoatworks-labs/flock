# --- builder ---
FROM rust:1-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p flock

# --- runtime ---
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/flock /usr/local/bin/flock
COPY config/example.toml /app/config/flock.toml

EXPOSE 8080
VOLUME ["/app/data"]

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["flock", "config/flock.toml"]
