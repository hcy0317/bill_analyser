import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

async function read(relativePath) {
  return readFile(resolve(root, relativePath), "utf8");
}

test("the production compose owns the complete Bill Analyser runtime", async () => {
  const [compose, dockerfile, nginx, prepare, verify, gitignore] = await Promise.all([
    read("compose.yml"),
    read("Dockerfile"),
    read("deploy/nginx.conf"),
    read("scripts/prepare_docker_runtime.ps1"),
    read("scripts/verify_docker_deployment.ps1"),
    read(".gitignore"),
  ]);

  for (const service of ["postgres", "weaviate", "backend", "frontend"]) {
    assert.match(compose, new RegExp(`^  ${service}:$`, "m"));
  }

  assert.match(compose, /^name: bill_analyser$/m);
  assert.match(compose, /target: backend-runtime/);
  assert.match(compose, /target: frontend-runtime/);
  assert.match(compose, /\.runtime\/docker\/backend\.env/);
  assert.match(compose, /\.runtime\/docker\/postgres\.env/);
  const postgresService = compose.match(/^  postgres:\r?\n[\s\S]*?(?=^  weaviate:)/m)?.[0] || "";
  const weaviateService = compose.match(/^  weaviate:\r?\n[\s\S]*?(?=^  backend:)/m)?.[0] || "";
  const backendService = compose.match(/^  backend:\r?\n[\s\S]*?(?=^  frontend:)/m)?.[0] || "";
  assert.match(postgresService, /\.runtime\/docker\/postgres\.env/);
  assert.doesNotMatch(postgresService, /backend\.env/);
  assert.doesNotMatch(postgresService, /^    ports:/m);
  assert.doesNotMatch(weaviateService, /^    ports:/m);
  assert.match(backendService, /\.runtime\/docker\/backend\.env/);
  assert.match(backendService, /dns:\r?\n\s+- 100\.100\.100\.100/);
  assert.doesNotMatch(backendService, /postgres\.env/);
  assert.match(compose, /condition: service_healthy/g);
  assert.match(compose, /BILL_ANALYSER_HTTP_BIND: 0\.0\.0\.0:5000/);
  assert.doesNotMatch(compose, /BILL_ANALYSER_POSTGRES_URL:/);
  assert.match(compose, /127\.0\.0\.1:\$\{BILL_ANALYSER_WEB_PORT:-18082\}:8080/);
  assert.match(compose, /\.\/data:\/app\/data/);
  assert.match(compose, /\.\/backup:\/app\/backup/);
  assert.doesNotMatch(compose, /(?:^|\s)-\s*["']?\$\{?BILL_ANALYSER_BACKEND_PORT/);

  assert.match(dockerfile, /^FROM rust:1\.89-bookworm AS backend-builder$/m);
  assert.match(dockerfile, /^FROM node:24-alpine AS frontend-builder$/m);
  assert.match(dockerfile, /^FROM debian:bookworm-slim AS backend-runtime$/m);
  assert.match(dockerfile, /^FROM nginx:1\.28-alpine AS frontend-runtime$/m);
  assert.match(dockerfile, /cargo build --locked --release -p bill-analyser-http --bin bill_http_server/);
  assert.match(dockerfile, /COPY tests\/backend \.\/tests\/backend/);
  assert.match(dockerfile, /tesseract-ocr-chi-sim/);
  assert.match(dockerfile, /rapidocr==3\.9\.2/);
  assert.match(dockerfile, /PP-OCRv6 small \(ONNX Runtime\)/);

  assert.match(nginx, /listen 8080/);
  assert.match(nginx, /proxy_pass http:\/\/backend:5000/);
  assert.match(nginx, /location = \/healthz/);
  assert.match(nginx, /location = \/desktop/);
  assert.match(nginx, /location = \/mobile/);

  assert.match(prepare, /control-credential\.json|server\.json|JWT_SECRET_KEY/);
  assert.match(prepare, /bill-analyser-postgres/);
  assert.match(prepare, /BILL_ANALYSER_POSTGRES_URL/);
  assert.match(prepare, /postgres:5432/);
  assert.match(prepare, /RandomNumberGenerator/);
  assert.doesNotMatch(prepare, /bill_analyser_dev/);
  assert.match(prepare, /backend\.env/);
  assert.match(prepare, /postgres\.env/);
  assert.match(prepare, /BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST/);
  assert.match(prepare, /https:\/\/sub2api\.long-antares\.ts\.net/);
  assert.match(prepare, /SetAccessRuleProtection/);
  assert.match(prepare, /DirectorySecurity/);
  assert.doesNotMatch(prepare, /Write-(?:Host|Output).*secret/i);

  assert.match(verify, /docker compose/);
  assert.match(verify, /api\/health\/ready/);
  assert.match(verify, /hcy-bill\.long-antares\.ts\.net/);
  assert.match(verify, /LocalOps/);
  assert.match(verify, /svc:hcy-bill/);
  assert.match(verify, /BILL_ANALYSER_AUTH_JWT_SECRET/);
  assert.match(verify, /POSTGRES_PASSWORD/);
  assert.match(verify, /rapidocr_adapter\.py --check/);
  assert.match(verify, /bundled_ocr/);
  assert.match(verify, /BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST/);
  assert.match(verify, /sub2api\.long-antares\.ts\.net/);
  assert.match(verify, /getent hosts/);

  assert.match(gitignore, /^\.runtime\/$/m);
});
