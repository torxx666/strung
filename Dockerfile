FROM rust:latest AS builder

WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    nano \
    vim-tiny \
    && rm -rf /var/lib/apt/lists/*

# Create a temporary directory for drop files
RUN mkdir -p /tmp && chmod 1777 /tmp

COPY --from=builder /app/target/release/strung /usr/local/bin/strung

ENTRYPOINT ["strung"]
