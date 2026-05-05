"""Parser tag normalization helpers for import contracts."""

from __future__ import annotations

import json
from typing import Any

PARSER_CHANNEL_TAGS: dict[str, str] = {
    "wechat": "wallet",
    "alipay": "wallet",
    "abc": "bank",
    "ccb": "bank",
    "cmbc": "bank",
    "icbc": "bank",
}


def _normalize_tag_value(value: Any) -> str:
    """Normalize one tag token into the repository's lowercase form."""
    return str(value or "").strip().lower()


def _dedupe_tags(tags: list[str]) -> list[str]:
    """Deduplicate tags while preserving the original order."""
    unique_tags: list[str] = []
    seen: set[str] = set()

    for tag in tags:
        if not tag or tag in seen:
            continue
        seen.add(tag)
        unique_tags.append(tag)

    return unique_tags


def normalize_parser_tags(raw_tags: Any) -> list[str]:
    """Normalize raw parser-tag payloads into a flat tag list."""
    if raw_tags in (None, ""):
        return []

    if isinstance(raw_tags, str):
        text = raw_tags.strip()
        if not text:
            return []

        try:
            return normalize_parser_tags(json.loads(text))
        except (TypeError, ValueError, json.JSONDecodeError):
            normalized_parts = [
                _normalize_tag_value(part)
                for part in text.split(",")
                if str(part).strip()
            ]
            return _dedupe_tags(normalized_parts)

    if isinstance(raw_tags, (list, tuple, set)):
        normalized_items = [
            _normalize_tag_value(item)
            for item in raw_tags
            if _normalize_tag_value(item)
        ]
        return _dedupe_tags(normalized_items)

    return []


def _detect_channel_tag(parser_id: str, payment_method: str, channel: str) -> str:
    """Infer a normalized channel tag from parser identity or payment text."""
    parser_key = _normalize_tag_value(parser_id)
    if parser_key in PARSER_CHANNEL_TAGS:
        return PARSER_CHANNEL_TAGS[parser_key]

    combined_text = f"{payment_method} {channel}".lower()
    if any(
        keyword in combined_text
        for keyword in ("wechat", "微信", "alipay", "支付宝", "零钱", "wallet")
    ):
        return "wallet"
    if any(keyword in combined_text for keyword in ("信用卡", "credit")):
        return "credit_card"
    if any(keyword in combined_text for keyword in ("银行", "bank", "借记卡", "储蓄卡")):
        return "bank"

    return ""


def build_parser_tags(
    *,
    parser_id: str = "",
    payment_method: str = "",
    channel: str = "",
) -> list[str]:
    """Build the default parser tags from parser identity and channel hints."""
    tags: list[str] = []
    normalized_parser_id = _normalize_tag_value(parser_id)
    if normalized_parser_id:
        tags.append(f"parser:{normalized_parser_id}")

    channel_tag = _detect_channel_tag(parser_id, payment_method, channel)
    if channel_tag:
        tags.append(f"channel:{channel_tag}")

    return _dedupe_tags(tags)


def resolve_parser_tags(
    raw_tags: Any,
    *,
    parser_id: str = "",
    payment_method: str = "",
    channel: str = "",
) -> list[str]:
    """Prefer normalized explicit tags, otherwise derive a minimal default set."""
    normalized_tags = normalize_parser_tags(raw_tags)
    if normalized_tags:
        return normalized_tags

    return build_parser_tags(
        parser_id=parser_id,
        payment_method=payment_method,
        channel=channel,
    )


def serialize_parser_tags(
    raw_tags: Any,
    *,
    parser_id: str = "",
    payment_method: str = "",
    channel: str = "",
) -> str:
    """Serialize parser tags as JSON text for database persistence."""
    return json.dumps(
        resolve_parser_tags(
            raw_tags,
            parser_id=parser_id,
            payment_method=payment_method,
            channel=channel,
        ),
        ensure_ascii=False,
    )
