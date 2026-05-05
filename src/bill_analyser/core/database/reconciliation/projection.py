"""Import reconciliation candidate and merge-ledger persistence helpers."""

# pylint: disable=too-many-arguments,too-many-locals

from __future__ import annotations

import json
import sqlite3
from difflib import SequenceMatcher
from typing import Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now_iso
from .base import ReconciliationProjectionConflictError


class ReconciliationProjectionMixin(object):
        async def _get_reconciliation_candidate_with_group(
            self,
            conn: Any,
            *,
            candidate_id: str,
            user_id: int,
        ) -> dict[str, Any]:
            async with conn.execute(
                """
                SELECT c.*, g.id AS group_id, g.status AS group_status,
                       g.canonical_bill_id AS canonical_bill_id,
                       g.metadata_json AS group_metadata_json
                FROM bill_reconciliation_candidates c
                JOIN bill_merge_groups g
                  ON g.user_id = c.user_id
                 AND g.family = c.family
                 AND g.group_key = c.group_key
                WHERE c.user_id = ? AND c.candidate_id = ?
                LIMIT 1
                """,
                (user_id, candidate_id),
            ) as cursor:
                row = await cursor.fetchone()
            if row is None:
                raise LookupError("Reconciliation candidate not found")
            return self._row_to_reconciliation_candidate(dict(row))

        async def _get_bill_projection_snapshot(
            self,
            conn: Any,
            *,
            bill_id: int,
            user_id: int,
        ) -> dict[str, Any]:
            async with conn.execute(
                "SELECT * FROM bills WHERE id = ? AND user_id = ?",
                (bill_id, user_id),
            ) as cursor:
                bill_row = await cursor.fetchone()
            if bill_row is None:
                raise LookupError("Bill not found")

            async with conn.execute(
                """
                SELECT bt.tag_id
                FROM bill_tags bt
                JOIN tags t ON t.id = bt.tag_id
                WHERE bt.bill_id = ? AND t.user_id = ?
                ORDER BY bt.tag_id
                """,
                (bill_id, user_id),
            ) as cursor:
                tag_rows = await cursor.fetchall()

            snapshot = dict(bill_row)
            snapshot["tag_ids"] = [int(row["tag_id"]) for row in tag_rows]
            return snapshot

        async def _filter_existing_tag_ids(
            self,
            conn: Any,
            tag_ids: list[int],
            *,
            user_id: int,
        ) -> list[int]:
            normalized_tag_ids = self._normalize_tag_ids(tag_ids)
            if not normalized_tag_ids:
                return []
            placeholders = ",".join("?" for _ in normalized_tag_ids)
            async with conn.execute(
                f"SELECT id FROM tags WHERE user_id = ? AND id IN ({placeholders})",
                (user_id, *normalized_tag_ids),
            ) as cursor:
                rows = await cursor.fetchall()
            existing_ids = {int(row["id"]) for row in rows}
            return [tag_id for tag_id in normalized_tag_ids if tag_id in existing_ids]

        async def _replace_bill_projection_tags(
            self,
            conn: Any,
            *,
            bill_id: int,
            tag_ids: list[int],
            user_id: int,
            now: str,
        ) -> list[int]:
            filtered_tag_ids = await self._filter_existing_tag_ids(
                conn,
                tag_ids,
                user_id=user_id,
            )
            await conn.execute("DELETE FROM bill_tags WHERE bill_id = ?", (bill_id,))
            if filtered_tag_ids:
                await conn.executemany(
                    "INSERT OR IGNORE INTO bill_tags (bill_id, tag_id, created_at) VALUES (?, ?, ?)",
                    [(bill_id, tag_id, now) for tag_id in filtered_tag_ids],
                )
            return filtered_tag_ids

        async def _assert_reconciliation_projection_current(
            self,
            conn: Any,
            *,
            bill_id: int,
            user_id: int,
            metadata: dict[str, Any],
        ) -> None:
            projection = (
                dict(metadata.get("projection"))
                if isinstance(metadata.get("projection"), dict)
                else {}
            )
            candidate_ids = [
                str(candidate_id)
                for candidate_id in list(projection.get("candidate_ids") or [])
                if str(candidate_id)
            ]
            if not projection or not candidate_ids:
                return

            current_bill = await self._get_bill_projection_snapshot(
                conn,
                bill_id=bill_id,
                user_id=user_id,
            )
            current_description = str(current_bill.get("description") or "")
            projected_description = str(projection.get("description") or "")
            current_tag_ids = sorted(self._normalize_tag_ids(current_bill.get("tag_ids")))
            projected_tag_ids = sorted(self._normalize_tag_ids(projection.get("tag_ids")))
            if (
                current_description != projected_description
                or current_tag_ids != projected_tag_ids
            ):
                raise ReconciliationProjectionConflictError(
                    "Bill changed since reconciliation projection, please refresh"
                )

        async def _find_reconciliation_preview_id(
            self,
            conn: Any,
            candidate: dict[str, Any],
            *,
            user_id: int,
        ) -> int | None:
            preview_id = self._normalize_optional_int(candidate.get("preview_id"))
            if preview_id is not None:
                return preview_id

            session_id = str(candidate.get("session_id") or "").strip()
            import_bill_key = str(candidate.get("import_bill_key") or "").strip()
            template_prefix = f"session:{session_id}:template:"
            if not session_id or not import_bill_key.startswith(template_prefix):
                return None
            template_id = self._normalize_optional_int(import_bill_key.removeprefix(template_prefix))
            if template_id is None:
                return None

            async with conn.execute(
                """
                SELECT id, dedup_source_ids
                FROM bills_preview
                WHERE session_id = ? AND user_id = ?
                ORDER BY id
                """,
                (session_id, user_id),
            ) as cursor:
                preview_rows = await cursor.fetchall()
            for row in preview_rows:
                source_ids = self._normalize_tag_ids(row["dedup_source_ids"])
                if template_id in source_ids:
                    return int(row["id"])
            return None

        async def _set_reconciliation_preview_selected(
            self,
            conn: Any,
            *,
            preview_id: int | None,
            selected: bool,
            user_id: int,
        ) -> None:
            if preview_id is None:
                return
            await conn.execute(
                "UPDATE bills_preview SET preview_selected = ? WHERE id = ? AND user_id = ?",
                (1 if selected else 0, preview_id, user_id),
            )

        async def _load_group_candidates(
            self,
            conn: Any,
            *,
            group_key: str,
            user_id: int,
        ) -> list[dict[str, Any]]:
            async with conn.execute(
                """
                SELECT c.*, g.id AS group_id, g.status AS group_status,
                       g.canonical_bill_id AS canonical_bill_id,
                       g.metadata_json AS group_metadata_json
                FROM bill_reconciliation_candidates c
                JOIN bill_merge_groups g
                  ON g.user_id = c.user_id
                 AND g.family = c.family
                 AND g.group_key = c.group_key
                WHERE c.user_id = ? AND c.group_key = ?
                ORDER BY c.id ASC
                """,
                (user_id, group_key),
            ) as cursor:
                rows = await cursor.fetchall()
            return [self._row_to_reconciliation_candidate(dict(row)) for row in rows]

        async def _recompute_reconciliation_projection(
            self,
            conn: Any,
            *,
            group_id: int,
            group_key: str,
            group_type: str,
            base_bill: dict[str, Any],
            user_id: int,
            now: str,
            metadata: dict[str, Any],
        ) -> dict[str, Any]:
            bill_id = int(base_bill.get("id") or 0)
            if bill_id <= 0:
                raise LookupError("Bill not found")
            await self._assert_reconciliation_projection_current(
                conn,
                bill_id=bill_id,
                user_id=user_id,
                metadata=metadata,
            )

            group_candidates = await self._load_group_candidates(
                conn,
                group_key=group_key,
                user_id=user_id,
            )
            applied_candidates = [
                candidate
                for candidate in group_candidates
                if str(candidate.get("status") or "") in self._APPLIED_STATUSES
            ]
            previous_projection = (
                dict(metadata.get("projection"))
                if isinstance(metadata.get("projection"), dict)
                else {}
            )
            previous_candidate_ids = [
                str(candidate_id)
                for candidate_id in list(previous_projection.get("candidate_ids") or [])
                if str(candidate_id)
            ]
            had_applied_projection = bool(previous_projection and previous_candidate_ids)
            if not applied_candidates and not had_applied_projection:
                next_metadata = {**metadata}
                next_metadata.pop("base_bill_snapshot", None)
                next_metadata.pop("projection", None)
                await conn.execute(
                    """
                    UPDATE bill_merge_groups
                    SET status = ?, canonical_bill_id = NULL, metadata_json = ?, updated_at = ?
                    WHERE id = ? AND user_id = ?
                    """,
                    (
                        self._PENDING_STATUS,
                        self._json_dumps(next_metadata),
                        now,
                        group_id,
                        user_id,
                    ),
                )
                return {}

            import_snapshots = [
                dict(candidate.get("import_bill_snapshot") or {})
                for candidate in applied_candidates
            ]

            merged_description = self._merge_description_values(
                [
                    base_bill.get("description", ""),
                    *[snapshot.get("description", "") for snapshot in import_snapshots],
                ]
            )
            merged_tag_ids = self._normalize_tag_ids(base_bill.get("tag_ids"))
            for snapshot in import_snapshots:
                for tag_id in self._snapshot_tag_ids(snapshot):
                    if tag_id not in merged_tag_ids:
                        merged_tag_ids.append(tag_id)

            await conn.execute(
                "UPDATE bills SET description = ?, updated_at = ? WHERE id = ? AND user_id = ?",
                (merged_description, now, bill_id, user_id),
            )
            merged_tag_ids = await self._replace_bill_projection_tags(
                conn,
                bill_id=bill_id,
                tag_ids=merged_tag_ids,
                user_id=user_id,
                now=now,
            )

            signal_payload = self._build_reconciliation_projection_signal(
                candidate_type=group_type,
                base_bill=base_bill,
                import_snapshots=import_snapshots,
            )
            projection = {
                **signal_payload,
                "bill_id": bill_id,
                "description": merged_description,
                "tag_ids": merged_tag_ids,
                "candidate_ids": [
                    str(candidate.get("candidate_id") or "")
                    for candidate in applied_candidates
                    if str(candidate.get("candidate_id") or "")
                ],
            }
            next_metadata = {**metadata}
            if applied_candidates:
                next_metadata["base_bill_snapshot"] = base_bill
                next_metadata["projection"] = projection
            else:
                next_metadata.pop("base_bill_snapshot", None)
                next_metadata.pop("projection", None)
            next_status = "merged" if applied_candidates else self._PENDING_STATUS
            await conn.execute(
                """
                UPDATE bill_merge_groups
                SET status = ?, canonical_bill_id = ?, metadata_json = ?, updated_at = ?
                WHERE id = ? AND user_id = ?
                """,
                (
                    next_status,
                    bill_id if applied_candidates else None,
                    self._json_dumps(next_metadata),
                    now,
                    group_id,
                    user_id,
                ),
            )
            return projection

        async def _prepare_reconciliation_base_snapshot(
            self,
            conn: Any,
            candidate: dict[str, Any],
            *,
            user_id: int,
        ) -> tuple[dict[str, Any], dict[str, Any]]:
            metadata = dict(candidate.get("group_metadata") or {})
            base_bill = (
                dict(metadata.get("base_bill_snapshot"))
                if isinstance(metadata.get("base_bill_snapshot"), dict)
                else {}
            )
            if base_bill:
                return base_bill, metadata

            bill_id = int(candidate.get("existing_bill_id") or 0)
            base_bill = await self._get_bill_projection_snapshot(
                conn,
                bill_id=bill_id,
                user_id=user_id,
            )
            metadata["base_bill_snapshot"] = base_bill
            return base_bill, metadata

        @log_method
        async def get_bill_reconciliation_projection(
            self,
            bill_id: int,
            *,
            user_id: int = 1,
        ) -> dict[str, Any] | None:
            """Return the latest merge projection/provenance for a formal bill."""
            normalized_bill_id = int(bill_id)
            normalized_user_id = int(user_id)
            if normalized_bill_id <= 0 or normalized_user_id <= 0:
                return None

            conn = await self._get_connection()
            async with conn.execute(
                """
                SELECT g.*
                FROM bill_merge_groups g
                JOIN bill_merge_members m ON m.group_id = g.id
                WHERE g.user_id = ?
                  AND g.family = ?
                  AND m.bill_id = ?
                  AND m.member_type = 'existing_bill'
                ORDER BY g.updated_at DESC, g.id DESC
                """,
                (normalized_user_id, self._IMPORT_RECONCILIATION_FAMILY, normalized_bill_id),
            ) as cursor:
                group_rows = await cursor.fetchall()
            if not group_rows:
                return None

            for group_row in group_rows:
                group = dict(group_row)
                metadata = self._json_loads(group.get("metadata_json"))
                projection = (
                    dict(metadata.get("projection"))
                    if isinstance(metadata.get("projection"), dict)
                    else {}
                )
                candidate_ids = [
                    str(candidate_id)
                    for candidate_id in list(projection.get("candidate_ids") or [])
                    if str(candidate_id)
                ]
                if not projection or not candidate_ids:
                    continue
                return {
                    "group_id": int(group.get("id") or 0),
                    "group_type": str(group.get("group_type") or ""),
                    "status": str(group.get("status") or ""),
                    "canonical_bill_id": self._normalize_optional_int(group.get("canonical_bill_id")),
                    "signal_label": str(projection.get("signal_label") or ""),
                    "source_chain": (
                        list(projection.get("source_chain") or [])
                        if isinstance(projection.get("source_chain"), list)
                        else []
                    ),
                    "candidate_ids": candidate_ids,
                    "description": str(projection.get("description") or ""),
                    "tag_ids": self._normalize_tag_ids(projection.get("tag_ids")),
                }
            return None
