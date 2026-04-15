"""Shared helpers for investment candidate eligibility and profile extraction."""

from __future__ import annotations

import re
from typing import Any

from .investment_settings import (
    DEFAULT_INVESTMENT_NAMED_PRODUCT_PATTERNS,
    DEFAULT_INVESTMENT_PLATFORM_ALIASES,
    DEFAULT_INVESTMENT_PRODUCT_PATTERNS,
    build_user_investment_keyword_settings,
)


def score_investment_candidate(
    bill: dict[str, Any],
    *,
    allow_existing_investment: bool = False,
    keyword_config: dict[str, list[str]] | None = None,
) -> dict[str, Any] | None:
    """Calculate whether a bill should be treated as an investment candidate."""
    # pylint: disable=too-many-locals,too-many-branches
    current_type = str(bill.get("type", "") or "").strip().lower()
    if current_type in ["转账", "transfer", "4"]:
        return None
    if not allow_existing_investment and current_type in ["投资", "investment", "5"]:
        return None

    text_parts = [
        str(bill.get("counterparty", "") or "").strip(),
        str(bill.get("payment_method", "") or "").strip(),
        str(bill.get("description", "") or "").strip(),
        str(bill.get("main_category", "") or "").strip(),
        str(bill.get("sub_category", "") or "").strip(),
        str(bill.get("original_category", "") or "").strip(),
    ]
    text_blob = " ".join(part for part in text_parts if part)
    if not text_blob:
        return None

    text_lower = text_blob.lower()
    effective_keyword_config = keyword_config or build_user_investment_keyword_settings(None)
    investment_profile = extract_investment_profile(
        text_blob,
        keyword_config=effective_keyword_config,
    )
    normalized_platform = investment_profile.get("platform", "")
    normalized_product = investment_profile.get("product", "")

    platform_keywords = effective_keyword_config["platform_keywords"]
    product_keywords = effective_keyword_config["product_keywords"]
    exclude_keywords = effective_keyword_config["exclude_keywords"]

    matched_platforms: list[str] = []
    if normalized_platform:
        matched_platforms.append(normalized_platform)
    matched_platforms.extend(
        [
            keyword
            for keyword in platform_keywords
            if keyword not in matched_platforms and keyword.lower() in text_lower
        ]
    )
    matched_products = [keyword for keyword in product_keywords if keyword.lower() in text_lower]
    if normalized_product and normalized_product not in matched_products:
        matched_products.insert(0, normalized_product)
    matched_excludes = [keyword for keyword in exclude_keywords if keyword.lower() in text_lower]

    score = 0.0
    if matched_platforms:
        score += min(0.65, 0.38 * len(matched_platforms[:2]))
    if matched_products:
        score += min(0.42, 0.18 * len(matched_products[:3]))
    if matched_excludes:
        score -= min(0.48, 0.28 * len(matched_excludes[:2]))

    if not matched_platforms and len(matched_products) < 2:
        return None

    if score < 0.55:
        return None

    reason_parts: list[str] = []
    if matched_platforms:
        reason_parts.append("platform:" + "/".join(matched_platforms[:2]))
    if matched_products:
        reason_parts.append("product:" + "/".join(matched_products[:3]))
    if matched_excludes:
        reason_parts.append("exclude:" + "/".join(matched_excludes[:2]))

    hint_tokens = matched_platforms[:1] + matched_products[:2]
    hint_text = " ".join(hint_tokens)

    return {
        "score": round(min(score, 1.0), 2),
        "reason": ", ".join(reason_parts),
        "hint_text": hint_text,
        "platform": normalized_platform,
        "product": normalized_product,
    }


def extract_investment_profile(
    text: str,
    *,
    keyword_config: dict[str, list[str]] | None = None,
) -> dict[str, str]:
    """Extract normalized investment platform and product hints from text."""
    # pylint: disable=too-many-locals,too-many-branches
    raw_text = str(text or "").strip()
    if not raw_text:
        return {"platform": "", "product": ""}

    text_lower = raw_text.lower()
    effective_keyword_config = keyword_config or build_user_investment_keyword_settings(None)
    generic_platforms = {"基金销售平台", "证券账户"}

    platform = ""
    best_platform_score = (-1, -1)
    for canonical, aliases in DEFAULT_INVESTMENT_PLATFORM_ALIASES:
        alias_candidates = [canonical, *aliases]
        matched_aliases = [alias for alias in alias_candidates if alias.lower() in text_lower]
        if not matched_aliases:
            continue

        best_alias = max(matched_aliases, key=len)
        platform_score = (0 if canonical in generic_platforms else 1, len(best_alias))
        if platform_score > best_platform_score:
            best_platform_score = platform_score
            platform = canonical

    if not platform:
        for keyword in sorted(effective_keyword_config["platform_keywords"], key=len, reverse=True):
            if keyword.lower() in text_lower:
                platform = keyword
                break

    product = ""
    named_product = ""
    matched_config_products = [
        keyword
        for keyword in sorted(effective_keyword_config["product_keywords"], key=len, reverse=True)
        if keyword.lower() in text_lower
    ]
    if matched_config_products:
        product = matched_config_products[0]

    for pattern in DEFAULT_INVESTMENT_NAMED_PRODUCT_PATTERNS:
        match = re.search(pattern, raw_text, re.IGNORECASE)
        if not match:
            continue
        candidate = clean_investment_product_name(match.group(1).strip(), platform=platform)
        if candidate:
            named_product = candidate
            break

    if named_product and (not product or len(named_product) > len(product)):
        product = named_product

    if not product:
        best_product_score = -1
        for canonical, aliases in DEFAULT_INVESTMENT_PRODUCT_PATTERNS:
            alias_candidates = [canonical, *aliases]
            matched_aliases = [alias for alias in alias_candidates if alias.lower() in text_lower]
            if not matched_aliases:
                continue

            best_alias = max(matched_aliases, key=len)
            if len(best_alias) > best_product_score:
                best_product_score = len(best_alias)
                product = best_alias

    if product:
        product = clean_investment_product_name(product, platform=platform)

    return {"platform": platform, "product": product}


def clean_investment_product_name(product: str, *, platform: str = "") -> str:
    """Normalize an extracted investment product name."""
    cleaned = str(product or "").strip().strip("|｜,， ")
    if not cleaned:
        return ""

    cleaned = re.sub(
        r"^(买入|卖出|申购|赎回|定投|扣款|自动定投|转入|转出)[-－:：\s]*",
        "",
        cleaned,
        flags=re.IGNORECASE,
    )
    cleaned = re.sub(
        r"[-－:：\s]*(买入|卖出|申购|赎回|定投|扣款|自动定投|转入|转出|确认份额|分红再投资)$",
        "",
        cleaned,
        flags=re.IGNORECASE,
    )

    for canonical, aliases in DEFAULT_INVESTMENT_PLATFORM_ALIASES:
        alias_candidates = [canonical, *aliases]
        for alias in alias_candidates:
            cleaned = re.sub(rf"^{re.escape(alias)}[-－:：\s]*", "", cleaned, flags=re.IGNORECASE)

    if platform:
        cleaned = re.sub(rf"^{re.escape(platform)}[-－:：\s]*", "", cleaned, flags=re.IGNORECASE)

    cleaned = re.sub(
        r"^(买入|卖出|申购|赎回|定投|扣款|自动定投|转入|转出)[-－:：\s]*",
        "",
        cleaned,
        flags=re.IGNORECASE,
    )

    if "|" in cleaned or "｜" in cleaned:
        candidates = [segment.strip().strip("|｜,， ") for segment in re.split(r"[|｜]", cleaned)]
        preferred = [
            segment
            for segment in candidates
            if re.search(
                r"(基金|ETF|LOF|REITs|REIT|理财|计划|组合|债券|股票|黄金|A类|C类|联接)",
                segment,
                re.IGNORECASE,
            )
        ]
        if preferred:
            cleaned = max(preferred, key=len)

    cleaned = re.sub(r"\s+", " ", cleaned).strip().strip("-－:：|｜,， ")
    return cleaned[:80]
