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


class ReconciliationActionsMixin(object):
        async def _set_reconciliation_candidate_status(
            self,
            conn: Any,
            *,
            row_id: int,
            user_id: int,
            status: str,
            now: str,
            event_id: int | None = None,
        ) -> None:
            resolved_at = now if status in {"merged", "accepted", "rejected"} else None
            await conn.execute(
                """
                UPDATE bill_reconciliation_candidates
                SET status = ?, resolved_at = ?, resolution_event_id = ?,
                    updated_at = ?
                WHERE id = ? AND user_id = ?
                """,
                (status, resolved_at, event_id, now, row_id, user_id),
            )

        @log_method
        async def accept_import_reconciliation_candidate(
            self,
            candidate_id: str,
            *,
            user_id: int = 1,
        ) -> dict[str, Any]:
            """Apply a manual import-to-existing reconciliation candidate."""
            normalized_user_id = int(user_id)
            normalized_candidate_id = str(candidate_id or "").strip()
            if normalized_user_id <= 0 or not normalized_candidate_id:
                raise ValueError("Invalid reconciliation candidate")

            conn = await self._get_connection()
            try:
                await conn.execute("BEGIN IMMEDIATE")
                candidate = await self._get_reconciliation_candidate_with_group(
                    conn,
                    candidate_id=normalized_candidate_id,
                    user_id=normalized_user_id,
                )
                group_id = int(candidate.get("group_id") or 0)
                base_bill, metadata = await self._prepare_reconciliation_base_snapshot(
                    conn,
                    candidate,
                    user_id=normalized_user_id,
                )
                now = utc_now_iso()
                preview_id = await self._find_reconciliation_preview_id(
                    conn,
                    candidate,
                    user_id=normalized_user_id,
                )
                preview_selection_before = None
                if preview_id is not None:
                    async with conn.execute(
                        "SELECT preview_selected FROM bills_preview WHERE id = ? AND user_id = ?",
                        (preview_id, normalized_user_id),
                    ) as cursor:
                        preview_row = await cursor.fetchone()
                    preview_selection_before = (
                        bool(preview_row["preview_selected"]) if preview_row else None
                    )

                await self._set_reconciliation_candidate_status(
                    conn,
                    row_id=int(candidate["id"]),
                    user_id=normalized_user_id,
                    status="merged",
                    now=now,
                )
                projection = await self._recompute_reconciliation_projection(
                    conn,
                    group_id=group_id,
                    group_key=str(candidate.get("group_key") or ""),
                    group_type=str(candidate.get("candidate_type") or ""),
                    base_bill=base_bill,
                    user_id=normalized_user_id,
                    now=now,
                    metadata=metadata,
                )
                await self._set_reconciliation_preview_selected(
                    conn,
                    preview_id=preview_id,
                    selected=False,
                    user_id=normalized_user_id,
                )
                event_id = await self._append_bill_merge_event(
                    conn,
                    user_id=normalized_user_id,
                    group_id=group_id,
                    candidate_id=normalized_candidate_id,
                    event_type="merge_applied",
                    payload={
                        "candidate_type": candidate.get("candidate_type"),
                        "base_bill": base_bill,
                        "projection": projection,
                        "preview_id": preview_id,
                        "preview_selected_before": preview_selection_before,
                    },
                    now=now,
                )
                await self._set_reconciliation_candidate_status(
                    conn,
                    row_id=int(candidate["id"]),
                    user_id=normalized_user_id,
                    status="merged",
                    now=now,
                    event_id=event_id,
                )
                await conn.commit()
                return {
                    "candidate_id": normalized_candidate_id,
                    "action": "accept",
                    "group_id": group_id,
                    "bill": {
                        **base_bill,
                        "description": projection.get("description", ""),
                        "tag_ids": list(projection.get("tag_ids") or []),
                    },
                    "projection": projection,
                }
            except Exception:
                await conn.rollback()
                raise

        @log_method
        async def reject_import_reconciliation_candidate(
            self,
            candidate_id: str,
            *,
            user_id: int = 1,
        ) -> dict[str, Any]:
            """Reject a reconciliation candidate and undo its projection if needed."""
            normalized_user_id = int(user_id)
            normalized_candidate_id = str(candidate_id or "").strip()
            if normalized_user_id <= 0 or not normalized_candidate_id:
                raise ValueError("Invalid reconciliation candidate")

            conn = await self._get_connection()
            try:
                await conn.execute("BEGIN IMMEDIATE")
                candidate = await self._get_reconciliation_candidate_with_group(
                    conn,
                    candidate_id=normalized_candidate_id,
                    user_id=normalized_user_id,
                )
                group_id = int(candidate.get("group_id") or 0)
                was_applied = str(candidate.get("status") or "") in self._APPLIED_STATUSES
                base_bill, metadata = await self._prepare_reconciliation_base_snapshot(
                    conn,
                    candidate,
                    user_id=normalized_user_id,
                )
                now = utc_now_iso()
                preview_id = await self._find_reconciliation_preview_id(
                    conn,
                    candidate,
                    user_id=normalized_user_id,
                )
                await self._set_reconciliation_candidate_status(
                    conn,
                    row_id=int(candidate["id"]),
                    user_id=normalized_user_id,
                    status="rejected",
                    now=now,
                )
                projection = await self._recompute_reconciliation_projection(
                    conn,
                    group_id=group_id,
                    group_key=str(candidate.get("group_key") or ""),
                    group_type=str(candidate.get("candidate_type") or ""),
                    base_bill=base_bill,
                    user_id=normalized_user_id,
                    now=now,
                    metadata=metadata,
                )
                if was_applied and preview_id is not None:
                    group_candidates = await self._load_group_candidates(
                        conn,
                        group_key=str(candidate.get("group_key") or ""),
                        user_id=normalized_user_id,
                    )
                    same_preview_still_applied = False
                    for group_candidate in group_candidates:
                        if str(group_candidate.get("status") or "") not in self._APPLIED_STATUSES:
                            continue
                        group_preview_id = await self._find_reconciliation_preview_id(
                            conn,
                            group_candidate,
                            user_id=normalized_user_id,
                        )
                        same_preview_still_applied = group_preview_id == preview_id
                        if same_preview_still_applied:
                            break
                    if not same_preview_still_applied:
                        await self._set_reconciliation_preview_selected(
                            conn,
                            preview_id=preview_id,
                            selected=True,
                            user_id=normalized_user_id,
                        )
                event_id = await self._append_bill_merge_event(
                    conn,
                    user_id=normalized_user_id,
                    group_id=group_id,
                    candidate_id=normalized_candidate_id,
                    event_type="candidate_rejected",
                    payload={
                        "candidate_type": candidate.get("candidate_type"),
                        "projection": projection,
                    },
                    now=now,
                )
                await self._set_reconciliation_candidate_status(
                    conn,
                    row_id=int(candidate["id"]),
                    user_id=normalized_user_id,
                    status="rejected",
                    now=now,
                    event_id=event_id,
                )
                await conn.commit()
                return {
                    "candidate_id": normalized_candidate_id,
                    "action": "reject",
                    "group_id": group_id,
                    "projection": projection,
                }
            except Exception:
                await conn.rollback()
                raise

        @log_method
        async def clear_import_reconciliation_candidate(
            self,
            candidate_id: str,
            *,
            user_id: int = 1,
        ) -> dict[str, Any]:
            """Clear a reconciliation decision and recompute the group projection."""
            normalized_user_id = int(user_id)
            normalized_candidate_id = str(candidate_id or "").strip()
            if normalized_user_id <= 0 or not normalized_candidate_id:
                raise ValueError("Invalid reconciliation candidate")

            conn = await self._get_connection()
            try:
                await conn.execute("BEGIN IMMEDIATE")
                candidate = await self._get_reconciliation_candidate_with_group(
                    conn,
                    candidate_id=normalized_candidate_id,
                    user_id=normalized_user_id,
                )
                group_id = int(candidate.get("group_id") or 0)
                base_bill, metadata = await self._prepare_reconciliation_base_snapshot(
                    conn,
                    candidate,
                    user_id=normalized_user_id,
                )
                now = utc_now_iso()
                preview_id = await self._find_reconciliation_preview_id(
                    conn,
                    candidate,
                    user_id=normalized_user_id,
                )
                await self._set_reconciliation_candidate_status(
                    conn,
                    row_id=int(candidate["id"]),
                    user_id=normalized_user_id,
                    status=self._PENDING_STATUS,
                    now=now,
                )
                projection = await self._recompute_reconciliation_projection(
                    conn,
                    group_id=group_id,
                    group_key=str(candidate.get("group_key") or ""),
                    group_type=str(candidate.get("candidate_type") or ""),
                    base_bill=base_bill,
                    user_id=normalized_user_id,
                    now=now,
                    metadata=metadata,
                )
                if preview_id is not None:
                    group_candidates = await self._load_group_candidates(
                        conn,
                        group_key=str(candidate.get("group_key") or ""),
                        user_id=normalized_user_id,
                    )
                    same_preview_still_applied = False
                    for group_candidate in group_candidates:
                        if str(group_candidate.get("status") or "") not in self._APPLIED_STATUSES:
                            continue
                        group_preview_id = await self._find_reconciliation_preview_id(
                            conn,
                            group_candidate,
                            user_id=normalized_user_id,
                        )
                        same_preview_still_applied = group_preview_id == preview_id
                        if same_preview_still_applied:
                            break
                    if not same_preview_still_applied:
                        await self._set_reconciliation_preview_selected(
                            conn,
                            preview_id=preview_id,
                            selected=True,
                            user_id=normalized_user_id,
                        )

                event_id = await self._append_bill_merge_event(
                    conn,
                    user_id=normalized_user_id,
                    group_id=group_id,
                    candidate_id=normalized_candidate_id,
                    event_type="merge_rolled_back",
                    payload={
                        "candidate_type": candidate.get("candidate_type"),
                        "projection": projection,
                        "preview_id": preview_id,
                    },
                    now=now,
                )
                await self._set_reconciliation_candidate_status(
                    conn,
                    row_id=int(candidate["id"]),
                    user_id=normalized_user_id,
                    status=self._PENDING_STATUS,
                    now=now,
                    event_id=event_id,
                )
                await conn.commit()
                return {
                    "candidate_id": normalized_candidate_id,
                    "action": "clear",
                    "group_id": group_id,
                    "projection": projection,
                }
            except Exception:
                await conn.rollback()
                raise

        @log_method
        async def record_bill_merge_event(
            self,
            *,
            group_id: int,
            candidate_id: str,
            event_type: str,
            payload: dict[str, Any] | None = None,
            user_id: int = 1,
        ) -> dict[str, Any]:
            """Append a merge-ledger event without mutating bill data."""
            normalized_group_id = int(group_id)
            normalized_user_id = int(user_id)
            normalized_candidate_id = str(candidate_id or "").strip()
            normalized_event_type = str(event_type or "").strip().lower()
            if normalized_group_id <= 0 or normalized_user_id <= 0 or not normalized_event_type:
                raise ValueError("Invalid merge event")

            now = utc_now_iso()
            conn = await self._get_connection()
            async with conn.execute(
                """
                SELECT id
                FROM bill_merge_groups
                WHERE id = ? AND user_id = ?
                LIMIT 1
                """,
                (normalized_group_id, normalized_user_id),
            ) as cursor:
                group_row = await cursor.fetchone()
            if group_row is None:
                raise ValueError("Merge group not found")

            event_id = await self._append_bill_merge_event(
                conn,
                user_id=normalized_user_id,
                group_id=normalized_group_id,
                candidate_id=normalized_candidate_id,
                event_type=normalized_event_type,
                payload=payload or {},
                now=now,
            )
            await conn.commit()
            return {
                "id": event_id,
                "group_id": normalized_group_id,
                "candidate_id": normalized_candidate_id,
                "event_type": normalized_event_type,
                "payload": payload or {},
                "created_at": now,
            }
