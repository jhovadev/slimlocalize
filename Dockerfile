FROM rust:1.99-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p slimlocalize-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl util-linux \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 --shell /usr/sbin/nologin slimlocalize
COPY --from=builder /app/target/release/slimlocalize-server /usr/local/bin/slimlocalize-server
COPY scripts/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod 0755 /usr/local/bin/slimlocalize-server /usr/local/bin/docker-entrypoint.sh
ENV SLIMLOCALIZE_BIND=0.0.0.0:8787
ENV SLIMLOCALIZE_ALLOW_PUBLIC_READONLY=true
ENV SLIMLOCALIZE_DB=/data/slimlocalize.sqlite3
ENV SLIMLOCALIZE_LOG_FORMAT=json
VOLUME ["/data"]
EXPOSE 8787
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8787/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
