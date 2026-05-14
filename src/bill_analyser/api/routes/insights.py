"""Empty Flask sidecar blueprint for Rust-owned insights routes."""

from __future__ import annotations

from flask import Blueprint

bp = Blueprint("insights", __name__)

__all__ = ["bp"]
