# Web app (Expo export) + Rust server in one image. Build from the repo root.
# Base images are pinned to a release (Dependabot proposes bumps); add @sha256 digests for byte-identical rebuilds.
# Toolchains match CI: Rust as in .github/workflows/server.yml, Node as in app.yml / android.yml. Bump them together
# (Dependabot only proposes patch bumps for these two).
FROM node:24.22-slim AS web
WORKDIR /app
COPY app/package.json app/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY app ./
ENV CI=1
RUN npx expo export --platform web --output-dir /web

FROM rust:1.99.1-slim AS server
WORKDIR /src
COPY server/Cargo.toml server/Cargo.lock server/build.rs ./
COPY server/src src
COPY server/migrations migrations
COPY server/deck deck
COPY server/assets assets
# build.rs bakes what /api/version reports into the binary: these build args (deploy/deploy.sh passes them), else
# the version in app.json (read from ../app/app.json, so /app here). No .git in the context: no commit without GIT_SHA.
COPY app/app.json /app/app.json
ARG VERSION=
ARG GIT_SHA=
ENV QUIDENOUS_VERSION=$VERSION QUIDENOUS_GIT_SHA=$GIT_SHA
RUN cargo build --release --locked

FROM debian:trixie-slim
RUN useradd --system --uid 10001 app && mkdir -p /data && chown app /data
COPY --from=server /src/target/release/quidenous-server /usr/local/bin/server
COPY --from=web /web /web
# The first community packs (docs/PACK_FILES.md, `server seed-packs /packs`).
COPY server/packs /packs
USER app
# Also in the image's environment: deploy.sh's rollback reads the previous image's commit from it.
ARG VERSION=
ARG GIT_SHA=
ENV DATA_DIR=/data WEB_DIR=/web PORT=8080 QUIDENOUS_VERSION=$VERSION QUIDENOUS_GIT_SHA=$GIT_SHA
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s CMD ["/usr/local/bin/server", "--health"]
CMD ["server"]
