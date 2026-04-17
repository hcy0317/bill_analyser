"""End-to-end regression tests for shared new_ui test-user cleanup hooks."""

from __future__ import annotations

import asyncio

from tests.new_ui.test_bills_api import _build_isolated_auth_headers
from tests.user_cleanup_support import cleanup_registered_test_users


def _lookup_user_by_username(db, username: str | None):
    """Return one user row by username from the shared new_ui test database."""

    async def _lookup():
        if not username:
            return None
        return await db.get_user_by_username(username)

    return asyncio.run(_lookup())


def test_auth_identity_user_can_be_cleaned_via_shared_registry(auth_identity, db):
    """The auth_identity fixture should register a real user for shared cleanup."""
    username = auth_identity["username"]

    assert _lookup_user_by_username(db, username) is not None

    cleaned_ids = cleanup_registered_test_users(db)

    assert cleaned_ids
    assert _lookup_user_by_username(db, username) is None


def test_isolated_auth_helper_user_can_be_cleaned_via_shared_registry(client, db):
    """The isolated auth helper should register its user for shared cleanup too."""
    auth_headers = _build_isolated_auth_headers(client, "test_cleanup_helper")
    profile_response = client.get("/api/profile", headers=auth_headers)
    assert profile_response.status_code == 200

    username = profile_response.get_json()["result"]["username"]
    assert _lookup_user_by_username(db, username) is not None

    cleaned_ids = cleanup_registered_test_users(db)

    assert cleaned_ids
    assert _lookup_user_by_username(db, username) is None
