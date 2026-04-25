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

GENERIC_INVESTMENT_PLATFORMS = {"基金销售平台", "证券账户"}

INVESTMENT_ACTION_KEYWORDS = {
    "买入",
    "卖出",
    "申购",
    "赎回",
    "定投",
    "扣款",
    "自动定投",
    "转入",
    "转出",
    "分红",
    "分红发放",
    "收益",
    "收益发放",
    "亏损",
    "确认份额",
    "购买",
}

INVESTMENT_PNL_GAIN_KEYWORDS = {
    "收益",
    "收益发放",
    "分红",
    "分红发放",
    "红利",
    "派息",
    "盈利",
    "利润",
}

INVESTMENT_PNL_LOSS_KEYWORDS = {
    "亏损",
    "亏损调整",
    "亏损扣款",
    "损失",
    "浮亏",
}

INTRINSIC_INVESTMENT_NEGATIVE_KEYWORDS = {
    "服务费",
    "账户服务",
    "账户管理",
    "开户",
    "销户",
    "生活缴费",
    "话费",
}

EXPLICIT_INVESTMENT_TYPES = {"投资", "investment", "5"}

INVESTMENT_CONTEXT_KEYWORDS = {
    "投资",
    "基金",
    "理财",
    "证券",
    "股票",
    "债券",
    "黄金",
    "etf",
    "lof",
    "reits",
}

ORDINARY_BANK_CONTEXT_KEYWORDS = {
    "银行",
    "银行卡",
    "储蓄卡",
    "借记卡",
    "活期",
    "存款",
    "账户",
    "结算户",
}

ORDINARY_BANK_INTEREST_KEYWORDS = {
    "结息",
    "账户结息",
    "银行结息",
    "存款结息",
    "活期结息",
    "利息收入",
    "利息入账",
    "银行利息",
    "存款利息",
    "活期利息",
}


def _append_unique(items: list[str], value: str) -> None:
    """Append a non-empty value once, preserving order."""
    normalized = str(value or "").strip()
    if normalized and normalized not in items:
        items.append(normalized)


def _contains_any(text_lower: str, keywords: set[str]) -> list[str]:
    """Return keywords contained in text, case-insensitively."""
    return [
        keyword
        for keyword in sorted(keywords, key=lambda item: (-len(item), item))
        if keyword.lower() in text_lower
    ]


def is_ordinary_bank_interest_income(
    bill: dict[str, Any],
    *,
    keyword_config: dict[str, list[str]] | None = None,
) -> bool:
    """Return True for ordinary bank interest/settlement rows, not investment PnL."""
    current_type = str(bill.get("type", "") or "").strip().lower()
    if current_type in ["转账", "transfer", "4"]:
        return False

    counterparty = str(bill.get("counterparty", "") or "").strip()
    payment_method = str(bill.get("payment_method", "") or "").strip()
    description = str(bill.get("description", "") or "").strip()
    original_category = str(bill.get("original_category", "") or "").strip()
    evidence_text = " ".join(
        part for part in [counterparty, payment_method, description, original_category] if part
    )
    if not evidence_text:
        return False

    evidence_text_lower = evidence_text.lower()
    has_settlement_interest = bool(
        _contains_any(evidence_text_lower, ORDINARY_BANK_INTEREST_KEYWORDS)
        or "结息" in evidence_text_lower
    )
    has_generic_interest = "利息" in evidence_text_lower
    if not has_settlement_interest and not has_generic_interest:
        return False

    if has_generic_interest and not has_settlement_interest:
        amount = float(bill.get("amount", 0) or 0)
        if current_type in ["支出", "expense", "3"] or amount < 0:
            return False

    if not any(keyword.lower() in evidence_text_lower for keyword in ORDINARY_BANK_CONTEXT_KEYWORDS):
        return False

    effective_keyword_config = keyword_config or build_user_investment_keyword_settings(None)
    investment_profile = extract_investment_profile(
        evidence_text,
        keyword_config=effective_keyword_config,
    )
    if investment_profile.get("platform") or investment_profile.get("product"):
        return False

    investment_keywords = [
        *effective_keyword_config["platform_keywords"],
        *effective_keyword_config["product_keywords"],
        *INVESTMENT_CONTEXT_KEYWORDS,
    ]
    return not any(keyword.lower() in evidence_text_lower for keyword in investment_keywords)


def score_investment_candidate(
    bill: dict[str, Any],
    *,
    allow_existing_investment: bool = False,
    keyword_config: dict[str, list[str]] | None = None,
) -> dict[str, Any] | None:
    """Calculate whether a bill should be treated as an investment candidate."""
    # pylint: disable=too-many-locals,too-many-branches
    # pylint: disable=too-many-return-statements,too-many-statements
    current_type = str(bill.get("type", "") or "").strip().lower()
    if current_type in ["转账", "transfer", "4"]:
        return None
    is_explicit_investment_type = current_type in EXPLICIT_INVESTMENT_TYPES
    if not allow_existing_investment and is_explicit_investment_type:
        return None

    counterparty = str(bill.get("counterparty", "") or "").strip()
    payment_method = str(bill.get("payment_method", "") or "").strip()
    description = str(bill.get("description", "") or "").strip()
    original_category = str(bill.get("original_category", "") or "").strip()
    assigned_category_text = " ".join(
        part
        for part in [
            str(bill.get("main_category", "") or "").strip(),
            str(bill.get("sub_category", "") or "").strip(),
        ]
        if part
    )
    evidence_text = " ".join(
        part for part in [counterparty, payment_method, description, original_category] if part
    )
    all_text = " ".join(part for part in [evidence_text, assigned_category_text] if part)
    if not evidence_text:
        return None

    evidence_text_lower = evidence_text.lower()
    all_text_lower = all_text.lower()
    effective_keyword_config = keyword_config or build_user_investment_keyword_settings(None)
    if is_ordinary_bank_interest_income(
        bill,
        keyword_config=effective_keyword_config,
    ) or classify_investment_pnl_change(
        bill,
        keyword_config=effective_keyword_config,
    ):
        return None

    investment_profile = extract_investment_profile(
        evidence_text,
        keyword_config=effective_keyword_config,
    )
    normalized_platform = investment_profile.get("platform", "")
    normalized_product = investment_profile.get("product", "")

    platform_keywords = effective_keyword_config["platform_keywords"]
    product_keywords = [
        keyword
        for keyword in effective_keyword_config["product_keywords"]
        if keyword not in INVESTMENT_ACTION_KEYWORDS
    ]
    exclude_keywords = effective_keyword_config["exclude_keywords"]
    action_matches = _contains_any(evidence_text_lower, INVESTMENT_ACTION_KEYWORDS)
    intrinsic_negative_matches = _contains_any(
        all_text_lower,
        INTRINSIC_INVESTMENT_NEGATIVE_KEYWORDS,
    )

    matched_platforms: list[str] = []
    _append_unique(matched_platforms, normalized_platform)
    for keyword in platform_keywords:
        if keyword.lower() in evidence_text_lower:
            _append_unique(matched_platforms, keyword)

    matched_products: list[str] = []
    _append_unique(matched_products, normalized_product)
    for keyword in product_keywords:
        if keyword.lower() in evidence_text_lower:
            _append_unique(matched_products, keyword)
    matched_excludes = [
        keyword for keyword in exclude_keywords if keyword.lower() in all_text_lower
    ]

    has_specific_platform = any(
        platform not in GENERIC_INVESTMENT_PLATFORMS for platform in matched_platforms
    )
    has_named_or_specific_product = any(
        product
        and len(product) > 2
        and product not in {"基金", "理财", "债券", "股票", "黄金", "组合", "计划"}
        for product in matched_products
    )

    score = 0.0
    if has_specific_platform:
        score += min(0.65, 0.38 * len(matched_platforms[:2]))
    elif matched_platforms:
        score += 0.28
    if matched_products:
        score += min(0.42, 0.18 * len(matched_products[:3]))
    if action_matches:
        score += 0.12
    explicit_investment_type_boost = (
        0.08
        if allow_existing_investment
        and is_explicit_investment_type
        and has_specific_platform
        and action_matches
        else 0.0
    )
    score += explicit_investment_type_boost
    if matched_excludes:
        score -= min(0.48, 0.28 * len(matched_excludes[:2]))
    if intrinsic_negative_matches:
        score -= min(0.55, 0.35 * len(intrinsic_negative_matches[:2]))

    if not matched_platforms and len(matched_products) < 2:
        return None
    if (
        matched_platforms
        and not has_specific_platform
        and not has_named_or_specific_product
        and not action_matches
    ):
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
    if intrinsic_negative_matches:
        reason_parts.append("negative:" + "/".join(intrinsic_negative_matches[:2]))
    if explicit_investment_type_boost > 0:
        reason_parts.append("type:investment")

    hint_tokens = matched_platforms[:1] + matched_products[:2]
    hint_text = " ".join(hint_tokens)

    return {
        "score": round(min(score, 1.0), 2),
        "reason": ", ".join(reason_parts),
        "hint_text": hint_text,
        "platform": normalized_platform,
        "product": normalized_product,
    }


def classify_investment_pnl_change(
    bill: dict[str, Any],
    *,
    keyword_config: dict[str, list[str]] | None = None,
) -> dict[str, Any] | None:
    """Detect same-account investment profit/loss rows as balance-changing PnL signals."""
    # pylint: disable=too-many-locals
    current_type = str(bill.get("type", "") or "").strip().lower()
    if current_type in ["转账", "transfer", "4"]:
        return None

    counterparty = str(bill.get("counterparty", "") or "").strip()
    payment_method = str(bill.get("payment_method", "") or "").strip()
    description = str(bill.get("description", "") or "").strip()
    original_category = str(bill.get("original_category", "") or "").strip()
    assigned_category_text = " ".join(
        part
        for part in [
            str(bill.get("main_category", "") or "").strip(),
            str(bill.get("sub_category", "") or "").strip(),
        ]
        if part
    )
    evidence_text = " ".join(
        part for part in [counterparty, payment_method, description, original_category] if part
    )
    all_text = " ".join(part for part in [evidence_text, assigned_category_text] if part)
    if not evidence_text:
        return None

    all_text_lower = all_text.lower()
    effective_keyword_config = keyword_config or build_user_investment_keyword_settings(None)
    if is_ordinary_bank_interest_income(bill, keyword_config=effective_keyword_config):
        return None

    loss_matches = _contains_any(all_text_lower, INVESTMENT_PNL_LOSS_KEYWORDS)
    gain_matches = _contains_any(all_text_lower, INVESTMENT_PNL_GAIN_KEYWORDS)
    if not loss_matches and not gain_matches:
        return None

    investment_profile = extract_investment_profile(
        evidence_text,
        keyword_config=effective_keyword_config,
    )
    normalized_platform = investment_profile.get("platform", "")
    normalized_product = investment_profile.get("product", "")
    has_config_platform = any(
        keyword.lower() in all_text_lower
        for keyword in effective_keyword_config["platform_keywords"]
    )
    has_config_product = any(
        keyword.lower() in all_text_lower
        for keyword in effective_keyword_config["product_keywords"]
    )
    has_explicit_investment_context = current_type in EXPLICIT_INVESTMENT_TYPES or any(
        keyword in all_text_lower for keyword in INVESTMENT_CONTEXT_KEYWORDS
    )
    if not (
        normalized_platform
        or normalized_product
        or has_config_platform
        or has_config_product
        or has_explicit_investment_context
    ):
        return None

    # Loss wins when both words are present in adjustment descriptions.
    direction = "loss" if loss_matches else "gain"
    direction_label = "亏损" if direction == "loss" else "收益"
    matched_keywords = loss_matches if direction == "loss" else gain_matches

    score = 0.72
    if normalized_platform or has_config_platform:
        score += 0.08
    if normalized_product or has_config_product:
        score += 0.06
    if current_type in EXPLICIT_INVESTMENT_TYPES:
        score += 0.04

    reason_parts = [f"盈亏变化:{direction_label}"]
    if normalized_platform:
        reason_parts.append(f"platform:{normalized_platform}")
    if normalized_product:
        reason_parts.append(f"product:{normalized_product}")
    reason_parts.append("keyword:" + "/".join(matched_keywords[:2]))

    hint_tokens = [token for token in [normalized_platform, normalized_product] if token]

    return {
        "score": round(min(score, 1.0), 2),
        "reason": ", ".join(reason_parts),
        "hint_text": " ".join(hint_tokens),
        "platform": normalized_platform,
        "product": normalized_product,
        "signal_type": "pnl_change",
        "label": "盈亏变化",
        "direction": direction,
        "direction_label": direction_label,
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

    platform = ""
    best_platform_score = (-1, -1)
    for canonical, aliases in DEFAULT_INVESTMENT_PLATFORM_ALIASES:
        alias_candidates = [canonical, *aliases]
        matched_aliases = [alias for alias in alias_candidates if alias.lower() in text_lower]
        if not matched_aliases:
            continue

        best_alias = max(matched_aliases, key=len)
        platform_score = (0 if canonical in GENERIC_INVESTMENT_PLATFORMS else 1, len(best_alias))
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
        if (
            platform in GENERIC_INVESTMENT_PLATFORMS
            and product in {"基金", "理财", "债券", "股票", "黄金", "组合", "计划"}
            and _contains_any(text_lower, INTRINSIC_INVESTMENT_NEGATIVE_KEYWORDS)
        ):
            product = ""

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
