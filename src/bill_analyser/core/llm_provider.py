"""LLM Provider abstraction layer for bill classification and rule induction."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from typing import Any, Protocol

import httpx

from ..utils.logger import get_logger

logger = get_logger("LLMProvider")

_DEFAULT_TIMEOUT = 30.0
_MAX_RETRIES = 3
_RETRY_BACKOFF = 1.0


@dataclass(frozen=True)
class LLMResponse:
    """Standardised response from any LLM provider."""

    content: str
    model: str
    provider: str
    tokens_used: int
    raw_response: dict[str, Any] = field(default_factory=dict)


class LLMProvider(Protocol):
    """Protocol for LLM provider implementations."""

    async def generate(
        self,
        prompt: str,
        system_prompt: str = "",
        temperature: float = 0.3,
        max_tokens: int = 2048,
    ) -> LLMResponse: ...


class OpenAIProvider:
    """OpenAI-compatible API provider using httpx."""

    def __init__(self, config: dict[str, Any]) -> None:
        self.api_key: str = config.get("api_key", "")
        self.base_url: str = config.get("base_url", "https://api.openai.com/v1")
        self.model: str = config.get("model", "gpt-4o-mini")
        self.timeout: float = config.get("timeout", _DEFAULT_TIMEOUT)

    async def generate(
        self,
        prompt: str,
        system_prompt: str = "",
        temperature: float = 0.3,
        max_tokens: int = 2048,
    ) -> LLMResponse:
        messages: list[dict[str, str]] = []
        if system_prompt:
            messages.append({"role": "system", "content": system_prompt})
        messages.append({"role": "user", "content": prompt})

        payload = {
            "model": self.model,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
        }

        data = await _request_with_retries(
            url=f"{self.base_url.rstrip('/')}/chat/completions",
            headers={
                "Authorization": f"Bearer {self.api_key}",
                "Content-Type": "application/json",
            },
            json_body=payload,
            timeout=self.timeout,
        )

        content = data.get("choices", [{}])[0].get("message", {}).get("content", "")
        tokens_used = data.get("usage", {}).get("total_tokens", 0)

        return LLMResponse(
            content=content,
            model=self.model,
            provider="openai",
            tokens_used=tokens_used,
            raw_response=data,
        )


class ClaudeProvider:
    """Anthropic Claude API provider using httpx."""

    def __init__(self, config: dict[str, Any]) -> None:
        self.api_key: str = config.get("api_key", "")
        self.base_url: str = config.get("base_url", "https://api.anthropic.com/v1")
        self.model: str = config.get("model", "claude-sonnet-4-20250514")
        self.timeout: float = config.get("timeout", _DEFAULT_TIMEOUT)

    async def generate(
        self,
        prompt: str,
        system_prompt: str = "",
        temperature: float = 0.3,
        max_tokens: int = 2048,
    ) -> LLMResponse:
        payload: dict[str, Any] = {
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": max_tokens,
            "temperature": temperature,
        }
        if system_prompt:
            payload["system"] = system_prompt

        data = await _request_with_retries(
            url=f"{self.base_url.rstrip('/')}/messages",
            headers={
                "x-api-key": self.api_key,
                "anthropic-version": "2023-06-01",
                "Content-Type": "application/json",
            },
            json_body=payload,
            timeout=self.timeout,
        )

        content_blocks = data.get("content", [])
        content = "".join(
            block.get("text", "") for block in content_blocks if block.get("type") == "text"
        )
        usage = data.get("usage", {})
        tokens_used = usage.get("input_tokens", 0) + usage.get("output_tokens", 0)

        return LLMResponse(
            content=content,
            model=self.model,
            provider="claude",
            tokens_used=tokens_used,
            raw_response=data,
        )


class OllamaProvider:
    """Ollama local model provider using httpx."""

    def __init__(self, config: dict[str, Any]) -> None:
        self.base_url: str = config.get("base_url", "http://localhost:11434")
        self.model: str = config.get("model", "llama3")
        self.timeout: float = config.get("timeout", _DEFAULT_TIMEOUT * 2)

    async def generate(
        self,
        prompt: str,
        system_prompt: str = "",
        temperature: float = 0.3,
        max_tokens: int = 2048,
    ) -> LLMResponse:
        payload: dict[str, Any] = {
            "model": self.model,
            "prompt": prompt,
            "stream": False,
            "options": {
                "temperature": temperature,
                "num_predict": max_tokens,
            },
        }
        if system_prompt:
            payload["system"] = system_prompt

        data = await _request_with_retries(
            url=f"{self.base_url.rstrip('/')}/api/generate",
            headers={"Content-Type": "application/json"},
            json_body=payload,
            timeout=self.timeout,
        )

        content = data.get("response", "")
        tokens_used = data.get("eval_count", 0) + data.get("prompt_eval_count", 0)

        return LLMResponse(
            content=content,
            model=self.model,
            provider="ollama",
            tokens_used=tokens_used,
            raw_response=data,
        )


class ProviderFactory:
    """Factory for creating LLM provider instances."""

    _PROVIDERS: dict[str, type] = {
        "openai": OpenAIProvider,
        "claude": ClaudeProvider,
        "ollama": OllamaProvider,
    }

    @classmethod
    def create(cls, provider_name: str, config: dict[str, Any]) -> LLMProvider:
        """Create a provider instance by name."""
        provider_cls = cls._PROVIDERS.get(provider_name.lower())
        if provider_cls is None:
            raise ValueError(
                f"Unknown provider '{provider_name}'. "
                f"Available: {list(cls._PROVIDERS.keys())}"
            )
        return provider_cls(config)  # type: ignore[return-value]

    @classmethod
    def available_providers(cls) -> list[str]:
        return list(cls._PROVIDERS.keys())


async def _request_with_retries(
    url: str,
    headers: dict[str, str],
    json_body: dict[str, Any],
    timeout: float = _DEFAULT_TIMEOUT,
) -> dict[str, Any]:
    """Execute HTTP POST with retry logic."""
    last_error: Exception | None = None

    for attempt in range(_MAX_RETRIES):
        try:
            async with httpx.AsyncClient(timeout=timeout) as client:
                resp = await client.post(url, headers=headers, json=json_body)
                resp.raise_for_status()
                return resp.json()
        except httpx.TimeoutException as exc:
            last_error = exc
            logger.warning(
                "LLM request timeout (attempt %d/%d): %s", attempt + 1, _MAX_RETRIES, url
            )
        except httpx.HTTPStatusError as exc:
            last_error = exc
            if exc.response.status_code >= 500:
                logger.warning(
                    "LLM server error %d (attempt %d/%d)",
                    exc.response.status_code,
                    attempt + 1,
                    _MAX_RETRIES,
                )
            else:
                raise
        except httpx.HTTPError as exc:
            last_error = exc
            logger.warning(
                "LLM request error (attempt %d/%d): %s", attempt + 1, _MAX_RETRIES, exc
            )

        if attempt < _MAX_RETRIES - 1:
            await asyncio.sleep(_RETRY_BACKOFF * (attempt + 1))

    raise RuntimeError(f"LLM request failed after {_MAX_RETRIES} retries: {last_error}")
