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

from bill_analyser.core.bill_date_utils import parse_bill_datetime
from bill_analyser.core.category_engine import CategoryEngine
from bill_analyser.core.database.reconciliation import ReconciliationProjectionConflictError
from bill_analyser.core.db import Database
from bill_analyser.core.import_learning.features import (
    FEATURE_SCHEMA_VERSION,
    build_feature_payload,
    parse_route_label,
    parse_semantic_label,
)
from bill_analyser.core.import_learning.model import MODEL_KEY, predict_dual_head
from bill_analyser.core.import_learning.policy import POLICY_VERSION, evaluate_learning_policy
from bill_analyser.core.investment.matching import (
    classify_investment_pnl_change,
    clean_investment_product_name,
    extract_investment_profile,
    is_ordinary_bank_interest_income,
    score_investment_candidate,
)
from bill_analyser.core.investment.settings import build_user_investment_keyword_settings
from bill_analyser.core.matching import build_matching_session_candidates, build_preview_matching_payload
from bill_analyser.core.matching.candidate_ids import (
    build_formal_learning_candidate_id,
    build_learning_rule_revision,
    parse_matching_candidate_id,
)
from bill_analyser.core.smart_dedup import DeduplicationType, SmartDeduplicationEngine
from bill_analyser.parsers.factory import ParserFactory
from bill_analyser.import_contracts.parser_tags import resolve_parser_tags
from bill_analyser.utils.constants import TransactionType
from bill_analyser.utils.logger import get_logger, log_method, log_step
from bill_analyser.utils.validator import BillValidator
