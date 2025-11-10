FROM rust:1.73 as build
WORKDIR /app
COPY . .
RUN cargo build -p sync-domain --release

FROM debian:12-slim
WORKDIR /srv
COPY --from=build /app/target/release/sync-domain /usr/local/bin/sync-domain
COPY sync-domain/sync-domain.toml /srv/sync-domain.toml
ENV RUST_LOG=info
EXPOSE 8082
RUN useradd -u 10001 -m appuser
USER 10001
CMD ["/bin/sh","-c","/usr/local/bin/sync-domain --config /srv/sync-domain.toml --port 8082 --database-url $DATABASE_URL"]