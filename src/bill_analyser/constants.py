"""Project-wide filesystem constants."""

from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2] / "data"
PACKAGE_DIR = Path(__file__).resolve().parent
STATIC_DIR = PACKAGE_DIR / "static"
