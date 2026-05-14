"""Current-contract regression checks for frontend service guards."""

import os


class TestFrontendServicesTypeScript:
    """Static checks for the current frontend service implementation."""

    def test_services_ts_has_response_guard(self):
        """The cancelable response branch should guard error.response before access."""
        services_path = os.path.join(
            os.path.dirname(__file__),
            "..",
            "src",
            "web",
            "src",
            "lib",
            "services.ts",
        )

        assert os.path.exists(services_path), f"services.ts不存在: {services_path}"

        with open(services_path, "r", encoding="utf-8") as file_obj:
            content = file_obj.read()

        assert "if (error.response?.config && 'cancelableUuid' in error.response.config" in content
        assert "if (error.response && !error.response.config.ignoreError)" in content
