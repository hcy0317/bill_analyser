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

COPY deploy/ocr/requirements.txt /tmp/bill-analyser-ocr-requirements.txt

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        curl \
        python3 \
        python3-venv \
        tesseract-ocr \
        tesseract-ocr-chi-sim \
    && python3 -m venv /opt/bill-analyser-ocr \
    && /opt/bill-analyser-ocr/bin/pip install --no-cache-dir -r /tmp/bill-analyser-ocr-requirements.txt \
    && /opt/bill-analyser-ocr/bin/pip download --no-deps --dest /tmp/bill-analyser-ocr-wheel rapidocr==3.9.2 \
    && echo "04d6b8d151f823d930bd91910555f57bea897c0c44fa6794267b94cf9c1ef9a0  /tmp/bill-analyser-ocr-wheel/rapidocr-3.9.2-py3-none-any.whl" | sha256sum --check --strict \
    && /opt/bill-analyser-ocr/bin/pip install --no-cache-dir --no-deps /tmp/bill-analyser-ocr-wheel/rapidocr-3.9.2-py3-none-any.whl \
    && /opt/bill-analyser-ocr/bin/rapidocr check \
    && rm -f /tmp/bill-analyser-ocr-requirements.txt \
    && rm -rf /tmp/bill-analyser-ocr-wheel \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home --home-dir /app bill-analyser \
    && install -d -o bill-analyser -g bill-analyser /app/data /app/backup

WORKDIR /app
COPY --from=backend-builder /build/target/release/bill_http_server /usr/local/bin/bill_http_server
COPY deploy/ocr/rapidocr_adapter.py /opt/bill-analyser-ocr/rapidocr_adapter.py
COPY deploy/ocr/NOTICE.md /usr/share/doc/bill-analyser-ocr/NOTICE.md

RUN chmod 0555 /opt/bill-analyser-ocr/rapidocr_adapter.py \
    && /opt/bill-analyser-ocr/bin/python /opt/bill-analyser-ocr/rapidocr_adapter.py --check

ENV BILL_ANALYSER_RUST_OCR_LOCAL_JSON_COMMAND=/opt/bill-analyser-ocr/bin/python \
    BILL_ANALYSER_RUST_OCR_LOCAL_JSON_COMMAND_ARGS="/opt/bill-analyser-ocr/rapidocr_adapter.py;{input};{mime}" \
    BILL_ANALYSER_RUST_OCR_LOCAL_JSON_INPUT_MODE=file \
    BILL_ANALYSER_RUST_OCR_LOCAL_JSON_TIMEOUT_MS=45000 \
    BILL_ANALYSER_RUST_OCR_LOCAL_JSON_BUNDLED=1 \
    BILL_ANALYSER_RUST_OCR_LOCAL_JSON_DISPLAY_NAME=RapidOCR \
    BILL_ANALYSER_RUST_OCR_LOCAL_JSON_MODEL="PP-OCRv6 small (ONNX Runtime)" \
    OMP_NUM_THREADS=2 \
    OPENBLAS_NUM_THREADS=2

USER bill-analyser
EXPOSE 5000
ENTRYPOINT ["/usr/local/bin/bill_http_server"]

FROM nginx:1.28-alpine AS frontend-runtime

COPY deploy/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=frontend-builder /build/src/web/dist /usr/share/nginx/html

EXPOSE 8080
