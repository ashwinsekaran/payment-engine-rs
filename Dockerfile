# syntax=docker/dockerfile:1

FROM rust:1.85-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app

COPY --from=builder /app/target/release/payment-engine-rs /usr/local/bin/payment-engine-rs

ENTRYPOINT ["/usr/local/bin/payment-engine-rs"]
