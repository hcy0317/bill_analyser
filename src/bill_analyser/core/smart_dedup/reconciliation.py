"""Import-to-existing reconciliation candidate detection."""
# pylint: disable=broad-exception-caught,duplicate-code,line-too-long,too-many-arguments
# pylint: disable=too-many-branches,too-many-locals

import hashlib
from collections import defaultdict
from typing import Any

from ...utils.logger import log_method


# pylint: disable=too-few-public-methods


class ReconciliationCandidateMixin:

    """ReconciliationCandidateMixin implementation shard."""



    def _build_transfer_source_snapshot(self, bill: dict[str, Any], *, role: str) -> dict[str, Any]:
        """Capture explicit transfer-side source metadata for downstream matching."""
        parser_id = str(bill.get("_parser_id", "") or "").strip()
        return {
            "role": role,
            "parser_id": parser_id,
            "payment_method": str(bill.get("payment_method", "") or "").strip(),
            "counterparty": str(bill.get("counterparty", "") or "").strip(),
            "source_account_id": bill.get("source_account_id"),
            "account_name": (
                bill.get("account_name")
                or bill.get("source_account_name")
                or bill.get("account")
                or bill.get("payment_method", "")
            ),
            "tags": self._merge_parser_tags(bill),
        }

    def _get_reconciliation_source_token(self, bill: dict[str, Any]) -> str:
        """Return the best stable source token for import reconciliation comparisons."""
        for field_name in ("_parser_id", "source", "source_account_id", "payment_method"):
            raw_value = bill.get(field_name)
            if raw_value in (None, "", 0, "0"):
                continue
            return str(raw_value).strip().lower()
        return ""

    def _build_reconciliation_import_key(self, bill: dict[str, Any]) -> str:
        """Build a stable import-side key without requiring a persisted preview id."""
        preview_id = bill.get("preview_id") or bill.get("_preview_id") or bill.get("id")
        if preview_id not in (None, "", 0, "0"):
            return f"preview:{int(preview_id)}"

        session_id = str(
            bill.get("session_id")
            or bill.get("_session_id")
            or bill.get("import_session_id")
            or ""
        ).strip()
        template_id = (
            bill.get("_template_id")
            or bill.get("template_id")
            or bill.get("parser_template_id")
        )
        if session_id and template_id not in (None, "", 0, "0"):
            return f"session:{session_id}:template:{int(template_id)}"

        if bill.get("_dedup_id"):
            return f"dedup:{bill['_dedup_id']}"

        key_fields = [
            str(bill.get("date", "")),
            str(bill.get("amount", "")),
            str(bill.get("counterparty", "")),
            str(bill.get("description", "")),
            str(bill.get("_parser_id", "") or bill.get("source", "")),
            str(bill.get("source_account_id", "")),
        ]
        digest = hashlib.sha1("|".join(key_fields).encode("utf-8")).hexdigest()[:16]
        return f"hash:{digest}"

    def _build_reconciliation_candidate_id(
        self,
        *,
        candidate_type: str,
        existing_bill_id: Any,
        import_bill_key: str,
    ) -> str:
        safe_import_key = hashlib.sha1(import_bill_key.encode("utf-8")).hexdigest()[:16]
        return f"reconcile:import:{candidate_type}:bill:{int(existing_bill_id)}:{safe_import_key}"

    def _build_reconciliation_bill_snapshot(self, bill: dict[str, Any], *, imported: bool) -> dict[str, Any]:
        """Capture the minimum auditable bill state for a reconciliation candidate."""
        snapshot = {
            "id": bill.get("id") if not imported else None,
            "date": str(bill.get("date") or ""),
            "type": str(bill.get("type") or ""),
            "amount": float(bill.get("amount") or 0.0),
            "counterparty": str(bill.get("counterparty") or ""),
            "description": str(bill.get("description") or ""),
            "payment_method": str(bill.get("payment_method") or ""),
            "main_category": str(bill.get("main_category") or ""),
            "sub_category": str(bill.get("sub_category") or ""),
            "source_account_id": bill.get("source_account_id") or 0,
            "destination_account_id": bill.get("destination_account_id") or 0,
        }
        if imported:
            session_id = str(
                bill.get("session_id")
                or bill.get("_session_id")
                or bill.get("import_session_id")
                or ""
            ).strip()
            template_id = (
                bill.get("_template_id")
                or bill.get("template_id")
                or bill.get("parser_template_id")
            )
            if session_id:
                snapshot["session_id"] = session_id
            if template_id not in (None, "", 0, "0"):
                snapshot["template_id"] = int(template_id)
            snapshot["parser_id"] = str(bill.get("_parser_id") or bill.get("source") or "")
            snapshot["parser_tags"] = self._merge_parser_tags(bill)
        return snapshot

    def _resolve_reconciliation_candidate_type(
        self,
        imported_bill: dict[str, Any],
        existing_bill: dict[str, Any],
    ) -> tuple[str, str] | None:
        """Classify an import-to-existing pair as transfer or duplicate."""
        imported_amount = float(imported_bill.get("amount") or 0.0)
        existing_amount = float(existing_bill.get("amount") or 0.0)
        if abs(abs(imported_amount) - abs(existing_amount)) > self.AMOUNT_TOLERANCE:
            return None

        imported_has_transfer_intent = self._has_transfer_intent_keywords(imported_bill)
        existing_has_transfer_intent = self._has_transfer_intent_keywords(existing_bill)
        duplicate_evidence = self._has_reconciliation_duplicate_evidence(imported_bill, existing_bill)

        if self._amount_opposite(imported_amount, existing_amount):
            if duplicate_evidence and not (imported_has_transfer_intent or existing_has_transfer_intent):
                return "duplicate", "opposite_amount|duplicate_text_evidence"

            imported_source = self._get_reconciliation_source_token(imported_bill)
            existing_source = self._get_reconciliation_source_token(existing_bill)
            if imported_source and existing_source and imported_source == existing_source:
                return None
            return "transfer", "opposite_amount|same_day|time_close"

        if self._amount_equal_same_direction(imported_amount, existing_amount):
            reason = "same_amount|same_direction|same_day|time_close"
            if duplicate_evidence:
                reason += "|duplicate_text_evidence"
            return "duplicate", reason

        return None

    def _has_reconciliation_duplicate_evidence(
        self,
        imported_bill: dict[str, Any],
        existing_bill: dict[str, Any],
    ) -> bool:
        """Return whether two same-day amount-matched bills look duplicated."""
        comparable_fields = ["counterparty", "description", "payment_method"]
        for field_name in comparable_fields:
            left = str(imported_bill.get(field_name, "") or "").strip()
            right = str(existing_bill.get(field_name, "") or "").strip()
            if not left or not right:
                continue
            if left in right or right in left:
                return True
            if self._calculate_similarity(left, right) >= self.SIMILARITY_THRESHOLD:
                return True

        imported_text = self._bill_text_for_intent(imported_bill)
        existing_text = self._bill_text_for_intent(existing_bill)
        if not imported_text and not existing_text:
            return True
        return bool(
            imported_text
            and existing_text
            and self._calculate_similarity(imported_text, existing_text) >= 0.62
        )

    @log_method
    async def find_import_reconciliation_candidates(
        self,
        bills: list[dict[str, Any]],
        db,
        user_id: int = 1,
        *,
        persist: bool = True,
        time_tolerance_seconds: int | None = None,
    ) -> list[dict[str, Any]]:
        """Find non-destructive same-day import-to-existing reconciliation candidates."""
        active_bills = [bill for bill in bills if not bill.get("_skip_reconciliation_candidate", False)]
        if not active_bills:
            return []

        tolerance_seconds = int(time_tolerance_seconds or self.TIME_TOLERANCE)
        bills_by_day: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for bill in active_bills:
            bill_dt = self._parse_datetime(str(bill.get("date", "") or ""))
            if bill_dt is None:
                continue
            bills_by_day[bill_dt.strftime("%Y-%m-%d")].append(bill)

        if not bills_by_day:
            return []

        candidates: list[dict[str, Any]] = []
        matched_pairs: set[tuple[str, int, str]] = set()
        for day_key, day_bills in sorted(bills_by_day.items()):
            try:
                existing_bills = await db.get_bills_by_date_range(day_key, day_key, user_id=user_id)
            except Exception as exc:
                self.logger.error("[导入后匹配] 查询同日已有账单失败: %s", exc)
                continue

            if not existing_bills:
                continue

            for imported_bill in day_bills:
                imported_dt = self._parse_datetime(str(imported_bill.get("date", "") or ""))
                if imported_dt is None:
                    continue
                import_bill_key = self._build_reconciliation_import_key(imported_bill)
                session_id = str(
                    imported_bill.get("session_id")
                    or imported_bill.get("_session_id")
                    or imported_bill.get("import_session_id")
                    or ""
                ).strip()
                preview_id = imported_bill.get("preview_id") or imported_bill.get("_preview_id")
                for existing_bill in existing_bills:
                    existing_id = int(existing_bill.get("id") or 0)
                    if existing_id <= 0:
                        continue
                    existing_dt = self._parse_datetime(str(existing_bill.get("date", "") or ""))
                    if existing_dt is None or existing_dt.strftime("%Y-%m-%d") != day_key:
                        continue
                    time_diff_seconds = abs((imported_dt - existing_dt).total_seconds())
                    if time_diff_seconds > tolerance_seconds:
                        continue

                    classification = self._resolve_reconciliation_candidate_type(imported_bill, existing_bill)
                    if classification is None:
                        continue
                    candidate_type, reason = classification
                    pair_key = (candidate_type, existing_id, import_bill_key)
                    if pair_key in matched_pairs:
                        continue
                    matched_pairs.add(pair_key)

                    amount_abs = abs(float(imported_bill.get("amount") or 0.0))
                    candidate_id = self._build_reconciliation_candidate_id(
                        candidate_type=candidate_type,
                        existing_bill_id=existing_id,
                        import_bill_key=import_bill_key,
                    )
                    group_key = f"import_reconciliation:{candidate_type}:bill:{existing_id}:amount:{amount_abs:.2f}:{day_key}"
                    time_score = max(0.0, 1.0 - (time_diff_seconds / max(tolerance_seconds, 1)))
                    candidates.append(
                        {
                            "candidate_id": candidate_id,
                            "candidate_type": candidate_type,
                            "session_id": session_id or None,
                            "preview_id": preview_id,
                            "import_bill_key": import_bill_key,
                            "existing_bill_id": existing_id,
                            "group_key": group_key,
                            "amount_abs": amount_abs,
                            "time_diff_seconds": int(time_diff_seconds),
                            "score": round(0.8 + (0.19 * time_score), 2),
                            "level": "high" if time_diff_seconds <= self.TIME_TOLERANCE else "medium",
                            "reason": reason,
                            "import_bill_snapshot": self._build_reconciliation_bill_snapshot(
                                imported_bill,
                                imported=True,
                            ),
                            "existing_bill_snapshot": self._build_reconciliation_bill_snapshot(
                                existing_bill,
                                imported=False,
                            ),
                            "source_payload": {
                                "family": "import_reconciliation",
                                "same_day": day_key,
                                "time_tolerance_seconds": tolerance_seconds,
                            },
                        }
                    )

        persist_handler = getattr(db, "persist_import_reconciliation_candidates", None)
        if persist and candidates and callable(persist_handler):
            return await persist_handler(candidates, user_id=user_id)
        return candidates
