"""DB integration coverage for audit-log and backup-job edge paths."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from bill_analyser.core.db import Database

# pylint: disable=duplicate-code,protected-access

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_audit_backup_paths.db"))
    await db.init_db()
    return db


@pytest.mark.asyncio
async def test_backup_jobs_roundtrip_and_invalid_metadata_merge_paths(tmp_path: Path) -> None:
    """备份域应覆盖 job CRUD 查询与损坏 metadata 的合并恢复路径。"""
    db = await _create_database(tmp_path)
    try:
        assert await db.get_backup_jobs() == []

        backup_record_id = await db.create_backup_record(
            {
                "backup_name": "broken-metadata.zip",
                "storage_type": "local",
                "file_path": "C:/tmp/broken-metadata.zip",
                "checksum": "checksum-broken",
                "encrypted": False,
                "status": "created",
                "metadata": {},
            }
        )
        assert backup_record_id >= 0

        conn = await db._get_connection()
        await conn.execute(
            "UPDATE backup_records SET metadata_json = ? WHERE backup_name = ?",
            ("{broken-json", "broken-metadata.zip"),
        )
        await conn.commit()

        assert await db.update_backup_record_by_filename(
            "broken-metadata.zip",
            {
                "status": "verified",
                "metadata": {"recovered": True, "verified_by": "pytest"},
            },
        ) is True

        backup_records = await db.get_backup_records()
        broken_record = next(
            record
            for record in backup_records
            if record["backup_name"] == "broken-metadata.zip"
        )
        assert broken_record["status"] == "verified"
        assert broken_record["metadata"] == {"recovered": True, "verified_by": "pytest"}

        cleanup_job_id = await db.create_or_update_backup_job(
            {
                "job_type": "cleanup",
                "schedule_expr": "0 0 * * *",
                "retention_days": 7,
                "retention_count": 2,
                "enabled": False,
                "last_status": "idle",
            }
        )
        periodic_job_id = await db.create_or_update_backup_job(
            {
                "job_type": "full_backup",
                "schedule_expr": "0 1 * * *",
                "retention_days": 30,
                "retention_count": 10,
            }
        )
        assert cleanup_job_id > 0
        assert periodic_job_id > cleanup_job_id

        jobs = await db.get_backup_jobs()
        assert [int(job["id"]) for job in jobs] == [periodic_job_id, cleanup_job_id]
        jobs_by_id = {int(job["id"]): job for job in jobs}
        assert jobs_by_id[cleanup_job_id]["enabled"] is False
        assert jobs_by_id[periodic_job_id]["enabled"] is True
        assert int(jobs_by_id[cleanup_job_id]["retention_days"]) == 7
        assert int(jobs_by_id[periodic_job_id]["retention_count"]) == 10

        updated_job_id = await db.create_or_update_backup_job(
            {
                "id": cleanup_job_id,
                "job_type": "cleanup",
                "schedule_expr": "0 2 * * *",
                "retention_days": 14,
                "retention_count": 5,
                "enabled": True,
                "last_status": "success",
            }
        )
        assert updated_job_id == cleanup_job_id

        refreshed_jobs = await db.get_backup_jobs()
        refreshed_cleanup_job = next(
            job for job in refreshed_jobs if int(job["id"]) == cleanup_job_id
        )
        assert refreshed_cleanup_job["schedule_expr"] == "0 2 * * *"
        assert int(refreshed_cleanup_job["retention_days"]) == 14
        assert int(refreshed_cleanup_job["retention_count"]) == 5
        assert refreshed_cleanup_job["enabled"] is True
        assert refreshed_cleanup_job["last_status"] == "success"
    finally:
        await db.close()
