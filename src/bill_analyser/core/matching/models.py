"""Models for additive import-preview matching payloads."""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from typing import Any


@dataclass(frozen=True)
class TransferMatchingPayload:
    """Transfer suggestion mirror payload."""

    candidate_type: str = ""
    score: float = 0.0
    level: str = ""
    reason: str = ""
    review_status: str = ""
    reviewed_type: str = ""
    suppressed: bool = False
    pair_order: str = ""
    source_chain: list[dict[str, Any]] = field(default_factory=list)


@dataclass(frozen=True)
class InvestmentMatchingPayload:
    """Investment signal mirror payload."""

    score: float = 0.0
    level: str = ""
    reason: str = ""
    platform: str = ""
    product: str = ""
    review_status: str = ""
    suppressed: bool = False


@dataclass(frozen=True)
# pylint: disable=too-many-instance-attributes
class LearningMatchingPayload:
    """Learning recommendation mirror payload."""

    rule_id: int | None = None
    score: float = 0.0
    level: str = ""
    reason: str = ""
    recommended_type: str = ""
    summary: str = ""
    review_status: str = ""
    suppressed: bool = False


@dataclass(frozen=True)
class RecurringMatchingPayload:
    """Recurring candidate mirror payload."""

    id: int | None = None
    name: str = ""
    candidate_count: int = 0
    match_score: float = 0.0
    match_reasons: str = ""
    matched_date: str = ""


@dataclass(frozen=True)
class DedupMatchingPayload:
    """Dedup metadata mirror payload."""

    type: str = ""
    source_ids: Any = field(default_factory=list)
    source_count: int = 0
    source_labels: list[str] = field(default_factory=list)
    sources: list[dict[str, Any]] = field(default_factory=list)


@dataclass(frozen=True)
class ParserMatchingPayload:
    """Parser metadata mirror payload."""

    id: str = ""
    tags: list[str] = field(default_factory=list)
    source_chain: list[dict[str, Any]] = field(default_factory=list)


@dataclass(frozen=True)
class AnnotationMatchingPayload:
    """Manual-annotation metadata mirror payload."""

    is_manually_annotated: bool = False


@dataclass(frozen=True)
class ReconciliationMatchingPayload:
    """Import-to-formal-bill reconciliation provenance payload."""

    candidate_id: str = ""
    candidate_type: str = ""
    status: str = ""
    existing_bill_id: int | None = None
    group_id: int | None = None
    score: float = 0.0
    level: str = ""
    reason: str = ""
    signal_label: str = ""
    source_chain: list[dict[str, Any]] = field(default_factory=list)


@dataclass(frozen=True)
class PreviewMatchingPayload:
    """Additive nested matching structure for import preview items."""

    transfer: TransferMatchingPayload = field(default_factory=TransferMatchingPayload)
    investment: InvestmentMatchingPayload = field(default_factory=InvestmentMatchingPayload)
    learning: LearningMatchingPayload = field(default_factory=LearningMatchingPayload)
    recurring: RecurringMatchingPayload = field(default_factory=RecurringMatchingPayload)
    dedup: DedupMatchingPayload = field(default_factory=DedupMatchingPayload)
    parser: ParserMatchingPayload = field(default_factory=ParserMatchingPayload)
    annotation: AnnotationMatchingPayload = field(default_factory=AnnotationMatchingPayload)
    reconciliation: ReconciliationMatchingPayload = field(default_factory=ReconciliationMatchingPayload)

    def to_dict(self) -> dict[str, Any]:
        """Serialize the payload as a plain dictionary."""
        return asdict(self)
