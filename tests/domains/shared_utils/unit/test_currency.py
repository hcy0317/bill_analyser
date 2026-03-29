from __future__ import annotations

import pytest

from bill_analyser.utils import currency


@pytest.mark.parametrize(
    ("cents", "expected_yuan"),
    [
        (10050, 100.50),
        ("5000", 50.0),
        (-99, -0.99),
        ("1", 0.01),
    ],
)
def test_cents_to_yuan_converts_supported_inputs(cents: int | str, expected_yuan: float) -> None:
    """分到元转换应保持两位小数精度。"""
    assert currency.cents_to_yuan(cents) == pytest.approx(expected_yuan)


def test_cents_to_yuan_returns_zero_for_empty_values() -> None:
    """空输入应稳定回退为 0 元。"""
    assert currency.cents_to_yuan(None) == 0.0
    assert currency.cents_to_yuan("") == 0.0


def test_cents_to_yuan_logs_warning_for_invalid_string(monkeypatch: pytest.MonkeyPatch) -> None:
    """无法解析的字符串输入应记录告警并返回 0。"""
    class LoggerStub:
        def __init__(self) -> None:
            self.warning_calls = 0

        def warning(self, *_args: object, **_kwargs: object) -> None:
            self.warning_calls += 1

    mock_logger = LoggerStub()
    monkeypatch.setattr(currency, "logger", mock_logger)

    assert currency.cents_to_yuan("not-a-number") == 0.0
    assert mock_logger.warning_calls == 1


@pytest.mark.parametrize(
    ("yuan", "expected_cents"),
    [
        (100.50, 10050),
        ("50.0", 5000),
        (1.235, 124),
        (1.234, 123),
        (-1.235, -124),
    ],
)
def test_yuan_to_cents_rounds_half_up(yuan: float | str, expected_cents: int) -> None:
    """元到分转换应使用四舍五入。"""
    assert currency.yuan_to_cents(yuan) == expected_cents


def test_yuan_to_cents_logs_warning_for_invalid_string(monkeypatch: pytest.MonkeyPatch) -> None:
    """非法元金额应记录告警并返回 0 分。"""
    class LoggerStub:
        def __init__(self) -> None:
            self.warning_calls = 0

        def warning(self, *_args: object, **_kwargs: object) -> None:
            self.warning_calls += 1

    mock_logger = LoggerStub()
    monkeypatch.setattr(currency, "logger", mock_logger)

    assert currency.yuan_to_cents("bad-value") == 0
    assert mock_logger.warning_calls == 1


def test_format_currency_display_supports_known_and_unknown_currency_symbols() -> None:
    """格式化函数应支持内置符号和未知币种前缀。"""
    assert currency.format_currency_display(1234.56) == "¥1,234.56"
    assert currency.format_currency_display(None, "USD") == "$0.00"
    assert currency.format_currency_display(1234.56, "BTC") == "BTC1,234.56"


@pytest.mark.parametrize(
    ("amount", "expected"),
    [
        ("12.34", True),
        (0.01, True),
        (0.009, False),
        (-1, False),
        (1_000_000_000, False),
        (float("nan"), False),
        (float("inf"), False),
        (None, False),
        ("", False),
        ("oops", False),
    ],
)
def test_validate_amount_covers_range_and_non_finite_inputs(amount: float | str | None, expected: bool) -> None:
    """金额验证应拒绝范围外、空值和非有限数。"""
    assert currency.validate_amount(amount) is expected
