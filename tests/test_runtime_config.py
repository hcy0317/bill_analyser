"""运行时配置收口回归测试。"""

import pytest

from bill_analyser.utils import config as config_module


def test_load_auth_settings_requires_jwt_secret(monkeypatch):
    """认证配置不应再回退到代码里的默认 JWT secret。"""
    monkeypatch.setattr(config_module, "get_server_config", lambda use_cache=True: {})
    monkeypatch.setattr(config_module, "load_env_settings", lambda: {})

    with pytest.raises(config_module.ConfigValidationError, match="JWT secret"):
        config_module.load_auth_settings()


def test_load_auth_settings_prefers_environment_secret_over_server_config(monkeypatch):
    """JWT secret 应优先取环境变量/.env，而不是 server_config.json。"""
    monkeypatch.setattr(config_module, "get_server_config", lambda use_cache=True: {"jwt_secret": "config-secret"})
    monkeypatch.setattr(config_module, "load_env_settings", lambda: {"JWT_SECRET_KEY": "env-secret"})

    settings = config_module.load_auth_settings()

    assert settings["jwt_secret"] == "env-secret"


def test_load_auth_settings_falls_back_to_server_config_when_env_missing(monkeypatch):
    """当环境变量/.env 未提供时，应继续兼容 server_config.json。"""
    monkeypatch.setattr(config_module, "get_server_config", lambda use_cache=True: {"jwt_secret": "config-secret"})
    monkeypatch.setattr(config_module, "load_env_settings", lambda: {})

    settings = config_module.load_auth_settings()

    assert settings["jwt_secret"] == "config-secret"


def test_load_api_runtime_settings_reads_api_section(monkeypatch):
    """API 监听地址、端口和 CORS 应从配置读取。"""
    monkeypatch.setattr(
        config_module,
        "get_server_config",
        lambda use_cache=True: {
            "api": {
                "host": "0.0.0.0",
                "port": "6789",
                "debug": True,
                "threaded": False,
                "cors": {
                    "origins": "http://localhost:3000, http://127.0.0.1:3000",
                    "max_age_seconds": "7200",
                },
            }
        },
    )

    runtime_config = config_module.load_api_runtime_settings()

    assert runtime_config["host"] == "0.0.0.0"
    assert runtime_config["port"] == 6789
    assert runtime_config["debug"] is True
    assert runtime_config["threaded"] is False
    assert runtime_config["cors"]["origins"] == ["http://localhost:3000", "http://127.0.0.1:3000"]
    assert runtime_config["cors"]["max_age_seconds"] == 7200


def test_load_api_runtime_settings_env_can_rebind_python_sidecar(monkeypatch):
    """Rust 主 HTTP 服务启动时可通过环境变量把 Python fallback 移到 sidecar 端口。"""
    monkeypatch.setattr(
        config_module,
        "get_server_config",
        lambda use_cache=True: {"api": {"host": "127.0.0.1", "port": 5000}},
    )
    monkeypatch.setattr(
        config_module,
        "load_env_settings",
        lambda: {"BILL_ANALYSER_API_HOST": "127.0.0.1", "BILL_ANALYSER_API_PORT": "5001"},
    )

    runtime_config = config_module.load_api_runtime_settings()

    assert runtime_config["host"] == "127.0.0.1"
    assert runtime_config["port"] == 5001


def test_load_default_user_settings_requires_explicit_credentials(monkeypatch):
    """默认管理员如启用自动创建，账号凭证必须显式配置。"""
    monkeypatch.setattr(
        config_module,
        "get_server_config",
        lambda use_cache=True: {
            "default_user": {
                "auto_create": True,
                "username": "admin",
            }
        },
    )

    with pytest.raises(config_module.ConfigValidationError, match="password, email"):
        config_module.load_default_user_settings()
