"""Empty Flask sidecar blueprint for Rust-owned statistics routes."""

from __future__ import annotations

from flask import Blueprint

bp = Blueprint("statistics", __name__)

__all__ = ["bp"]
