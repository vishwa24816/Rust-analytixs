FROM rust:1.82-slim AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev sqlite3 && rm -rf /var/lib/apt/lists/* \
 && cargo build --release
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates sqlite3 && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/rust_analytix /usr/local/bin/rust_analytix
COPY migrations /app/migrations
COPY tracker/js /app/tracker/js
WORKDIR /app
ENV LISTEN_ADDR=0.0.0.0:8000 DATABASE_URL=sqlite:/data/app.db?mode=rwc TRACKER_DIR=/app/tracker/js
VOLUME /data
EXPOSE 8000
CMD ["rust_analytix"]
