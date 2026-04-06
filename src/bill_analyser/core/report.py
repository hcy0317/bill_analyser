"""Backward-compatible report export facade for the legacy core import path."""

from __future__ import annotations

from typing import TYPE_CHECKING

from ..constants import OUTPUT_DIR
from ..utils.report_export import ReportExporter

if TYPE_CHECKING:
    from pathlib import Path


class ReportGenerator(ReportExporter):
    """Compatibility wrapper that preserves ``bill_analyser.core.report`` imports."""

    def __init__(self, output_dir: str | Path | None = None):
        """Initialize the exporter with the legacy module-level default output dir."""
        super().__init__(output_dir=output_dir or OUTPUT_DIR)

    async def export_report(self, *args, **kwargs):
        """Keep a concrete public method on the compatibility wrapper for lint clarity."""
        return await super().export_report(*args, **kwargs)
