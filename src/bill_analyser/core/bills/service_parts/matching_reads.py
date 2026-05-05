"""Session/formal-bill matching read models and reconcile-history aggregation."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    build_formal_learning_candidate_id,
    build_learning_rule_revision,
    build_matching_session_candidates,
    is_ordinary_bank_interest_income,
    log_method,
    parse_bill_datetime,
)

class MatchingReadsMixin:
    """Session/formal-bill matching read models and reconcile-history aggregation."""

    @log_method
    async def get_matching_session_candidates(self, session_id: str, user_id: int = 1) -> dict[str, Any]:
        """Project import-preview matching payloads into a session-scoped candidate list."""
        preview_items = await self.get_import_preview(session_id, selected_only=False, user_id=user_id)
        return build_matching_session_candidates(session_id, preview_items)

    @log_method
    async def get_matching_bill_candidates(self, bill_id: int, user_id: int = 1) -> dict[str, Any]:
        """Return matching candidates for a persisted historical bill."""
        result = await self.db.get_bill_transfer_candidates(bill_id, user_id=user_id)
        if not result.get("bill"):
            return {"success": False, "error": "Bill not found", "status_code": 404}

        reconciliation_projection = await self._build_reconciliation_projection_for_bill(
            bill_id,
            user_id=user_id,
        )
        if result.get("linked_pair"):
            return {
                "success": True,
                "bill_id": bill_id,
                "linked_pair": result.get("linked_pair"),
                "candidates": [],
                "reconciliation": reconciliation_projection,
            }

        reconciliation_candidates = await self._build_reconciliation_candidates_for_bill(
            bill_id,
            user_id=user_id,
        )
        transfer_candidates = list(result.get("candidates") or [])
        investment_candidates = await self._build_investment_candidates_for_bill(
            dict(result.get("bill") or {}),
            user_id=user_id,
        )
        learning_candidates = await self._build_learning_candidates_for_bill(
            dict(result.get("bill") or {}),
            user_id=user_id,
        )

        return {
            "success": True,
            "bill_id": bill_id,
            "linked_pair": result.get("linked_pair"),
            "candidates": [
                *reconciliation_candidates,
                *transfer_candidates,
                *investment_candidates,
                *learning_candidates,
            ],
            "reconciliation": reconciliation_projection,
        }

    async def _build_reconciliation_projection_for_bill(
        self,
        bill_id: int,
        *,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        projection_handler = getattr(self.db, "get_bill_reconciliation_projection", None)
        if not callable(projection_handler):
            return None
        return await projection_handler(int(bill_id), user_id=user_id)

    async def _build_reconciliation_candidates_for_bill(
        self,
        bill_id: int,
        *,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        list_handler = getattr(self.db, "list_import_reconciliation_candidates", None)
        if not callable(list_handler):
            return []
        raw_candidates = await list_handler(
            user_id=user_id,
            existing_bill_id=int(bill_id),
            status="pending",
            limit=50,
        )
        candidates: list[dict[str, Any]] = []
        for candidate in raw_candidates:
            import_snapshot = dict(candidate.get("import_bill_snapshot") or {})
            signal_label = str(candidate.get("signal_label") or "")
            candidates.append(
                {
                    "candidate_id": str(candidate.get("candidate_id") or ""),
                    "kind": f"reconciliation_{candidate.get('candidate_type') or ''}",
                    "bill_id": None,
                    "score": float(candidate.get("score") or 0.0),
                    "level": str(candidate.get("level") or ""),
                    "reason": str(candidate.get("reason") or ""),
                    "bill": self._build_historical_candidate_bill_snapshot(import_snapshot),
                    "summary": signal_label,
                    "suppressed": False,
                    "status": str(candidate.get("status") or ""),
                    "reconciliation": {
                        "group_id": candidate.get("group_id"),
                        "candidate_type": candidate.get("candidate_type"),
                        "signal_label": signal_label,
                        "source_chain": list(candidate.get("source_chain") or []),
                    },
                }
            )
        return candidates

    @staticmethod
    def _derive_matching_candidate_level(score: float) -> str:
        if score >= 0.8:
            return "high"
        if score >= 0.65:
            return "medium"
        if score > 0:
            return "low"
        return ""

    @staticmethod
    def _build_historical_candidate_bill_snapshot(candidate_bill: dict[str, Any]) -> dict[str, Any]:
        return {
            "id": int(candidate_bill.get("id") or 0),
            "date": str(candidate_bill.get("date") or ""),
            "type": str(candidate_bill.get("type") or ""),
            "amount": float(candidate_bill.get("amount") or 0.0),
            "counterparty": str(candidate_bill.get("counterparty") or ""),
            "description": str(candidate_bill.get("description") or ""),
            "payment_method": str(candidate_bill.get("payment_method") or ""),
            "main_category": str(candidate_bill.get("main_category") or ""),
            "sub_category": str(candidate_bill.get("sub_category") or ""),
            "source_account_id": int(candidate_bill.get("source_account_id") or 0),
            "destination_account_id": int(candidate_bill.get("destination_account_id") or 0),
        }

    @log_method
    async def _build_investment_candidates_for_bill(
        self,
        bill: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        bill_id = int(bill.get("id") or 0)
        if bill_id <= 0:
            return []

        keyword_config = await self._get_investment_keyword_config(user_id)
        if (
            is_ordinary_bank_interest_income(bill, keyword_config=keyword_config)
            or self._classify_investment_pnl_change(bill, keyword_config=keyword_config)
        ):
            return []

        if not self._score_investment_candidate(
            bill,
            allow_existing_investment=True,
            keyword_config=keyword_config,
        ):
            return []

        result = await self.db.get_bill_investment_candidate_bills(bill_id, user_id=user_id)
        raw_candidates = list(result.get("candidates") or [])
        if not raw_candidates:
            return []

        anchor_datetime = parse_bill_datetime(bill.get("date"))
        if anchor_datetime is None:
            return []

        max_window_seconds = 3 * 24 * 60 * 60
        candidates: list[dict[str, Any]] = []
        for candidate_bill in raw_candidates:
            candidate_id = int(candidate_bill.get("id") or 0)
            if candidate_id <= 0 or candidate_id == bill_id:
                continue
            if not self._score_investment_candidate(
                candidate_bill,
                allow_existing_investment=True,
                keyword_config=keyword_config,
            ):
                continue

            candidate_datetime = parse_bill_datetime(candidate_bill.get("date"))
            if candidate_datetime is None:
                continue

            time_diff_seconds = abs((anchor_datetime - candidate_datetime).total_seconds())
            if time_diff_seconds > max_window_seconds:
                continue

            time_score = max(0.0, 1.0 - (time_diff_seconds / max_window_seconds))
            score = round(min(0.86 + (0.12 * time_score), 0.98), 2)
            reason_parts = ["investment_keyword", "opposite_amount", "different_source_account"]
            reason_parts.append("time_close" if time_diff_seconds <= 3600 else "date_window")
            candidates.append(
                {
                    "candidate_id": f"bill:{bill_id}:investment:{candidate_id}",
                    "kind": "investment",
                    "bill_id": candidate_id,
                    "score": score,
                    "level": self._derive_matching_candidate_level(score),
                    "reason": "|".join(reason_parts),
                    "bill": self._build_historical_candidate_bill_snapshot(candidate_bill),
                    "time_diff_seconds": int(time_diff_seconds),
                    "suppressed": False,
                }
            )

        candidates.sort(
            key=lambda candidate: (
                -float(candidate.get("score") or 0.0),
                int(candidate.get("time_diff_seconds") or 0),
                int(candidate.get("bill_id") or 0),
            )
        )
        for candidate in candidates:
            candidate.pop("time_diff_seconds", None)
        return candidates

    @log_method
    async def _build_learning_candidates_for_bill(
        self,
        bill: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        bill_id = int(bill.get("id") or 0)
        if bill_id <= 0:
            return []

        user = await self.db.get_user_by_id(user_id)
        if not user or not bool(user.get("import_learning_enabled", 1)):
            return []

        learning_rules = await self.db.get_import_learning_rules(
            user_id=user_id,
            enabled_only=True,
            limit=None,
        )
        composite_learning_rules = [
            rule for rule in learning_rules if rule.get("match_type") == "composite" and rule.get("match_features_json")
        ]
        if not composite_learning_rules:
            return []

        suppressed_rule_created_at_map = await self.db._get_bill_learning_rule_suppression_created_at_map(  # pylint: disable=protected-access
            bill_id,
            user_id=user_id,
        )
        learning_categories = await self.db.get_all_categories(user_id=user_id)
        learning_accounts = await self.db.get_all_accounts(user_id=user_id)
        learning_categories_by_id = {
            int(category["id"]): category for category in learning_categories if category.get("id") is not None
        }
        learning_accounts_by_id = {
            int(account["id"]): account for account in learning_accounts if account.get("id") is not None
        }

        bill_features = self.db.build_composite_match_features(
            parser_id="",
            counterparty=str(bill.get("counterparty") or ""),
            description=str(bill.get("description") or ""),
            payment_method=str(bill.get("payment_method") or ""),
        )
        if not bill_features:
            return []

        candidates: list[dict[str, Any]] = []
        for rule in composite_learning_rules:
            rule_id = int(rule.get("id") or 0)
            if rule_id <= 0:
                continue

            suppression_created_at = str(suppressed_rule_created_at_map.get(rule_id) or "")
            rule_revision = build_learning_rule_revision(rule)
            if suppression_created_at and suppression_created_at == rule_revision:
                continue

            rule_features = self._deserialize_learning_match_features(rule)
            if not rule_features:
                continue

            score_payload = self._score_learning_rule_similarity(bill_features, rule_features)
            if not score_payload:
                continue

            score = float(score_payload["score"])
            if score < 0.72:
                continue

            if score >= 0.9:
                level = "high"
            elif score >= 0.82:
                level = "medium"
            else:
                level = "low"

            candidates.append(
                {
                    "candidate_id": build_formal_learning_candidate_id(bill_id, rule_id, rule_revision),
                    "kind": "learning",
                    "rule_id": rule_id,
                    "score": score,
                    "level": level,
                    "reason": ", ".join(score_payload["reason_parts"]),
                    "recommended_type": str(rule.get("learned_type") or "").strip(),
                    "summary": self._build_learning_rule_result_summary(
                        rule,
                        learning_categories_by_id,
                        learning_accounts_by_id,
                    ),
                    "suppressed": False,
                }
            )

        candidates.sort(
            key=lambda candidate: (
                -float(candidate.get("score") or 0.0),
                -int(candidate.get("rule_id") or 0),
            )
        )
        return candidates

    @staticmethod
    def _coerce_matching_history_bill_id(raw_bill_id: Any) -> int:
        if isinstance(raw_bill_id, bool):
            raise ValueError("Invalid billIds")

        if isinstance(raw_bill_id, int):
            normalized_bill_id = raw_bill_id
        elif isinstance(raw_bill_id, str):
            normalized_raw_bill_id = raw_bill_id.strip()
            if not normalized_raw_bill_id.isdigit():
                raise ValueError("Invalid billIds")
            normalized_bill_id = int(normalized_raw_bill_id)
        else:
            raise ValueError("Invalid billIds")

        if normalized_bill_id <= 0:
            raise ValueError("Invalid billIds")

        return normalized_bill_id

    @classmethod
    def _normalize_matching_history_bill_ids(cls, raw_bill_ids: list[int] | None) -> list[int]:
        if raw_bill_ids is None or not isinstance(raw_bill_ids, list):
            raise ValueError("Invalid billIds")

        normalized_bill_ids: list[int] = []
        seen_bill_ids: set[int] = set()

        for raw_bill_id in raw_bill_ids:
            normalized_bill_id = cls._coerce_matching_history_bill_id(raw_bill_id)
            if normalized_bill_id in seen_bill_ids:
                continue
            seen_bill_ids.add(normalized_bill_id)
            normalized_bill_ids.append(normalized_bill_id)

        return normalized_bill_ids

    @staticmethod
    def _build_matching_history_pair_key(linked_pair: dict[str, Any]) -> tuple[Any, ...] | None:
        pair_id = linked_pair.get("id")
        if pair_id not in (None, ""):
            return ("id", int(pair_id))

        left_bill_id = linked_pair.get("left_bill_id")
        right_bill_id = linked_pair.get("right_bill_id")
        if left_bill_id not in (None, "") and right_bill_id not in (None, ""):
            return (
                "bills",
                min(int(left_bill_id), int(right_bill_id)),
                max(int(left_bill_id), int(right_bill_id)),
            )

        return None

    _RECONCILE_ALLOWED_FAMILIES: set[str] = {"transfer", "investment", "learning"}

    @log_method
    async def reconcile_matching_history(
        self,
        bill_ids: list[int],
        user_id: int = 1,
        families: list[str] | None = None,
    ) -> dict[str, Any]:
        """Aggregate matching candidates for explicit historical bill anchors.

        *families* controls which candidate kinds are included:
        ``['transfer']`` (default for backward-compat),
        ``['transfer', 'investment', 'learning']`` for multi-family.
        """
        try:
            normalized_bill_ids = self._normalize_matching_history_bill_ids(bill_ids)
        except (TypeError, ValueError):
            return {"success": False, "error": "Invalid billIds", "status_code": 400}

        if not normalized_bill_ids:
            return {
                "success": False,
                "error": "billIds must be a non-empty list",
                "status_code": 400,
            }

        allowed_families = self._normalize_reconcile_families(families)

        results: list[dict[str, Any]] = []
        candidate_count = 0
        linked_pair_keys: set[tuple[Any, ...]] = set()

        for bill_id in normalized_bill_ids:
            result = await self.get_matching_bill_candidates(bill_id, user_id=user_id)
            if not result.get("success"):
                return {
                    "success": False,
                    "error": str(result.get("error") or "Bill not found"),
                    "status_code": int(result.get("status_code", 404) or 404),
                    "bill_id": bill_id,
                }

            raw_linked_pair = result.get("linked_pair")
            linked_pair = (
                raw_linked_pair
                if isinstance(raw_linked_pair, dict)
                and str(raw_linked_pair.get("pair_type") or "transfer") in allowed_families
                else None
            )
            candidates = [
                candidate
                for candidate in list(result.get("candidates") or [])
                if str(candidate.get("kind") or "transfer") in allowed_families
            ]
            results.append(
                {
                    "bill_id": int(result.get("bill_id") or bill_id),
                    "linked_pair": linked_pair,
                    "candidates": candidates,
                }
            )
            candidate_count += len(candidates)
            if isinstance(linked_pair, dict):
                pair_key = self._build_matching_history_pair_key(linked_pair)
                if pair_key is not None:
                    linked_pair_keys.add(pair_key)

        return {
            "success": True,
            "results": results,
            "summary": {
                "bill_count": len(results),
                "candidate_count": candidate_count,
                "linked_pair_count": len(linked_pair_keys),
            },
        }

    def _normalize_reconcile_families(self, families: list[str] | None) -> set[str]:
        """Return the validated set of candidate families for reconcile-history."""
        if families is None:
            return {"transfer"}
        if not isinstance(families, list):
            return {"transfer"}
        normalized = {
            str(f).strip().lower()
            for f in families
            if isinstance(f, str) and str(f).strip().lower() in self._RECONCILE_ALLOWED_FAMILIES
        }
        return normalized if normalized else {"transfer"}

    @log_method
    async def get_matching_pairs(self, user_id: int = 1) -> dict[str, Any]:
        """Return persisted manual bill pairs for the current user."""
        pairs = await self.db.list_manual_transfer_pairs(user_id=user_id)
        return {"success": True, "pairs": pairs}

    @log_method
    async def get_matching_bill_feedback(self, bill_id: int, user_id: int = 1) -> dict[str, Any]:
        """Return append-only formal-bill matching feedback events for a persisted bill."""
        bill = await self.db.get_bill_by_id(int(bill_id), user_id=user_id)
        if not bill:
            return {"success": False, "error": "Bill not found", "status_code": 404}

        events = await self.db.list_bill_matching_feedback_for_bill(int(bill_id), user_id=user_id)
        return {
            "success": True,
            "bill_id": int(bill_id),
            "events": events,
        }
