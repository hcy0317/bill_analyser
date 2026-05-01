"""Request normalization helpers shared by import-session LLM flows."""
# pylint: disable=too-few-public-methods

from __future__ import annotations

from typing import Any

from .errors import LLMImportSessionAnalysisError
from .limits import _MAX_PREVIEW_UPDATE_BATCH, _MAX_SESSION_SELECTION


class PreviewInputNormalizationMixin:
    """Validate and normalize preview ids and draft updates."""

    @staticmethod
    def _normalize_selected_preview_ids(
        *,
        preview_ids: list[int] | None,
        preview_updates: list[dict[str, Any]] | None,
    ) -> list[int]:
        """Build a stable ordered preview-id list from request inputs."""
        ordered_ids: list[int] = []

        if preview_ids is not None:
            if not isinstance(preview_ids, list):
                raise ValueError("preview_ids must be a list")
            if len(preview_ids) > _MAX_SESSION_SELECTION:
                raise LLMImportSessionAnalysisError(
                    "PREVIEW_SELECTION_TOO_LARGE",
                    (
                        "Too many preview rows selected for session analysis "
                        f"(max {_MAX_SESSION_SELECTION})"
                    )
                )
            for preview_id in preview_ids:
                normalized_preview_id = int(preview_id)
                if normalized_preview_id > 0 and normalized_preview_id not in ordered_ids:
                    ordered_ids.append(normalized_preview_id)
        elif preview_updates is not None:
            for item in preview_updates:
                preview_id = int(item.get("id") or 0)
                if preview_id > 0 and preview_id not in ordered_ids:
                    ordered_ids.append(preview_id)

        if len(ordered_ids) > _MAX_SESSION_SELECTION:
            raise LLMImportSessionAnalysisError(
                "PREVIEW_SELECTION_TOO_LARGE",
                (
                    "Too many preview rows selected for session analysis "
                    f"(max {_MAX_SESSION_SELECTION})"
                )
            )

        return ordered_ids

    @staticmethod
    def _normalize_preview_updates(
        preview_updates: list[dict[str, Any]] | None,
    ) -> list[dict[str, Any]] | None:
        """Validate, bound, and deduplicate preview updates by preview id."""
        if preview_updates is None:
            return None
        if not isinstance(preview_updates, list):
            raise ValueError("preview_updates must be a list")
        if len(preview_updates) > _MAX_PREVIEW_UPDATE_BATCH:
            raise LLMImportSessionAnalysisError(
                "PREVIEW_SELECTION_TOO_LARGE",
                (
                    "Too many preview updates submitted for session analysis "
                    f"(max {_MAX_PREVIEW_UPDATE_BATCH})"
                )
            )

        deduped_updates: dict[int, dict[str, Any]] = {}
        preview_order: list[int] = []

        for update_item in preview_updates:
            if not isinstance(update_item, dict):
                raise ValueError("preview_updates entries must be objects")

            preview_id = int(update_item.get("id") or 0)
            if preview_id <= 0:
                continue

            if preview_id not in preview_order:
                preview_order.append(preview_id)

            normalized_update_item = dict(update_item)
            normalized_update_item["id"] = preview_id
            deduped_updates[preview_id] = normalized_update_item

        return [deduped_updates[preview_id] for preview_id in preview_order]
