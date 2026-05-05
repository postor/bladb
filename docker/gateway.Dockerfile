FROM rust:1.95-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/examples ./apps/examples

RUN cargo build -p bladb-gateway --release

FROM debian:bookworm-slim

WORKDIR /app

COPY --from=builder /workspace/target/release/bladb-gateway /usr/local/bin/bladb-gateway
COPY --from=builder /workspace/apps/examples /app/apps/examples
COPY bladb.yml /app/bladb.yml

EXPOSE 8787

CMD ["bladb-gateway", "serve", "0.0.0.0:8787", "/app/bladb.yml"]
