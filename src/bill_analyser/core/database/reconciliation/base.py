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


class ReconciliationProjectionConflictError(ValueError):
    """Raised when a bill changed after the last reconciliation projection."""


class ReconciliationBaseMixin(object):
        _IMPORT_RECONCILIATION_FAMILY = "import_reconciliation"

        _VALID_RECONCILIATION_TYPES = {"transfer", "duplicate"}

        _PENDING_STATUS = "pending"

        _APPLIED_STATUSES = {"accepted", "merged"}

        _PARSER_DISPLAY_LABELS = {
            "wechat": "微信",
            "alipay": "支付宝",
            "abc": "农业银行",
            "ccb": "建设银行",
            "cmbc": "民生银行",
            "icbc": "工商银行",
            "generic": "通用来源",
        }

        @staticmethod
        def _json_dumps(payload: Any) -> str:
            if not isinstance(payload, (dict, list)):
                payload = {}
            return json.dumps(payload, ensure_ascii=False, sort_keys=True)

        @staticmethod
        def _json_loads(raw_payload: Any) -> dict[str, Any]:
            if isinstance(raw_payload, dict):
                return dict(raw_payload)
            if raw_payload in (None, ""):
                return {}
            try:
                payload = json.loads(str(raw_payload))
            except (TypeError, ValueError, json.JSONDecodeError):
                return {}
            return dict(payload) if isinstance(payload, dict) else {}

        @staticmethod
        def _normalize_optional_int(raw_value: Any) -> int | None:
            if raw_value in (None, "", 0, "0"):
                return None
            try:
                normalized_value = int(raw_value)
            except (TypeError, ValueError):
                return None
            return normalized_value if normalized_value > 0 else None

        @classmethod
        def _parser_display_label(cls, parser_id: Any) -> str:
            normalized_parser_id = str(parser_id or "").strip().lower()
            if not normalized_parser_id:
                return ""
            return cls._PARSER_DISPLAY_LABELS.get(normalized_parser_id, normalized_parser_id)

        @staticmethod
        def _dedupe_text_items(items: list[str]) -> list[str]:
            deduped: list[str] = []
            for item in items:
                normalized_item = str(item or "").strip()
                if normalized_item and normalized_item not in deduped:
                    deduped.append(normalized_item)
            return deduped

        @staticmethod
        def _split_description_segments(description: Any) -> list[str]:
            return [
                segment.strip()
                for segment in str(description or "").split("|")
                if segment.strip()
            ]

        @classmethod
        def _merge_description_values(cls, descriptions: list[Any]) -> str:
            segments: list[str] = []
            for description in descriptions:
                for segment in cls._split_description_segments(description):
                    if any(
                        segment == existing_segment
                        or SequenceMatcher(None, segment, existing_segment).ratio() >= 0.92
                        for existing_segment in segments
                    ):
                        continue
                    segments.append(segment)
            return "|".join(segments)

        @classmethod
        def _normalize_tag_ids(cls, raw_value: Any) -> list[int]:
            if raw_value in (None, ""):
                return []

            raw_items: list[Any]
            if isinstance(raw_value, str):
                raw_items = [item.strip() for item in raw_value.split(",") if item.strip()]
            elif isinstance(raw_value, list):
                raw_items = list(raw_value)
            elif isinstance(raw_value, (tuple, set)):
                raw_items = list(raw_value)
            else:
                raw_items = [raw_value]

            tag_ids: list[int] = []
            for item in raw_items:
                raw_id = item.get("id") if isinstance(item, dict) else item
                normalized_id = cls._normalize_optional_int(raw_id)
                if normalized_id is not None and normalized_id not in tag_ids:
                    tag_ids.append(normalized_id)
            return tag_ids

        @classmethod
        def _snapshot_tag_ids(cls, snapshot: dict[str, Any]) -> list[int]:
            tag_ids = cls._normalize_tag_ids(snapshot.get("tag_ids"))
            if tag_ids:
                return tag_ids
            return cls._normalize_tag_ids(snapshot.get("tags"))

        @classmethod
        def _source_label_from_import_snapshot(cls, snapshot: dict[str, Any]) -> str:
            for field_name in ("parser_id", "source", "payment_method"):
                raw_value = snapshot.get(field_name)
                if raw_value in (None, "", 0, "0"):
                    continue
                label = cls._parser_display_label(raw_value)
                if label:
                    return label
            return "导入"

        @staticmethod
        def _infer_bill_flow_role(snapshot: dict[str, Any]) -> str:
            bill_type = str(snapshot.get("type") or "").strip().lower()
            amount = float(snapshot.get("amount") or 0.0)
            if bill_type in {"支出", "expense"} or amount < 0:
                return "outgoing"
            if bill_type in {"收入", "income"} or amount > 0:
                return "incoming"
            return "primary"

        @classmethod
        def _build_reconciliation_projection_signal(
            cls,
            *,
            candidate_type: str,
            base_bill: dict[str, Any],
            import_snapshots: list[dict[str, Any]],
        ) -> dict[str, Any]:
            base_source = {
                "role": cls._infer_bill_flow_role(base_bill),
                "label": "人工",
                "source": "manual",
            }
            import_sources = [
                {
                    "role": cls._infer_bill_flow_role(snapshot),
                    "label": cls._source_label_from_import_snapshot(snapshot),
                    "parser_id": str(snapshot.get("parser_id") or ""),
                    "source": "parser",
                }
                for snapshot in import_snapshots
            ]

            if candidate_type == "transfer":
                sources_by_role: dict[str, list[str]] = {"outgoing": [], "incoming": []}
                for source in import_sources:
                    role = str(source.get("role") or "primary")
                    label = str(source.get("label") or "")
                    if role in sources_by_role and label:
                        sources_by_role[role].append(label)
                base_role = str(base_source.get("role") or "primary")
                if base_role in sources_by_role:
                    sources_by_role[base_role].append("人工")
                else:
                    sources_by_role.setdefault(base_role, []).append("人工")

                side_labels = [
                    "&".join(cls._dedupe_text_items(sources_by_role.get(role, [])))
                    for role in ("outgoing", "incoming")
                    if sources_by_role.get(role)
                ]
                if not side_labels:
                    side_labels = ["人工"]
                signal_label = f"匹配：{'|'.join(side_labels)}"
            else:
                signal_label = "|".join(
                    cls._dedupe_text_items(
                        ["人工", *[str(source.get("label") or "") for source in import_sources]]
                    )
                )

            source_chain = [
                base_source,
                *import_sources,
            ]
            return {
                "signal_label": signal_label,
                "source_chain": source_chain,
            }

        @classmethod
        def _normalize_candidate_type(cls, raw_value: Any) -> str:
            candidate_type = str(raw_value or "").strip().lower()
            if candidate_type not in cls._VALID_RECONCILIATION_TYPES:
                raise ValueError("Invalid reconciliation candidate type")
            return candidate_type

        @classmethod
        def _normalize_candidate_payload(cls, candidate: dict[str, Any]) -> dict[str, Any]:
            candidate_type = cls._normalize_candidate_type(candidate.get("candidate_type"))
            candidate_id = str(candidate.get("candidate_id") or "").strip()
            import_bill_key = str(candidate.get("import_bill_key") or "").strip()
            existing_bill_id = cls._normalize_optional_int(candidate.get("existing_bill_id"))
            if not candidate_id or not import_bill_key or existing_bill_id is None:
                raise ValueError("Invalid reconciliation candidate")

            amount_abs = float(candidate.get("amount_abs") or candidate.get("amount") or 0.0)
            if amount_abs < 0:
                amount_abs = abs(amount_abs)

            group_key = str(candidate.get("group_key") or "").strip()
            if not group_key:
                group_key = (
                    f"{cls._IMPORT_RECONCILIATION_FAMILY}:"
                    f"{candidate_type}:bill:{existing_bill_id}:amount:{amount_abs:.2f}"
                )

            score = float(candidate.get("score") or 0.0)
            if score < 0:
                score = 0.0
            if score > 1:
                score = 1.0

            return {
                "family": cls._IMPORT_RECONCILIATION_FAMILY,
                "candidate_id": candidate_id,
                "candidate_type": candidate_type,
                "session_id": str(candidate.get("session_id") or "").strip() or None,
                "preview_id": cls._normalize_optional_int(candidate.get("preview_id")),
                "import_bill_key": import_bill_key,
                "existing_bill_id": existing_bill_id,
                "group_key": group_key,
                "amount_abs": amount_abs,
                "time_diff_seconds": cls._normalize_optional_int(candidate.get("time_diff_seconds")),
                "score": score,
                "level": str(candidate.get("level") or "").strip(),
                "reason": str(candidate.get("reason") or "").strip(),
                "import_bill_snapshot": (
                    dict(candidate.get("import_bill_snapshot"))
                    if isinstance(candidate.get("import_bill_snapshot"), dict)
                    else {}
                ),
                "existing_bill_snapshot": (
                    dict(candidate.get("existing_bill_snapshot"))
                    if isinstance(candidate.get("existing_bill_snapshot"), dict)
                    else {}
                ),
                "source_payload": (
                    dict(candidate.get("source_payload"))
                    if isinstance(candidate.get("source_payload"), dict)
                    else {}
                ),
            }

        @classmethod
        def _row_to_reconciliation_candidate(cls, row: dict[str, Any]) -> dict[str, Any]:
            group_metadata = cls._json_loads(row.get("group_metadata_json"))
            projection = (
                dict(group_metadata.get("projection"))
                if isinstance(group_metadata.get("projection"), dict)
                else {}
            )
            fallback_signal = cls._build_reconciliation_projection_signal(
                candidate_type=str(row.get("candidate_type") or ""),
                base_bill=cls._json_loads(
                    row.get("existing_bill_snapshot_json")
                ),
                import_snapshots=[
                    cls._json_loads(
                        row.get("import_bill_snapshot_json")
                    )
                ],
            )
            signal_payload = projection or fallback_signal
            return {
                "id": int(row.get("id") or 0),
                "user_id": int(row.get("user_id") or 0),
                "family": str(row.get("family") or ""),
                "candidate_id": str(row.get("candidate_id") or ""),
                "candidate_type": str(row.get("candidate_type") or ""),
                "status": str(row.get("status") or ""),
                "session_id": str(row.get("session_id") or ""),
                "preview_id": cls._normalize_optional_int(
                    row.get("preview_id")
                ),
                "import_bill_key": str(row.get("import_bill_key") or ""),
                "existing_bill_id": int(row.get("existing_bill_id") or 0),
                "group_key": str(row.get("group_key") or ""),
                "amount_abs": float(row.get("amount_abs") or 0.0),
                "time_diff_seconds": cls._normalize_optional_int(
                    row.get("time_diff_seconds")
                ),
                "score": float(row.get("score") or 0.0),
                "level": str(row.get("level") or ""),
                "reason": str(row.get("reason") or ""),
                "import_bill_snapshot": cls._json_loads(
                    row.get("import_bill_snapshot_json")
                ),
                "existing_bill_snapshot": cls._json_loads(
                    row.get("existing_bill_snapshot_json")
                ),
                "source_payload": cls._json_loads(
                    row.get("source_payload_json")
                ),
                "group_id": cls._normalize_optional_int(
                    row.get("group_id")
                ),
                "group_status": str(row.get("group_status") or ""),
                "canonical_bill_id": cls._normalize_optional_int(
                    row.get("canonical_bill_id")
                ),
                "group_metadata": group_metadata,
                "signal_label": str(signal_payload.get("signal_label") or ""),
                "source_chain": (
                    list(signal_payload.get("source_chain") or [])
                    if isinstance(signal_payload.get("source_chain"), list)
                    else []
                ),
                "seen_count": int(row.get("seen_count") or 0),
                "first_seen_at": str(row.get("first_seen_at") or ""),
                "last_seen_at": str(row.get("last_seen_at") or ""),
                "resolved_at": str(row.get("resolved_at") or ""),
                "resolution_event_id": cls._normalize_optional_int(
                    row.get("resolution_event_id")
                ),
                "created_at": str(row.get("created_at") or ""),
                "updated_at": str(row.get("updated_at") or ""),
            }
