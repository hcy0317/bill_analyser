# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403

def _normalize_import_suggestion_text(value: Any) -> str:
    """标准化导入建议比较文本。"""
    text = str(value or "").strip().lower()
    return re.sub(r"[\s_\-\/\\()（）\[\]【】:：]+", "", text)


def _score_header_keyword_match(normalized_header: str, column_type: int) -> float:
    """根据关键词规则计算表头与导入列类型的匹配分。"""
    if column_type == 3:
        if normalized_header in {"收支", "收/支", "收支类型", "借贷标志", "借贷"}:
            return IMPORT_HEADER_STRONG_EXACT_SCORE
        if normalized_header in {"交易类型", "交易分类", "类别", "类型"}:
            return IMPORT_HEADER_TYPE_HINT_SCORE

    if column_type == 6 and "余额" in normalized_header:
        return 0.0
    if column_type == 6 and any(token in normalized_header for token in ["对方", "对手", "目标", "收款", "相关"]):
        return 0.0

    best_score = 0.0
    for keyword in IMPORT_COLUMN_TYPE_KEYWORDS.get(column_type, []):
        normalized_keyword = _normalize_import_suggestion_text(keyword)
        if not normalized_keyword:
            continue
        if normalized_header == normalized_keyword:
            best_score = max(best_score, IMPORT_HEADER_EXACT_SCORE)
        elif normalized_keyword in normalized_header or normalized_header in normalized_keyword:
            best_score = max(best_score, IMPORT_HEADER_PARTIAL_SCORE)

    if column_type == 4 and normalized_header in {"交易分类", "原始分类"}:
        best_score = max(best_score, IMPORT_HEADER_EXACT_SCORE)

    if column_type == 6 and any(token in normalized_header for token in ["支付方式", "付款方式", "收付款方式", "支付渠道"]):
        best_score = max(best_score, IMPORT_HEADER_SEMANTIC_SCORE)

    if column_type == 9 and any(token in normalized_header for token in ["户名", "名称"]):
        best_score = max(best_score, IMPORT_HEADER_SEMANTIC_MEDIUM_SCORE)

    if column_type == 14 and any(token in normalized_header for token in ["商品说明", "商品", "交易摘要"]):
        best_score = max(best_score, IMPORT_HEADER_SEMANTIC_SCORE)

    if column_type == 9 and any(token in normalized_header for token in ["对方", "转入", "目标", "收款"]):
        best_score = max(best_score, IMPORT_HEADER_TYPE_HINT_SCORE)
    if (
        column_type == 11
        and any(token in normalized_header for token in ["对方", "转入", "目标", "收款"])
        and "金额" in str(normalized_header)
    ):
        best_score = max(best_score, IMPORT_HEADER_TYPE_HINT_SCORE)
    if (
        column_type == 6
        and "账户" in str(normalized_header)
        and not any(token in normalized_header for token in ["对方", "转入", "目标", "收款"])
    ):
        best_score = max(best_score, IMPORT_HEADER_CONTEXT_SCORE)
    if (
        column_type == 8
        and "金额" in str(normalized_header)
        and not any(token in normalized_header for token in ["对方", "转入", "目标", "收款"])
    ):
        best_score = max(best_score, IMPORT_HEADER_CONTEXT_SCORE)

    return best_score


def _extract_import_config_header_type_pairs(config: dict[str, Any]) -> list[tuple[str, int, float]]:
    """从历史模板中提取 表头 -> 导入列类型 的关联。"""
    field_mappings = config.get("field_mappings") or {}
    sample_headers = config.get("sample_headers") or []
    use_count = float(config.get("use_count", 0) or 0)
    base_weight = IMPORT_CONFIG_BASE_WEIGHT + min(use_count, IMPORT_CONFIG_USE_COUNT_CAP) * IMPORT_CONFIG_USE_COUNT_FACTOR
    pairs: list[tuple[str, int, float]] = []

    column_mapping = field_mappings.get("columnMapping") if isinstance(field_mappings, dict) else None
    if isinstance(column_mapping, dict):
        for column_type, column_index in column_mapping.items():
            try:
                column_type_int = int(column_type)
                column_index_int = int(column_index)
            except (TypeError, ValueError):
                continue
            if 0 <= column_index_int < len(sample_headers):
                normalized_header = _normalize_import_suggestion_text(sample_headers[column_index_int])
                if normalized_header:
                    pairs.append((normalized_header, column_type_int, base_weight))
        return pairs

    if isinstance(field_mappings, dict):
        for field_name, header_name in field_mappings.items():
            normalized_field = _normalize_import_suggestion_text(field_name)
            column_type_int = LEGACY_IMPORT_FIELD_TO_COLUMN_TYPE.get(normalized_field)
            normalized_header = _normalize_import_suggestion_text(header_name)
            if column_type_int and normalized_header:
                pairs.append((normalized_header, column_type_int, base_weight))

    return pairs


def _build_auto_transaction_type_mapping(sample_rows: list[list[Any]], type_column_index: int | None) -> dict[str, int]:
    """根据样本行自动推断交易类型映射。"""
    if type_column_index is None:
        return {}

    result: dict[str, int] = {}
    for row in sample_rows[:100]:
        if not isinstance(row, list) or type_column_index >= len(row):
            continue
        raw_value = str(row[type_column_index] or "").strip()
        if not raw_value or raw_value in result:
            continue
        mapped_type = AUTO_TRANSACTION_TYPE_MAPPING.get(raw_value.lower())
        if mapped_type:
            result[raw_value] = mapped_type
    return result


def _build_import_mapping_suggestion(
    headers: list[Any], configs: list[dict[str, Any]], sample_rows: list[list[Any]] | None = None
) -> dict[str, Any]:
    """基于表头关键词和历史模板构建列映射建议。"""
    normalized_headers = [_normalize_import_suggestion_text(header) for header in headers]
    historical_scores: dict[tuple[int, int], float] = {}

    for config in configs:
        for normalized_header, column_type, weight in _extract_import_config_header_type_pairs(config):
            for index, incoming_header in enumerate(normalized_headers):
                if incoming_header and incoming_header == normalized_header:
                    historical_scores[(column_type, index)] = historical_scores.get((column_type, index), 0.0) + weight

    candidates: list[tuple[float, int, int]] = []
    for index, normalized_header in enumerate(normalized_headers):
        if not normalized_header:
            continue
        for column_type in IMPORT_COLUMN_TYPE_KEYWORDS:
            score = _score_header_keyword_match(normalized_header, column_type)
            score += historical_scores.get((column_type, index), 0.0)
            if score > 0:
                candidates.append((score, column_type, index))

    candidates.sort(key=lambda item: (-item[0], item[1], item[2]))
    chosen_types = set()
    chosen_indices = set()
    column_mapping: dict[str, int] = {}
    suggestions: list[dict[str, Any]] = []

    for score, column_type, index in candidates:
        if score < IMPORT_HEADER_CANDIDATE_MIN_SCORE:
            continue
        if column_type in chosen_types or index in chosen_indices:
            continue
        chosen_types.add(column_type)
        chosen_indices.add(index)
        column_mapping[str(column_type)] = index
        suggestions.append(
            {"columnType": column_type, "columnIndex": index, "header": headers[index], "score": round(score, 2)}
        )

    transaction_type_mapping = _build_auto_transaction_type_mapping(sample_rows or [], column_mapping.get("3"))

    return {
        "includeHeader": True,
        "columnMapping": column_mapping,
        "transactionTypeMapping": transaction_type_mapping,
        "suggestions": suggestions,
    }


def _is_generic_import_data_like_cell(value: Any) -> bool:
    """判断单元格内容是否更像数据而非表头。"""
    text = str(value or "").strip()
    if not text:
        return False

    normalized = _normalize_import_suggestion_text(text)
    if normalized in AUTO_TRANSACTION_TYPE_MAPPING:
        return True

    amount_candidate = text.replace("¥", "").replace("￥", "").replace(",", "")
    if re.fullmatch(r"[+-]?\d+(?:\.\d+)?", amount_candidate):
        return True

    if re.fullmatch(r"\d{4}[-/.]\d{1,2}[-/.]\d{1,2}(?:\s+\d{1,2}:\d{2}(?::\d{2})?)?", text):
        return True

    if re.fullmatch(r"\d{1,2}[-/.]\d{1,2}[-/.]\d{4}(?:\s+\d{1,2}:\d{2}(?::\d{2})?)?", text):
        return True

    return False


def _score_generic_import_header_row(row: list[Any]) -> tuple[float, set[int]]:
    """为候选表头行打分。"""
    non_empty_cells = [str(cell or "").strip() for cell in row if str(cell or "").strip()]
    if len(non_empty_cells) < 2:
        return -1.0, set()

    matched_types: set[int] = set()
    total_score = 0.0
    data_like_count = 0

    for cell in non_empty_cells:
        normalized = _normalize_import_suggestion_text(cell)
        if not normalized:
            continue

        best_column_type = None
        best_score = 0.0
        for column_type in IMPORT_COLUMN_TYPE_KEYWORDS:
            score = _score_header_keyword_match(normalized, column_type)
            if score > best_score:
                best_score = score
                best_column_type = column_type

        if best_column_type is not None and best_score >= 5.0:
            total_score += best_score + (
                IMPORT_HEADER_MATCH_UNIQUE_BONUS if best_column_type not in matched_types else IMPORT_HEADER_MATCH_REPEAT_BONUS
            )
            matched_types.add(best_column_type)
            continue

        if _is_generic_import_data_like_cell(cell):
            data_like_count += 1

    total_score += len(matched_types) * IMPORT_HEADER_MATCHED_TYPES_WEIGHT
    total_score += min(len(non_empty_cells), IMPORT_HEADER_NON_EMPTY_CELL_CAP) * IMPORT_HEADER_NON_EMPTY_CELL_WEIGHT
    total_score -= data_like_count * IMPORT_HEADER_DATA_LIKE_PENALTY

    if len(matched_types) < IMPORT_HEADER_MIN_MATCHED_TYPES:
        total_score -= IMPORT_HEADER_LOW_MATCH_TYPES_PENALTY

    return total_score, matched_types


def _score_generic_import_data_row(row: list[Any]) -> float:
    """为候选数据行打分，用于辅助判断上一行是否为表头。"""
    non_empty_cells = [str(cell or "").strip() for cell in row if str(cell or "").strip()]
    if not non_empty_cells:
        return 0.0

    data_like_count = sum(1 for cell in non_empty_cells if _is_generic_import_data_like_cell(cell))
    return float(data_like_count) + (0.5 if len(non_empty_cells) >= 3 else 0.0)


def _detect_generic_import_header_row_index(rows: list[list[Any]]) -> int:
    """自动识别通用导入文件的表头行位置。"""
    if not rows:
        return 0

    best_index = 0
    best_score = float("-inf")
    scan_limit = min(len(rows), IMPORT_HEADER_SCAN_LIMIT)

    for index, row in enumerate(rows[:scan_limit]):
        header_score, matched_types = _score_generic_import_header_row(row)
        if len(matched_types) < IMPORT_HEADER_MIN_MATCHED_TYPES and header_score < IMPORT_HEADER_MIN_REVIEW_SCORE:
            continue

        next_row_score = 0.0
        if index + 1 < len(rows):
            next_row_score = _score_generic_import_data_row(rows[index + 1])

        candidate_score = header_score + min(next_row_score, IMPORT_HEADER_NEXT_ROW_SCORE_CAP) * IMPORT_HEADER_NEXT_ROW_WEIGHT
        if candidate_score > best_score:
            best_index = index
            best_score = candidate_score

    if best_score < IMPORT_HEADER_ACCEPT_SCORE:
        return 0

    return best_index


def _trim_generic_import_rows_to_header(rows: list[list[Any]]) -> tuple[list[list[Any]], int]:
    """将二维表裁剪到自动识别出的表头行起始位置。"""
    if not rows:
        return rows, 0

    header_row_index = _detect_generic_import_header_row_index(rows)
    return rows[header_row_index:], header_row_index

__all__ = [name for name in globals() if not name.startswith("__")]
