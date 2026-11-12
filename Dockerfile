# Web app (Expo export) + Rust server in one image. Build from the repo root.
FROM node:24-slim AS web
WORKDIR /app
COPY app/package.json app/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY app ./
ENV CI=1
RUN npx expo export --platform web --output-dir /web

FROM rust:1-slim AS server
WORKDIR /src
COPY server/Cargo.toml server/Cargo.lock ./
COPY server/src src
COPY server/migrations migrations
COPY server/deck deck
COPY server/assets assets
RUN cargo build --release --locked

FROM debian:trixie-slim
RUN useradd --system --uid 10001 app && mkdir -p /data && chown app /data
COPY --from=server /src/target/release/quidenous-server /usr/local/bin/server
COPY --from=web /web /web
USER app
ENV DATA_DIR=/data WEB_DIR=/web PORT=8080
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s CMD ["/usr/local/bin/server", "--health"]
CMD ["server"]
