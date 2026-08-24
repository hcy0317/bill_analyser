# syntax=docker/dockerfile:1.7

FROM rust:1.89-bookworm AS backend-builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/backend ./src/backend
COPY tests/backend ./tests/backend
RUN cargo build --locked --release -p bill-analyser-http --bin bill_http_server

FROM node:24-alpine AS frontend-builder

WORKDIR /build/src/web
COPY src/web/package.json src/web/package-lock.json ./
RUN npm ci
COPY src/web ./
RUN npm run build

FROM debian:bookworm-slim AS backend-runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        curl \
        tesseract-ocr \
        tesseract-ocr-chi-sim \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home --home-dir /app bill-analyser \
    && install -d -o bill-analyser -g bill-analyser /app/data /app/backup

WORKDIR /app
COPY --from=backend-builder /build/target/release/bill_http_server /usr/local/bin/bill_http_server

USER bill-analyser
EXPOSE 5000
ENTRYPOINT ["/usr/local/bin/bill_http_server"]

FROM nginx:1.28-alpine AS frontend-runtime

COPY deploy/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=frontend-builder /build/src/web/dist /usr/share/nginx/html

EXPOSE 8080
