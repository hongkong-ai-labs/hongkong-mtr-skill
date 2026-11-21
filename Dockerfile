# syntax=docker/dockerfile:1
FROM rust:1.89-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked --bins

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home mtr

COPY --from=builder /app/target/release/mtr-hk /usr/local/bin/mtr-hk
COPY --from=builder /app/target/release/mtr-hk-mcp /usr/local/bin/mtr-hk-mcp
COPY --from=builder /app/target/release/mtr-hk-mcp-http /usr/local/bin/mtr-hk-mcp-http

ENV MTR_HTTP_ADDR=0.0.0.0:8080
EXPOSE 8080

USER mtr
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/health || exit 1

# Default to remote HTTP/SSE MCP. For stdio, override the entrypoint:
#   docker run --rm -i --entrypoint mtr-hk-mcp mtr-hk-mcp
ENTRYPOINT ["mtr-hk-mcp-http"]
