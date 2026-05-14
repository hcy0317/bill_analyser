from __future__ import annotations

from bill_analyser.api.routes import statistics as statistics_module


def test_statistics_flask_sidecar_exports_only_empty_blueprint() -> None:
    """Statistics routes are Rust-owned; Flask keeps only the registration shim."""
    assert statistics_module.__all__ == ["bp"]
    assert statistics_module.bp.name == "statistics"


def test_statistics_runtime_handlers_are_not_exported_from_flask_package() -> None:
    """Rust-owned statistics handlers should not remain monkeypatchable in Flask."""
    deleted_handler_names = [
        "get_overview",
        "get_trends",
        "get_comparison",
        "get_category_analysis",
        "get_trend",
        "get_categorical_analysis",
        "get_trend_analysis",
        "get_asset_trends",
        "get_category_pie",
        "get_top_merchants",
        "get_transaction_amounts",
        "get_exchange_rates",
        "update_user_custom_exchange_rate",
        "delete_user_custom_exchange_rate",
        "_fetch_exchange_rates_from_providers",
        "_get_fallback_exchange_rates",
    ]

    for name in deleted_handler_names:
        assert not hasattr(statistics_module, name), name
