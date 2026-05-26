# Weaviate derived index

Weaviate is optional derived infrastructure for import learning recall. PostgreSQL remains authoritative for learning samples, features, lifecycle state, feedback counters, suppressions, audit, and outbox state. Deleting or rebuilding Weaviate must not change business data.

## Local compose

```powershell
docker compose -f docker-compose.postgres.yml up -d postgres weaviate
```

The compose service uses `cr.weaviate.io/semitechnologies/weaviate:1.37.4` by default and runs without vectorizer modules because Bill Analyser sends self-provided vectors.

## Runtime config

Default config keeps Weaviate disabled:

```powershell
$env:BILL_ANALYSER_WEAVIATE_ENABLED = "false"
```

Enable local Weaviate:

```powershell
$env:BILL_ANALYSER_WEAVIATE_ENABLED = "true"
$env:BILL_ANALYSER_WEAVIATE_ENDPOINT = "http://127.0.0.1:8088"
$env:BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX = "BillAnalyser"
```

`BILL_ANALYSER_WEAVIATE_API_KEY` is optional for local anonymous compose and is only reported as configured/unconfigured in health output.

## Operations

Health:

```powershell
cargo run -p bill-analyser-http --bin bill_weaviate_derived_index -- --mode health
```

Bootstrap collections:

```powershell
cargo run -p bill-analyser-http --bin bill_weaviate_derived_index -- --mode bootstrap
```

Process one outbox batch:

```powershell
cargo run -p bill-analyser-http --bin bill_weaviate_derived_index -- --mode process-outbox
```

Rebuild one user's derived objects from PostgreSQL:

```powershell
cargo run -p bill-analyser-http --bin bill_weaviate_derived_index -- --mode rebuild --user-id 1
```

Rebuild requires `BILL_ANALYSER_POSTGRES_URL`. When `--user-id` is provided, the command deletes that user's derived objects in each Bill Analyser Weaviate collection before re-upserting objects from PostgreSQL feature rows.

## Import recall

Stage2 import learning first runs deterministic category/account/learning rules. When Weaviate is enabled, rows without a deterministic learning signal build normalized counterparty, description, and composite feature payloads and query the derived collections with metadata filters for:

- `userId`
- `featureSchemaVersion`
- `transactionType`
- `ruleState = postgres_authoritative`

Vector hits are converted back into the same preview learning signal shape as deterministic learning, including recommendation key, previous/applied preview snapshots, score, source, and lifecycle counters. Weaviate metadata is derived evidence only: it can surface a pending recommendation, but it cannot by itself enable auto-apply. Green auto-apply still requires authoritative lifecycle state and a deterministic or otherwise authoritative projection.

Transfer previews keep their `转账` type. Learning may recommend transfer-compatible category/account fields, but conflicting learned income/expense/investment types are ignored instead of changing the transfer row.

## Health semantics

- `disabled`: vector service is off and deterministic import remains fully available.
- `configured`: config has an endpoint; synchronous health rendering has not probed it.
- `healthy`: `/v1/.well-known/ready` returned success.
- `degraded:<reason>`: enabled but endpoint is missing, readiness failed, or recall failed. Deterministic import must continue without vector recall.
