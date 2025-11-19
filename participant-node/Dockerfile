FROM rust:1.73 as build
WORKDIR /app
COPY . .

# Install common native build dependencies to avoid compile failures on VPS
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config build-essential libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Harden cargo build: retry network, limit jobs, and lock dependency resolution
ENV CARGO_NET_RETRY=5
ENV CARGO_BUILD_JOBS=2
# Build only the participant-node crate to avoid pulling other workspace members
RUN cargo build --manifest-path participant-node/Cargo.toml --release -j $CARGO_BUILD_JOBS

FROM debian:12-slim
WORKDIR /srv
COPY --from=build /app/target/release/garp-participant-node /usr/local/bin/participant
COPY participant-node/config.toml /srv/config.toml
ENV RUST_LOG=info
EXPOSE 8090

# Ensure TLS roots exist in runtime image and create non-root user
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -u 10001 -m appuser
USER 10001
CMD ["/bin/sh","-c","/usr/local/bin/participant --config /srv/config.toml --api-port 8090 --database-url $DATABASE_URL"]