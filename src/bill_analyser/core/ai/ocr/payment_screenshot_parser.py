"""Parse Alipay and WeChat Pay screenshot OCR text into transaction fields."""

# pylint: disable=too-many-return-statements

from __future__ import annotations

import re
import unicodedata
from dataclasses import dataclass
from datetime import datetime


@dataclass(frozen=True)
class PaymentScreenshotParseResult:
    """Structured fields parsed from payment screenshot OCR text."""

    amount: float | None
    trade_time: str | None
    description: str | None
    payment_platform: str | None
    confidence: float


_AMOUNT_PATTERNS: tuple[re.Pattern[str], ...] = (
    re.compile(
        r"(?:付款金额|支付金额|实付金额|实付款|订单金额|转账金额|收款金额|金额|合计|总计)"
        r"[\s:：¥￥]*(?P<amount>[+-]?\d+(?:\.\d{1,2})?)",
        re.IGNORECASE,
    ),
    re.compile(r"[¥￥]\s*(?P<amount>[+-]?\d+(?:\.\d{1,2})?)"),
    re.compile(r"(?P<amount>[+-]?\d+\.\d{2})\s*(?:元|CNY|RMB)", re.IGNORECASE),
)

_DATETIME_PATTERNS: tuple[re.Pattern[str], ...] = (
    re.compile(
        r"(?P<year>\d{4})[-/.年](?P<month>\d{1,2})[-/.月](?P<day>\d{1,2})[日]?"
        r"\s+(?P<hour>\d{1,2}):(?P<minute>\d{2})(?::(?P<second>\d{2}))?"
    ),
    re.compile(
        r"(?P<year>\d{4})[-/.年](?P<month>\d{1,2})[-/.月](?P<day>\d{1,2})[日]?"
    ),
    re.compile(
        r"(?P<month>\d{1,2})月(?P<day>\d{1,2})日\s+"
        r"(?P<hour>\d{1,2}):(?P<minute>\d{2})(?::(?P<second>\d{2}))?"
    ),
)

_DESCRIPTION_LABELS = (
    "交易对方",
    "收款方",
    "付款方",
    "商户",
    "商家",
    "对方账户",
    "商品",
    "商品说明",
    "订单名称",
    "备注",
    "说明",
)

_NOISE_TOKENS = (
    "微信支付",
    "支付宝",
    "支付成功",
    "交易成功",
    "商家服务",
    "当前状态",
    "付款方式",
    "支付方式",
    "交易单号",
    "商户单号",
    "订单号",
    "创建时间",
    "支付时间",
    "交易时间",
    "付款时间",
    "收款时间",
    "账单详情",
    "交易详情",
)


def parse_payment_screenshot_text(text: str) -> PaymentScreenshotParseResult:
    """Extract amount, time, description, and platform from OCR text."""

    normalized_text = _normalize_text(text)
    if not normalized_text:
        return PaymentScreenshotParseResult(None, None, None, None, 0.0)

    lines = _normalize_lines(normalized_text)
    payment_platform = _detect_payment_platform(normalized_text)
    amount = _parse_amount(normalized_text)
    trade_time = _parse_trade_time(normalized_text)
    description = _parse_description(lines)

    confidence = _score_confidence(
        amount=amount,
        trade_time=trade_time,
        description=description,
        payment_platform=payment_platform,
    )
    return PaymentScreenshotParseResult(
        amount=amount,
        trade_time=trade_time,
        description=description,
        payment_platform=payment_platform,
        confidence=confidence,
    )


def _normalize_text(text: str) -> str:
    text = unicodedata.normalize("NFKC", text or "")
    text = text.replace("\u00a0", " ")
    return text.strip()


def _normalize_lines(text: str) -> list[str]:
    lines: list[str] = []
    for raw_line in re.split(r"[\r\n]+", text):
        line = re.sub(r"\s+", " ", raw_line).strip(" :：\t")
        if line:
            lines.append(line)
    return lines


def _detect_payment_platform(text: str) -> str | None:
    lowered = text.lower()
    if "微信支付" in text or "财付通" in text or "wechat pay" in lowered:
        return "wechat_pay"
    if "支付宝" in text or "alipay" in lowered or "花呗" in text or "余额宝" in text:
        return "alipay"
    return None


def _parse_amount(text: str) -> float | None:
    for pattern in _AMOUNT_PATTERNS:
        match = pattern.search(text)
        if not match:
            continue
        amount_text = match.group("amount")
        try:
            return abs(float(amount_text.replace(",", "")))
        except ValueError:
            continue
    return None


def _parse_trade_time(text: str) -> str | None:
    for pattern in _DATETIME_PATTERNS:
        match = pattern.search(text)
        if not match:
            continue
        groups = match.groupdict()
        year = int(groups.get("year") or datetime.now().year)
        month = int(groups["month"])
        day = int(groups["day"])
        hour = int(groups.get("hour") or 0)
        minute = int(groups.get("minute") or 0)
        has_second = groups.get("second") is not None
        second = int(groups.get("second") or 0)
        try:
            value = datetime(year, month, day, hour, minute, second)
        except ValueError:
            continue
        if groups.get("hour") is None:
            return value.strftime("%Y-%m-%d")
        if has_second:
            return value.strftime("%Y-%m-%d %H:%M:%S")
        return value.strftime("%Y-%m-%d %H:%M")
    return None


def _parse_description(lines: list[str]) -> str | None:
    labeled = _parse_labeled_description(lines)
    if labeled:
        return labeled

    for line in lines:
        if _is_description_candidate(line):
            return _clean_description(line)
    return None


def _parse_labeled_description(lines: list[str]) -> str | None:
    for index, line in enumerate(lines):
        for label in _DESCRIPTION_LABELS:
            value = _value_after_label(line, label)
            if value and _is_description_candidate(value):
                return _clean_description(value)
            if line == label and index + 1 < len(lines):
                next_line = lines[index + 1]
                if _is_description_candidate(next_line):
                    return _clean_description(next_line)
    return None


def _value_after_label(line: str, label: str) -> str | None:
    if not line.startswith(label):
        return None
    remainder = line[len(label) :]
    if remainder and remainder[0] not in " :：":
        return None
    value = remainder.strip(" :：")
    return value or None


def _is_description_candidate(line: str) -> bool:
    if not line or len(line) > 80:
        return False
    if any(token in line for token in _NOISE_TOKENS):
        return False
    if any(pattern.search(line) for pattern in _AMOUNT_PATTERNS):
        return False
    if any(pattern.search(line) for pattern in _DATETIME_PATTERNS):
        return False
    if re.fullmatch(r"[0-9A-Za-z\-_:]{8,}", line):
        return False
    if line in _DESCRIPTION_LABELS:
        return False
    return bool(re.search(r"[\u4e00-\u9fffA-Za-z]", line))


def _clean_description(value: str) -> str:
    value = re.sub(r"\s+", " ", value).strip(" :：")
    return value[:60]


def _score_confidence(
    *,
    amount: float | None,
    trade_time: str | None,
    description: str | None,
    payment_platform: str | None,
) -> float:
    score = 0.0
    if amount is not None:
        score += 0.3
    if trade_time:
        score += 0.25
    if description:
        score += 0.25
    if payment_platform:
        score += 0.2
    return min(1.0, score)
