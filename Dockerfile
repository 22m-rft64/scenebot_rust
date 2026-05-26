# syntax=docker/dockerfile:1.7

# === Build stage ===
FROM --platform=linux/amd64 rust:1.93 AS builder

RUN apt-get update && \
    apt-get install -y --no-install-recommends libssl-dev pkg-config && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY . .
RUN cargo build --release --bins

# === Runtime stage ===
FROM --platform=linux/amd64 debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

ENV OPENSSL_MODULES=/usr/lib/x86_64-linux-gnu/ossl-modules \
    SSL_CERT_DIR=/etc/ssl/certs

WORKDIR /app

COPY --from=builder /app/target/release/scene_rust        /app/scene_rust
COPY --from=builder /app/target/release/migrate_guild_ids /app/migrate_guild_ids

LABEL scenebot=true

CMD ["/app/scene_rust"]
