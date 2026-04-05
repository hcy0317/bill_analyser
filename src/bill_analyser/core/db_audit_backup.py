"""Audit-log and backup persistence helpers for the split database facade."""

# pylint: disable=missing-function-docstring,line-too-long,too-many-arguments,too-many-positional-arguments,too-many-locals

from __future__ import annotations

import json
from typing import Any

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso


class DatabaseAuditBackupMixin(DatabaseFacadeBase):
    """Audit-log and backup-record persistence helpers."""

    @log_method
    async def create_audit_log(
        self,
        operation_type: str,
        operation_target: str,
        target_id: int | None = None,
        details: dict[str, Any] | None = None,
        affected_count: int = 0,
        ip_address: str | None = None,
        user_agent: str | None = None,
        session_id: str | None = None,
        status: str = "success",
        error_message: str | None = None,
    ) -> int:
        conn = await self._get_connection()
        now = utc_now_iso()
        details_json = json.dumps(details, ensure_ascii=False) if details else None

        cursor = await conn.execute(
            """
            INSERT INTO audit_logs (
                operation_type, operation_target, target_id, details,
                affected_count, ip_address, user_agent, session_id,
                status, error_message, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                operation_type,
                operation_target,
                target_id,
                details_json,
                affected_count,
                ip_address,
                user_agent,
                session_id,
                status,
                error_message,
                now,
            ),
        )
        await conn.commit()

        log_id = int(cursor.lastrowid or 0)
        self.logger.info(
            "创建审计日志: type=%s, target=%s, target_id=%s, status=%s, affected=%s",
            operation_type,
            operation_target,
            target_id,
            status,
            affected_count,
        )
        return log_id

    @log_method
    async def get_audit_logs(
        self,
        operation_type: str | None = None,
        operation_target: str | None = None,
        target_id: int | None = None,
        status: str | None = None,
        limit: int = 100,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        conditions: list[str] = []
        params: list[Any] = []

        if operation_type:
            conditions.append("operation_type = ?")
            params.append(operation_type)
        if operation_target:
            conditions.append("operation_target = ?")
            params.append(operation_target)
        if target_id is not None:
            conditions.append("target_id = ?")
            params.append(target_id)
        if status:
            conditions.append("status = ?")
            params.append(status)

        where_clause = f"WHERE {' AND '.join(conditions)}" if conditions else ""
        query = f"""
        SELECT * FROM audit_logs
        {where_clause}
        ORDER BY created_at DESC
        LIMIT ? OFFSET ?
        """
        params.extend([limit, offset])

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()

        logs: list[dict[str, Any]] = []
        for row in rows:
            log = dict(row)
            if log.get("details"):
                try:
                    log["details"] = json.loads(log["details"])
                except json.JSONDecodeError:
                    pass
            logs.append(log)
        return logs

    @log_method
    async def create_backup_record(self, payload: dict[str, Any]) -> int:
        conn = await self._get_connection()
        now = utc_now_iso()
        metadata_json = json.dumps(payload.get("metadata", {}), ensure_ascii=False)

        cursor = await conn.execute(
            """
            INSERT INTO backup_records (
                backup_name, storage_type, file_path, checksum,
                encrypted, status, metadata_json, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(backup_name) DO UPDATE SET
                storage_type = excluded.storage_type,
                file_path = excluded.file_path,
                checksum = excluded.checksum,
                encrypted = excluded.encrypted,
                status = excluded.status,
                metadata_json = excluded.metadata_json,
                updated_at = excluded.updated_at
            """,
            (
                payload.get("backup_name"),
                payload.get("storage_type", "local"),
                payload.get("file_path", ""),
                payload.get("checksum", ""),
                1 if payload.get("encrypted", False) else 0,
                payload.get("status", "created"),
                metadata_json,
                now,
                now,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid or 0)

    @log_method
    async def get_backup_records(self) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        async with conn.execute("SELECT * FROM backup_records ORDER BY created_at DESC, id DESC") as cursor:
            rows = await cursor.fetchall()

        result: list[dict[str, Any]] = []
        for row in rows:
            record = dict(row)
            try:
                record["metadata"] = json.loads(record.get("metadata_json") or "{}")
            except (TypeError, ValueError, json.JSONDecodeError):
                record["metadata"] = {}
            record["encrypted"] = bool(record.get("encrypted", 0))
            result.append(record)
        return result

    @log_method
    async def update_backup_record_by_filename(self, filename: str, updates: dict[str, Any]) -> bool:
        if not filename or not updates:
            return False

        conn = await self._get_connection()
        update_payload = {**updates, "updated_at": utc_now_iso()}
        if "metadata" in update_payload:
            metadata = dict(update_payload.pop("metadata") or {})
            async with conn.execute(
                "SELECT metadata_json FROM backup_records WHERE backup_name = ?",
                (filename,),
            ) as cursor:
                row = await cursor.fetchone()

            existing_metadata: dict[str, Any] = {}
            if row and row[0]:
                try:
                    existing_metadata = json.loads(row[0])
                except (TypeError, ValueError, json.JSONDecodeError):
                    existing_metadata = {}

            existing_metadata.update(metadata)
            update_payload["metadata_json"] = json.dumps(existing_metadata, ensure_ascii=False)

        set_clause = ", ".join(f"{key} = ?" for key in update_payload)
        values = [*update_payload.values(), filename]
        cursor = await conn.execute(
            f"UPDATE backup_records SET {set_clause} WHERE backup_name = ?",
            values,
        )
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def get_backup_jobs(self) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        async with conn.execute("SELECT * FROM backup_jobs ORDER BY created_at DESC, id DESC") as cursor:
            rows = await cursor.fetchall()

        result: list[dict[str, Any]] = []
        for row in rows:
            record = dict(row)
            record["enabled"] = bool(record.get("enabled", 0))
            result.append(record)
        return result

    @log_method
    async def create_or_update_backup_job(self, payload: dict[str, Any]) -> int:
        conn = await self._get_connection()
        now = utc_now_iso()
        job_id = int(payload.get("id") or 0)

        if job_id:
            await conn.execute(
                """
                UPDATE backup_jobs
                SET job_type = ?, schedule_expr = ?, retention_days = ?, retention_count = ?,
                    enabled = ?, last_status = ?, updated_at = ?
                WHERE id = ?
                """,
                (
                    payload.get("job_type"),
                    payload.get("schedule_expr"),
                    payload.get("retention_days", 30),
                    payload.get("retention_count", 10),
                    1 if payload.get("enabled", True) else 0,
                    payload.get("last_status"),
                    now,
                    job_id,
                ),
            )
            await conn.commit()
            return job_id

        cursor = await conn.execute(
            """
            INSERT INTO backup_jobs (
                job_type, schedule_expr, retention_days, retention_count,
                enabled, last_run_at, last_status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                payload.get("job_type"),
                payload.get("schedule_expr"),
                payload.get("retention_days", 30),
                payload.get("retention_count", 10),
                1 if payload.get("enabled", True) else 0,
                payload.get("last_run_at"),
                payload.get("last_status"),
                now,
                now,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid or 0)
