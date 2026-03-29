"""Project-wide filesystem constants."""

from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
PACKAGE_DIR = Path(__file__).resolve().parent
STATIC_DIR = PACKAGE_DIR / "static"

DATA_DIR = PROJECT_ROOT / "data"
CONFIG_DIR = DATA_DIR / "config"
LOG_DIR = DATA_DIR / "logs"
UPLOADS_DIR = DATA_DIR / "uploads"
BACKUP_DIR = PROJECT_ROOT / "backup"
OUTPUT_DIR = PROJECT_ROOT / "output"
LEGACY_CONFIG_DIR = PROJECT_ROOT / "config"

TEST_DB_DIR_ENV = "BILL_ANALYSER_TEST_DB_DIR"
