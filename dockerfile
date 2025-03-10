FROM rust:1.85.0-alpine3.21 AS builder

WORKDIR /app

COPY ./rust .

RUN cargo build --release -p vsnap-runner 
 
FROM alpine:3.21.3

RUN apk add --no-cache zstd

WORKDIR /app

COPY --from=builder /app/target/release/vsnap-runner .

ENTRYPOINT ["./vsnap-runner"]
 