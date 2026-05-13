"""Settings bundle REST sidecar removal regressions."""


def test_settings_bundle_routes_removed_from_flask_sidecar(client):
    """Settings bundle REST runtime is owned by Rust, not the Flask sidecar."""
    cases = [
        ("get", "/api/settings/bundle/export", None),
        ("get", "/api/settings/bundle/sections/transactionTags/export", None),
        (
            "post",
            "/api/settings/bundle/sections/llmConfigs/export",
            {"password": "Test123456!"},
        ),
        ("post", "/api/settings/bundle/import/preview", {"schemaVersion": 1, "sections": {}}),
        ("post", "/api/settings/bundle/import", {"schemaVersion": 1, "sections": {}}),
        (
            "post",
            "/api/settings/bundle/sections/transactionTags/import/preview",
            {"schemaVersion": 1, "sections": {"transactionTags": []}},
        ),
        (
            "post",
            "/api/settings/bundle/sections/transactionTags/import",
            {"schemaVersion": 1, "sections": {"transactionTags": []}},
        ),
    ]

    for method, url, json_body in cases:
        kwargs = {}
        if json_body is not None:
            kwargs["json"] = json_body
        response = getattr(client, method)(url, **kwargs)
        assert response.status_code in (404, 405), (
            f"{method.upper()} {url} should not be registered in Flask sidecar"
        )
