FROM rust:1.73 as build
WORKDIR /app
COPY . .
RUN cargo build -p global-synchronizer --release

FROM debian:12-slim
WORKDIR /srv
COPY --from=build /app/target/release/global-synchronizer /usr/local/bin/global-synchronizer
COPY global-synchronizer/config/global-sync.toml /srv/global-sync.toml
ENV RUST_LOG=info
EXPOSE 8000 7000
RUN useradd -u 10001 -m appuser
USER 10001
CMD ["/bin/sh","-c","/usr/local/bin/global-synchronizer --node-id $NODE_ID --config /srv/global-sync.toml --port 8000 --consensus-port 7000 --database-url $DATABASE_URL"]