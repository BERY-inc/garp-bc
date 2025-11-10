FROM rust:1.73 as build
WORKDIR /app
COPY . .
RUN cargo build -p garp-participant-node --release

FROM debian:12-slim
WORKDIR /srv
COPY --from=build /app/target/release/garp-participant-node /usr/local/bin/participant
COPY participant-node/config.toml /srv/config.toml
ENV RUST_LOG=info
EXPOSE 8090
RUN useradd -u 10001 -m appuser
USER 10001
CMD ["/bin/sh","-c","/usr/local/bin/participant --config /srv/config.toml --api-port 8090 --database-url $DATABASE_URL"]