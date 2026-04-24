"""Focused tests for LLM provider aliases used by the desktop config UI."""

from __future__ import annotations

from bill_analyser.core.llm_provider import ClaudeProvider, OpenAIProvider, ProviderFactory


def test_provider_factory_supports_frontend_provider_values() -> None:
    """Frontend provider values should be directly creatable by the backend factory."""
    expected_providers = {
        "openai",
        "claude",
        "anthropic",
        "deepseek",
        "ollama",
        "xai",
        "google",
        "openrouter",
        "openai_compatible",
        "openai-compatible",
        "azure",
        "azure_openai",
    }

    assert expected_providers.issubset(set(ProviderFactory.available_providers()))


def test_provider_factory_normalizes_anthropic_to_claude() -> None:
    provider = ProviderFactory.create("anthropic", {"api_key": "secret", "model": ""})

    assert isinstance(provider, ClaudeProvider)
    assert provider.base_url == "https://api.anthropic.com/v1"
    assert provider.model == "claude-sonnet-4-20250514"


def test_provider_factory_applies_openai_compatible_defaults() -> None:
    cases = {
        "deepseek": ("https://api.deepseek.com/v1", "deepseek-chat"),
        "xai": ("https://api.x.ai/v1", "grok-3-mini"),
        "google": ("https://generativelanguage.googleapis.com/v1beta/openai", "gemini-2.0-flash"),
        "openrouter": ("https://openrouter.ai/api/v1", "openai/gpt-4o-mini"),
    }

    for provider_name, (base_url, model) in cases.items():
        provider = ProviderFactory.create(provider_name, {"api_key": "secret", "base_url": "", "model": ""})

        assert isinstance(provider, OpenAIProvider)
        assert provider.base_url == base_url
        assert provider.model == model
        assert provider.provider_name == provider_name


def test_provider_factory_preserves_custom_openai_compatible_endpoint() -> None:
    provider = ProviderFactory.create(
        "openai_compatible",
        {
            "api_key": "secret",
            "base_url": "https://llm.example.test/v1",
            "model": "custom-chat",
        },
    )

    assert isinstance(provider, OpenAIProvider)
    assert provider.base_url == "https://llm.example.test/v1"
    assert provider.model == "custom-chat"
    assert provider.provider_name == "openai_compatible"
