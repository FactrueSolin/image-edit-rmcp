FROM rust:1.88-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY templates ./templates

RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/image-edit-rmcp /app/image-edit-rmcp
COPY templates ./templates

ENV RUST_LOG=info

CMD ["/app/image-edit-rmcp"]
