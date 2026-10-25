# syntax=docker/dockerfile:1
# Image unique de Foodflow (ADR-0009) : le binaire sert l'API et l'interface.

FROM node:24-trixie-slim AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npx ng build

FROM rust:1.98-trixie AS server
WORKDIR /src
# Les requêtes sqlx sont vérifiées à partir du cache .sqlx/ (ADR-0017).
ENV SQLX_OFFLINE=true
COPY Cargo.toml Cargo.lock rustfmt.toml clippy.toml ./
COPY .sqlx .sqlx
COPY crates crates
RUN cargo build --release --locked -p foodflow-server

FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends tzdata ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /data foodflow \
    && mkdir /data && chown foodflow:foodflow /data
COPY --from=server /src/target/release/foodflow /usr/local/bin/foodflow
COPY --from=web /src/web/dist/foodflow/browser /app/web
ENV FOODFLOW_DATA_DIR=/data \
    FOODFLOW_ADDR=0.0.0.0:3100 \
    FOODFLOW_WEB_DIR=/app/web \
    TZ=Europe/Paris
USER foodflow
VOLUME /data
EXPOSE 3100
ENTRYPOINT ["foodflow"]
CMD ["serve"]
