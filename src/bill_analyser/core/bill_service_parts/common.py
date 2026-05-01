"""Shared imports for BillService functional shards."""
# pylint: disable=unused-import

import asyncio
import inspect
import json
import re
import warnings
from datetime import datetime
from difflib import SequenceMatcher
from typing import Any, cast

from ...parsers.factory import ParserFactory
from ...parsers.parser_tags import resolve_parser_tags
from ...utils.constants import TransactionType
from ...utils.logger import get_logger, log_method, log_step
from ...utils.validator import BillValidator
from ..bill_date_utils import parse_bill_datetime
from ..category_engine import CategoryEngine
from ..db import Database
from ..db_reconciliation import ReconciliationProjectionConflictError
from ..import_learning.features import (
    FEATURE_SCHEMA_VERSION,
    build_feature_payload,
    parse_route_label,
    parse_semantic_label,
)
from ..import_learning.model import MODEL_KEY, predict_dual_head
from ..import_learning.policy import POLICY_VERSION, evaluate_learning_policy
from ..investment_matching import (
    classify_investment_pnl_change,
    clean_investment_product_name,
    extract_investment_profile,
    is_ordinary_bank_interest_income,
    score_investment_candidate,
)
from ..investment_settings import build_user_investment_keyword_settings
from ..matching import build_matching_session_candidates, build_preview_matching_payload
from ..matching.candidate_ids import (
    build_formal_learning_candidate_id,
    build_learning_rule_revision,
    parse_matching_candidate_id,
)
from ..smart_dedup import DeduplicationType, SmartDeduplicationEngine
