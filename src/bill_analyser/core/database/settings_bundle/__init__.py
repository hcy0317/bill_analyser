"""Unified JSON settings bundle import/export helpers."""

from __future__ import annotations

from bill_analyser.core.database.shared import DatabaseFacadeBase
from .shared import LOCAL_REF_NAMESPACE
from .shared import SETTINGS_BUNDLE_SECTION_KEYS
from .shared import OCR_CONFIG_SETTING_KEY
from .shared import SETTINGS_BUNDLE_SCHEMA_VERSION
from .base import SettingsBundleBaseMixin
from .exporters import SettingsBundleExportersMixin
from .accounts_categories_tags import SettingsBundleAccountsCategoriesTagsMixin
from .templates import SettingsBundleTemplatesMixin
from .resolution import SettingsBundleResolutionMixin
from .rules_llm_ocr import SettingsBundleRulesLlmOcrMixin


class DatabaseSettingsBundleMixin(  # pylint: disable=too-many-ancestors
    SettingsBundleBaseMixin,
    SettingsBundleExportersMixin,
    SettingsBundleAccountsCategoriesTagsMixin,
    SettingsBundleTemplatesMixin,
    SettingsBundleResolutionMixin,
    SettingsBundleRulesLlmOcrMixin,
    DatabaseFacadeBase,
):
    """Compatibility facade composed from db_settings_bundle persistence parts."""


__all__ = [
    "DatabaseSettingsBundleMixin",
    "SETTINGS_BUNDLE_SCHEMA_VERSION",
    "OCR_CONFIG_SETTING_KEY",
    "SETTINGS_BUNDLE_SECTION_KEYS",
    "LOCAL_REF_NAMESPACE",
]
