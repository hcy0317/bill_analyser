"""Import preview lightweight filter index, sorting, and paged read API."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions,missing-function-docstring

from .common import (
    Any,
    log_method,
)

class ImportPreviewPagingMixin:
    """Import preview lightweight filter index, sorting, and paged read API."""

    @classmethod
    def _build_import_preview_filter_index_item(
        cls,
        preview_item: dict[str, Any],
        *,
        categories_by_id: dict[int, dict[str, Any]],
        accounts_by_id: dict[int, dict[str, Any]],
    ) -> dict[str, Any]:
        preview_type = str(preview_item.get("preview_type") or "")
        frontend_type = cls._map_import_preview_type_to_frontend_value(preview_type)
        category_id = preview_item.get("category_id")
        normalized_category_id = str(category_id) if category_id not in (None, "", 0, "0") else ""
        category_row = categories_by_id.get(int(category_id)) if normalized_category_id and str(category_id).isdigit() else None
        actual_category_name = (
            str(
                category_row.get("name")
                or category_row.get("sub_category")
                or category_row.get("main_category")
                or ""
            )
            if category_row
            else str(preview_item.get("preview_sub_category") or preview_item.get("preview_main_category") or "")
        )

        source_account_id = preview_item.get("preview_source_account_id")
        normalized_source_account_id = (
            str(source_account_id) if source_account_id not in (None, "", 0, "0") else ""
        )
        source_account_row = accounts_by_id.get(int(source_account_id)) if normalized_source_account_id else None
        actual_source_account_name = (
            str(source_account_row.get("name") or "")
            if source_account_row
            else str(preview_item.get("preview_payment_method") or "")
        )

        destination_account_id = preview_item.get("preview_destination_account_id")
        normalized_destination_account_id = (
            str(destination_account_id) if destination_account_id not in (None, "", 0, "0") else ""
        )
        destination_account_row = accounts_by_id.get(int(destination_account_id)) if normalized_destination_account_id else None
        actual_destination_account_name = (
            str(destination_account_row.get("name") or "")
            if destination_account_row
            else ""
        )
        raw_dedup_source_ids = preview_item.get("dedup_source_ids") or []
        if isinstance(raw_dedup_source_ids, str):
            dedup_source_ids = [
                int(value.strip()) if value.strip().isdigit() else value.strip()
                for value in raw_dedup_source_ids.split(",")
                if value.strip()
            ]
        else:
            dedup_source_ids = list(raw_dedup_source_ids)

        return {
            "id": int(preview_item.get("id") or 0),
            "preview_date": str(preview_item.get("preview_date") or ""),
            "type": frontend_type,
            "source_amount": float(preview_item.get("preview_amount") or 0),
            "category_id": normalized_category_id,
            "actual_category_name": actual_category_name,
            "source_account_id": normalized_source_account_id,
            "destination_account_id": normalized_destination_account_id,
            "actual_source_account_name": actual_source_account_name,
            "actual_destination_account_name": actual_destination_account_name,
            "comment": str(preview_item.get("preview_description") or ""),
            "counterparty": str(preview_item.get("preview_counterparty") or ""),
            "payment_method": str(preview_item.get("preview_payment_method") or ""),
            "selected": bool(preview_item.get("preview_selected", True)),
            "is_manually_annotated": bool(preview_item.get("preview_is_manually_annotated")),
            "parser_source": str(preview_item.get("preview_parser_id") or ""),
            "parser_tags": list(preview_item.get("preview_parser_tags") or []),
            "dedup_type": str(preview_item.get("dedup_type") or ""),
            "dedup_source_ids": dedup_source_ids,
            "transfer_status": cls._resolve_import_preview_transfer_signal_status(preview_item),
            "transfer_title": str(preview_item.get("transfer_suggestion_reason") or ""),
            "learning_status": cls._resolve_import_preview_learning_signal_status(preview_item),
            "learning_title": str(preview_item.get("learning_recommendation_reason") or ""),
            "learning_summary": str(preview_item.get("learning_recommendation_summary") or ""),
            "learning_mode": str(preview_item.get("learning_recommendation_mode") or ""),
            "recurring_template_id": (
                str(preview_item.get("preview_recurring_id") or "")
                if preview_item.get("preview_recurring_id") not in (None, "", 0, "0")
                else ""
            ),
            "recurring_candidate_count": int(preview_item.get("preview_recurring_candidate_count") or 0),
            "recurring_match_reasons": str(preview_item.get("preview_recurring_match_reasons") or ""),
            "recurring_matched_date": str(preview_item.get("preview_recurring_matched_date") or ""),
        }

    @log_method
    async def get_import_preview_filter_index(
        self,
        session_id: str,
        *,
        selected_only: bool = False,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        previews = await self.db.get_preview_by_session(
            session_id,
            user_id=user_id,
            selected_only=selected_only,
        )
        if not previews:
            return []

        preview_user_id = int(previews[0].get("user_id") or user_id or 1)
        projection_context = await self._load_import_preview_projection_context(
            session_id,
            user_id=preview_user_id,
        )
        categories = await self.db.get_all_categories(user_id=preview_user_id)
        accounts = await self.db.get_all_accounts(user_id=preview_user_id)
        categories_by_id = {
            int(category["id"]): category
            for category in categories
            if category.get("id") not in (None, "", 0, "0")
        }
        accounts_by_id = {
            int(account["id"]): account
            for account in accounts
            if account.get("id") not in (None, "", 0, "0")
        }

        result: list[dict[str, Any]] = []
        for preview in previews:
            preview_item = await self._build_import_preview_item(
                preview,
                user_id=preview_user_id,
                projection_context=projection_context,
            )
            result.append(
                self._build_import_preview_filter_index_item(
                    preview_item,
                    categories_by_id=categories_by_id,
                    accounts_by_id=accounts_by_id,
                )
            )
        return result

    @staticmethod
    def _normalize_import_preview_page_sort_direction(sort_direction: str | None) -> str:
        return "desc" if str(sort_direction or "").strip().lower() == "desc" else "asc"

    @staticmethod
    def _normalize_import_preview_page_sort_key(sort_by: str | None) -> str:
        normalized_sort_by = str(sort_by or "").strip()
        if normalized_sort_by in {"time", "type", "sourceAmount", "counterparty", "paymentMethod", "comment"}:
            return normalized_sort_by
        return ""

    @classmethod
    def _sort_import_preview_page_items(
        cls,
        items: list[dict[str, Any]],
        *,
        sort_by: str | None,
        sort_direction: str | None,
    ) -> list[dict[str, Any]]:
        normalized_sort_by = cls._normalize_import_preview_page_sort_key(sort_by)
        if not normalized_sort_by:
            return list(items)

        normalized_sort_direction = cls._normalize_import_preview_page_sort_direction(sort_direction)
        sort_field_mapping = {
            "time": "preview_date",
            "type": "preview_type",
            "sourceAmount": "preview_amount",
            "counterparty": "preview_counterparty",
            "paymentMethod": "preview_payment_method",
            "comment": "preview_description",
        }
        sort_field = sort_field_mapping[normalized_sort_by]
        stabilized_items = sorted(items, key=lambda item: int(item.get("id") or 0))

        if sort_field == "preview_amount":
            def amount_key(item: dict[str, Any]) -> float:
                try:
                    return float(item.get(sort_field) or 0)
                except (TypeError, ValueError):
                    return 0.0

            return sorted(
                stabilized_items,
                key=amount_key,
                reverse=normalized_sort_direction == "desc",
            )

        return sorted(
            stabilized_items,
            key=lambda item: str(item.get(sort_field) or "").casefold(),
            reverse=normalized_sort_direction == "desc",
        )

    @log_method
    async def get_import_preview_page(
        self,
        session_id: str,
        *,
        page: int = 1,
        page_size: int = 50,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        preview_ids: list[int] | None = None,
        selected_only: bool = False,
        user_id: int = 1,
    ) -> dict[str, Any]:
        normalized_page = max(int(page or 1), 1)
        normalized_page_size = max(min(int(page_size or 50), 200), 1)
        normalized_sort_by = self._normalize_import_preview_page_sort_key(sort_by)
        normalized_sort_direction = self._normalize_import_preview_page_sort_direction(sort_direction)
        normalized_preview_ids = []
        if preview_ids:
            for preview_id in preview_ids:
                normalized_preview_id = int(preview_id or 0)
                if normalized_preview_id > 0 and normalized_preview_id not in normalized_preview_ids:
                    normalized_preview_ids.append(normalized_preview_id)

        if normalized_preview_ids and hasattr(self.db, "get_preview_by_ids"):
            previews = await self.db.get_preview_by_ids(
                session_id,
                normalized_preview_ids,
                user_id=user_id,
            )
            total = len(normalized_preview_ids)
        elif normalized_preview_ids:
            all_previews = await self.db.get_preview_by_session(
                session_id,
                user_id=user_id,
                selected_only=selected_only,
            )
            preview_lookup = {
                int(preview.get("id") or 0): preview
                for preview in all_previews
                if int(preview.get("id") or 0) > 0
            }
            previews = [
                preview_lookup[preview_id]
                for preview_id in normalized_preview_ids
                if preview_id in preview_lookup
            ]
            total = len(normalized_preview_ids)
        elif hasattr(self.db, "get_preview_page_by_session"):
            previews, total = await self.db.get_preview_page_by_session(
                session_id,
                user_id=user_id,
                page=normalized_page,
                page_size=normalized_page_size,
                sort_by=normalized_sort_by,
                sort_direction=normalized_sort_direction,
                selected_only=selected_only,
            )
        else:
            previews = await self.get_import_preview(session_id, selected_only=selected_only, user_id=user_id)
            previews = self._sort_import_preview_page_items(
                previews,
                sort_by=normalized_sort_by,
                sort_direction=normalized_sort_direction,
            )
            total = len(previews)
            start = (normalized_page - 1) * normalized_page_size
            end = start + normalized_page_size
            return {
                "preview": previews[start:end],
                "total": total,
                "page": normalized_page,
                "page_size": normalized_page_size,
            }

        if not previews:
            return {
                "preview": [],
                "total": total,
                "page": normalized_page,
                "page_size": normalized_page_size,
            }

        preview_user_id = int(previews[0].get("user_id") or user_id or 1)
        projection_context = await self._load_import_preview_projection_context(
            session_id,
            user_id=preview_user_id,
        )

        preview_items: list[dict[str, Any]] = []
        for preview in previews:
            preview_items.append(
                await self._build_import_preview_item(
                    preview,
                    user_id=int(preview.get("user_id") or preview_user_id),
                    projection_context=projection_context,
                )
            )

        return {
            "preview": preview_items,
            "total": total,
            "page": normalized_page,
            "page_size": normalized_page_size,
        }
