FROM rust:1-bookworm AS build
WORKDIR /app

RUN cargo install topcoat-cli --locked

COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY server ./server
COPY shared ./shared
COPY agent ./agent
COPY migrations ./migrations
RUN mkdir -p public styles && cargo build --release --bin senn && topcoat asset bundle --release

FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app

COPY --from=build /app/target/release/senn /app/senn
COPY --from=build /app/target/release/assets /app/assets
COPY --from=build /app/public /app/public
COPY styles ./styles
COPY public/favicon.svg ./public/favicon.svg
COPY public/logo.svg ./public/logo.svg
COPY public/fonts ./public/fonts

ENV HOST=0.0.0.0 \
    PORT=3000

EXPOSE 3000

CMD ["/app/senn"]
