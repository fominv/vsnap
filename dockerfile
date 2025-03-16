FROM rust:1.85.0-alpine3.21 AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apk add --no-cache musl-dev

WORKDIR /app

COPY ./rust rust

WORKDIR /app/rust

RUN cargo build --release --target=x86_64-unknown-linux-musl -p vsnap-runner 
 
FROM alpine:3.21.3

RUN apk add --no-cache zstd

WORKDIR /app

COPY --from=builder /app/rust/target/x86_64-unknown-linux-musl/release/vsnap-runner .

ENV RUST_BACKTRACE=1

ENTRYPOINT ["./vsnap-runner"]
 