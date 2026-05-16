param(
    [string]$SourceDbPath = "",
    [string]$WorkDir = "",
    [switch]$KeepCopy
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ([string]::IsNullOrWhiteSpace($SourceDbPath)) {
    if (-not [string]::IsNullOrWhiteSpace($env:BILL_ANALYSER_SQLITE_DB_PATH)) {
        $SourceDbPath = $env:BILL_ANALYSER_SQLITE_DB_PATH
    } else {
        $SourceDbPath = Join-Path (Resolve-Path (Join-Path $PSScriptRoot "..")).Path "data\bills.db"
    }
}

if (-not (Test-Path -LiteralPath $SourceDbPath -PathType Leaf)) {
    throw "Source database not found: $SourceDbPath"
}

$sourcePath = (Resolve-Path -LiteralPath $SourceDbPath).Path
$root = if ([string]::IsNullOrWhiteSpace($WorkDir)) {
    Join-Path ([System.IO.Path]::GetTempPath()) "bill-analyser-import-perf"
} else {
    $WorkDir
}
$runId = Get-Date -Format "yyyyMMdd-HHmmss-fff"
$runRoot = Join-Path $root $runId
New-Item -ItemType Directory -Path $runRoot -Force | Out-Null
$copyPath = Join-Path $runRoot "bills-copy.db"

$metadata = [ordered]@{
    source_path = $sourcePath
    copy_path = $copyPath
    copied_at = (Get-Date).ToString("o")
    source_size_bytes = (Get-Item -LiteralPath $sourcePath).Length
    copy_size_bytes = 0
    copy_retained = [bool]$KeepCopy
    engine = "none"
    integrity_check = $null
    table_counts = @{}
    largest_parser_sessions = @()
    largest_preview_sessions = @()
    benchmarks = @{}
}

try {
    Copy-Item -LiteralPath $sourcePath -Destination $copyPath -Force
    $metadata.copy_size_bytes = (Get-Item -LiteralPath $copyPath).Length

    $python = Get-Command python -ErrorAction SilentlyContinue
    if ($python) {
        $pythonCode = @'
import json
import sqlite3
import sys
import time

path = sys.argv[1]
tables = [
    "users",
    "bills",
    "import_sessions",
    "bills_parser_template",
    "bills_preview",
    "import_annotation_samples",
]

conn = sqlite3.connect(path)
try:
    cur = conn.cursor()
    integrity = cur.execute("PRAGMA integrity_check").fetchone()[0]
    counts = {}
    for table in tables:
        exists = cur.execute(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?",
            (table,),
        ).fetchone()
        counts[table] = None if exists is None else cur.execute(
            f"SELECT COUNT(*) FROM {table}"
        ).fetchone()[0]

    largest_parser_sessions = []
    parser_exists = cur.execute(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'bills_parser_template'"
    ).fetchone()
    if parser_exists is not None:
        largest_parser_sessions = [
            {"session_id": row[0], "user_id": row[1], "parser_rows": row[2]}
            for row in cur.execute(
                """
                SELECT session_id, user_id, COUNT(*)
                FROM bills_parser_template
                GROUP BY session_id, user_id
                ORDER BY COUNT(*) DESC
                LIMIT 5
                """
            )
        ]

    largest_preview_sessions = []
    preview_exists = cur.execute(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'bills_preview'"
    ).fetchone()
    if preview_exists is not None:
        largest_preview_sessions = [
            {"session_id": row[0], "user_id": row[1], "preview_rows": row[2]}
            for row in cur.execute(
                """
                SELECT session_id, user_id, COUNT(*)
                FROM bills_preview
                GROUP BY session_id, user_id
                ORDER BY COUNT(*) DESC
                LIMIT 5
                """
            )
        ]

    benchmarks = {}
    if largest_parser_sessions:
        session = largest_parser_sessions[0]
        params = (session["session_id"], session["user_id"])
        update_sql = """
            UPDATE bills_parser_template
            SET parser_is_processed = '1'
            WHERE session_id = ? AND user_id = ? AND parser_is_processed = '0'
        """
        unprocessed_rows = cur.execute(
            """
            SELECT COUNT(*)
            FROM bills_parser_template
            WHERE session_id = ? AND user_id = ? AND parser_is_processed = '0'
            """,
            params,
        ).fetchone()[0]
        update_plan = [
            row[3] for row in cur.execute("EXPLAIN QUERY PLAN " + update_sql, params)
        ]
        cur.execute("BEGIN IMMEDIATE")
        started = time.perf_counter()
        updated_rows = cur.execute(update_sql, params).rowcount
        elapsed_ms = round((time.perf_counter() - started) * 1000, 3)
        conn.rollback()
        benchmarks["parser_template_session_update"] = {
            "session_id": session["session_id"],
            "user_id": session["user_id"],
            "unprocessed_rows": unprocessed_rows,
            "rolled_back_updated_rows": updated_rows,
            "elapsed_ms": elapsed_ms,
            "query_plan": update_plan,
        }

    if largest_preview_sessions:
        session = largest_preview_sessions[0]
        params = (session["session_id"], session["user_id"])
        preview_index_sql = """
            SELECT
                id,
                preview_date,
                preview_type,
                preview_amount,
                preview_main_category,
                preview_sub_category,
                preview_source_account_id,
                preview_destination_account_id,
                preview_counterparty,
                preview_payment_method,
                preview_description,
                preview_parser_id,
                preview_parser_tags_json,
                preview_recurring_id,
                preview_recurring_candidate_count,
                preview_recurring_match_reasons,
                preview_recurring_matched_date,
                preview_selected,
                dedup_type,
                dedup_source_ids,
                preview_matching_feedback_json
            FROM bills_preview
            WHERE session_id = ? AND user_id = ?
            ORDER BY preview_date ASC, id ASC
        """
        preview_plan = [
            row[3] for row in cur.execute("EXPLAIN QUERY PLAN " + preview_index_sql, params)
        ]
        started = time.perf_counter()
        preview_rows = cur.execute(preview_index_sql, params).fetchall()
        elapsed_ms = round((time.perf_counter() - started) * 1000, 3)
        benchmarks["preview_filter_index_projection"] = {
            "session_id": session["session_id"],
            "user_id": session["user_id"],
            "rows": len(preview_rows),
            "elapsed_ms": elapsed_ms,
            "query_plan": preview_plan,
        }

    print(json.dumps({
        "integrity_check": integrity,
        "table_counts": counts,
        "largest_parser_sessions": largest_parser_sessions,
        "largest_preview_sessions": largest_preview_sessions,
        "benchmarks": benchmarks,
    }, ensure_ascii=False))
finally:
    if conn.in_transaction:
        conn.rollback()
    conn.close()
'@
        $pythonJson = $pythonCode | & $python.Source - $copyPath
        $pythonMetadata = $pythonJson | ConvertFrom-Json
        $metadata.engine = "python-sqlite3"
        $metadata.integrity_check = $pythonMetadata.integrity_check
        $metadata.table_counts = $pythonMetadata.table_counts
        $metadata.largest_parser_sessions = $pythonMetadata.largest_parser_sessions
        $metadata.largest_preview_sessions = $pythonMetadata.largest_preview_sessions
        $metadata.benchmarks = $pythonMetadata.benchmarks
    } else {
        $sqlite = Get-Command sqlite3 -ErrorAction SilentlyContinue
        if ($sqlite) {
            $metadata.engine = "sqlite3"
            $metadata.integrity_check = (& $sqlite.Source $copyPath "PRAGMA integrity_check;")
            foreach ($table in @("users", "bills", "import_sessions", "bills_parser_template", "bills_preview")) {
                $exists = & $sqlite.Source $copyPath "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '$table';"
                if ($exists -eq "1") {
                    $metadata.table_counts[$table] = [int64](& $sqlite.Source $copyPath "SELECT COUNT(*) FROM $table;")
                } else {
                    $metadata.table_counts[$table] = $null
                }
            }
        }
    }

    $metadata | ConvertTo-Json -Depth 6
} finally {
    if (-not $KeepCopy -and (Test-Path -LiteralPath $runRoot)) {
        Remove-Item -LiteralPath $runRoot -Recurse -Force
    }
}
