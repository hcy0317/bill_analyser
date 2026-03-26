"""Transaction adapter time conversion regressions."""

from datetime import datetime

from src.api.adapters.transaction_adapter import TransactionAdapter


def test_frontend_to_backend_accepts_unix_seconds() -> None:
    """The desktop client sends unix seconds for transaction time."""
    adapter = TransactionAdapter()
    unix_seconds = int(datetime(2025, 1, 1, 0, 0, 0).timestamp())

    backend_data, _ = adapter.frontend_to_backend({
        'type': 3,
        'time': unix_seconds,
        'sourceAmount': 12345,
        'destinationAmount': 0,
        'sourceAccountId': '1',
        'destinationAccountId': '0',
        'comment': 'seconds input'
    })

    assert backend_data['date'] == '2025-01-01 00:00:00'


def test_frontend_to_backend_accepts_unix_milliseconds() -> None:
    """Historical callers may still send unix milliseconds."""
    adapter = TransactionAdapter()
    unix_seconds = int(datetime(2025, 1, 1, 0, 0, 0).timestamp())

    backend_data, _ = adapter.frontend_to_backend({
        'type': 3,
        'time': unix_seconds * 1000,
        'sourceAmount': 12345,
        'destinationAmount': 0,
        'sourceAccountId': '1',
        'destinationAccountId': '0',
        'comment': 'milliseconds input'
    })

    assert backend_data['date'] == '2025-01-01 00:00:00'


async def test_backend_to_frontend_returns_unix_seconds() -> None:
    """Frontend DateTime components consume unix seconds, not milliseconds."""
    adapter = TransactionAdapter()
    expected_unix_seconds = int(datetime(2025, 1, 1, 0, 0, 0).timestamp())

    frontend_data = await adapter.backend_to_frontend({
        'id': 1,
        'time_sequence_id': 1,
        'type': '支出',
        'date': '2025-01-01 00:00:00',
        'amount': -123.45,
        'destination_amount': 0,
        'source_account_id': 1,
        'destination_account_id': 0,
        'main_category': '',
        'sub_category': '',
        'description': 'backend bill',
        'utc_offset': 480,
    })

    assert frontend_data['time'] == expected_unix_seconds
