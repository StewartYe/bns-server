FROM rust:1.92-slim AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY .sqlx ./.sqlx
COPY migrations ./migrations

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates tzdata curl && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/bns-server /bns-server
COPY server-ca.pem /usr/local/share/ca-certificates/valkey-ca.crt
RUN update-ca-certificates

EXPOSE 8080

CMD ["/bns-server"]
