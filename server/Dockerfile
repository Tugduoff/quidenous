FROM rust:1-slim AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY migrations migrations
COPY deck deck
RUN cargo build --release --locked

FROM debian:trixie-slim
RUN useradd --system --uid 10001 app && mkdir -p /data && chown app /data
COPY --from=build /src/target/release/quidenous-server /usr/local/bin/server
USER app
ENV DATA_DIR=/data PORT=8080
EXPOSE 8080
CMD ["server"]
