"""Learning similarity/model signal builders plus transfer/investment preview signal projection."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    Database,
    FEATURE_SCHEMA_VERSION,
    MODEL_KEY,
    POLICY_VERSION,
    SequenceMatcher,
    build_feature_payload,
    evaluate_learning_policy,
    json,
    parse_route_label,
    parse_semantic_label,
    predict_dual_head,
    re,
)
from .import_learning_rules import ImportLearningRulesMixin

class ImportLearningSignalsMixin:
    """Learning similarity/model signal builders plus transfer/investment preview signal projection."""

    @staticmethod
    def _deserialize_learning_match_features(rule: dict[str, Any]) -> dict[str, str]:
        """读取长期学习规则中的结构化匹配特征。"""
        raw_payload = rule.get("match_features_json")
        if not raw_payload:
            return {}

        try:
            payload = json.loads(raw_payload)
        except (TypeError, ValueError, json.JSONDecodeError):
            return {}

        if not isinstance(payload, dict):
            return {}

        normalized_payload: dict[str, str] = {}
        for key, value in payload.items():
            normalized_value = ImportLearningRulesMixin._normalize_learning_text(value)
            if normalized_value:
                normalized_payload[str(key)] = normalized_value

        return normalized_payload

    @staticmethod
    def _calculate_learning_feature_similarity(field: str, preview_value: str, rule_value: str) -> float:
        """计算单字段相似度分数。"""
        if not preview_value or not rule_value:
            return 0.0

        if preview_value == rule_value:
            return 1.0

        if field == "parser_id":
            return 0.0

        if preview_value in rule_value or rule_value in preview_value:
            return 0.92

        delimiter_pattern = r"[\s|,，/、_\-]+"
        preview_parts = {part for part in re.split(delimiter_pattern, preview_value) if part}
        rule_parts = {part for part in re.split(delimiter_pattern, rule_value) if part}

        overlap_score = 0.0
        if preview_parts and rule_parts:
            union = preview_parts | rule_parts
            overlap_score = len(preview_parts & rule_parts) / len(union) if union else 0.0

        sequence_score = SequenceMatcher(None, preview_value, rule_value).ratio()
        return max(overlap_score, sequence_score)

    @staticmethod
    def _score_learning_rule_similarity(
        preview_features: dict[str, str], rule_features: dict[str, str]
    ) -> dict[str, Any] | None:
        """使用加权规则为长期学习候选生成相似度分数。"""
        feature_weights = {
            "parser_id": 0.35,
            "counterparty": 0.30,
            "description": 0.20,
            "payment_method": 0.15,
        }
        preview_feature_keys = [field for field in feature_weights if preview_features.get(field)]
        if len(preview_feature_keys) < 2:
            return None

        total_possible_weight = sum(feature_weights[field] for field in preview_feature_keys)
        weighted_score = 0.0
        matched_fields: list[str] = []
        reason_parts: list[str] = []

        for field in preview_feature_keys:
            preview_value = preview_features.get(field, "")
            rule_value = rule_features.get(field, "")
            if not rule_value:
                continue

            similarity = ImportLearningSignalsMixin._calculate_learning_feature_similarity(
                field, preview_value, rule_value
            )
            if similarity <= 0:
                continue

            weighted_score += feature_weights[field] * similarity
            if similarity >= 0.8:
                matched_fields.append(field)
                match_label = "exact" if similarity >= 0.999 else f"similar({similarity:.2f})"
                reason_parts.append(f"{field}:{match_label}")

        if len(matched_fields) < 2 or total_possible_weight <= 0:
            return None

        return {
            "score": round(weighted_score / total_possible_weight, 2),
            "matched_fields": matched_fields,
            "reason_parts": reason_parts,
        }

    @staticmethod
    def _build_learning_rule_result_summary(
        rule: dict[str, Any], categories_by_id: dict[int, dict[str, Any]], accounts_by_id: dict[int, dict[str, Any]]
    ) -> str:
        """构建长期学习推荐结果摘要。"""
        parts: list[str] = []

        learned_type = str(rule.get("learned_type") or "").strip()
        if learned_type:
            parts.append(learned_type)

        learned_category_id = rule.get("learned_category_id")
        category = categories_by_id.get(int(learned_category_id)) if learned_category_id else None
        if category:
            main_category = str(category.get("main_category") or "").strip()
            sub_category = str(category.get("sub_category") or "").strip()
            if main_category and sub_category:
                parts.append(f"{main_category}/{sub_category}")
            elif main_category:
                parts.append(main_category)

        learned_source_account_id = rule.get("learned_source_account_id")
        learned_destination_account_id = rule.get("learned_destination_account_id")
        source_account = accounts_by_id.get(int(learned_source_account_id)) if learned_source_account_id else None
        destination_account = (
            accounts_by_id.get(int(learned_destination_account_id)) if learned_destination_account_id else None
        )
        source_account_name = str(source_account.get("name") or "").strip() if source_account else ""
        destination_account_name = (
            str(destination_account.get("name") or "").strip() if destination_account else ""
        )
        learned_type_key = learned_type.lower()
        if source_account_name or destination_account_name:
            if learned_type_key in {"转账", "投资", "transfer", "investment"}:
                if source_account_name and destination_account_name:
                    parts.append(f"{source_account_name} → {destination_account_name}")
                else:
                    parts.append(source_account_name or destination_account_name)
            else:
                parts.append(source_account_name or destination_account_name)

        return " | ".join(parts)

    @staticmethod
    def _learning_exact_rule_matches_preview(
        preview: dict[str, Any],
        learning_rules: list[dict[str, Any]],
    ) -> bool:
        preview_composite_hash = Database.build_composite_match_hash(
            parser_id=preview.get("preview_parser_id", ""),
            counterparty=preview.get("preview_counterparty", ""),
            description=preview.get("preview_description", ""),
            payment_method=preview.get("preview_payment_method", ""),
        )
        if not preview_composite_hash:
            return False
        return any(
            str(rule.get("composite_match_hash") or "") == preview_composite_hash
            for rule in learning_rules
        )

    @staticmethod
    def _build_learning_model_feature_row(preview: dict[str, Any]) -> dict[str, Any]:
        return {
            "parser_id": preview.get("preview_parser_id", ""),
            "counterparty": preview.get("preview_counterparty", ""),
            "description": preview.get("preview_description", ""),
            "payment_method": preview.get("preview_payment_method", ""),
            "source_snapshot_json": json.dumps(
                {
                    "preview_amount": preview.get("preview_amount"),
                    "preview_type": preview.get("preview_type"),
                },
                ensure_ascii=False,
                sort_keys=True,
            ),
        }

    @staticmethod
    def _build_learning_model_result_summary(
        recommendation: dict[str, Any],
        categories_by_id: dict[int, dict[str, Any]],
        accounts_by_id: dict[int, dict[str, Any]],
    ) -> str:
        return ImportLearningSignalsMixin._build_learning_rule_result_summary(
            {
                "learned_type": recommendation.get("recommended_type"),
                "learned_category_id": recommendation.get("category_id"),
                "learned_source_account_id": recommendation.get("source_account_id"),
                "learned_destination_account_id": recommendation.get("destination_account_id"),
            },
            categories_by_id,
            accounts_by_id,
        )

    @staticmethod
    def _build_preview_learning_model_apply_payload(recommendation: dict[str, Any]) -> dict[str, Any]:
        applied_result: dict[str, Any] = {}
        recommended_type = str(recommendation.get("recommended_type") or "").strip()
        if recommended_type:
            applied_result["preview_type"] = recommended_type

        category_id = recommendation.get("category_id")
        if category_id not in (None, "", 0, "0"):
            applied_result["preview_main_category"] = str(recommendation.get("main_category") or "")
            applied_result["preview_sub_category"] = str(recommendation.get("sub_category") or "")

        source_account_id = recommendation.get("source_account_id")
        if source_account_id not in (None, "", 0, "0"):
            applied_result["preview_source_account_id"] = int(source_account_id)

        destination_account_id = recommendation.get("destination_account_id")
        if destination_account_id not in (None, "", 0, "0"):
            applied_result["preview_destination_account_id"] = int(destination_account_id)

        return applied_result

    def _build_learning_model_signal_from_preview(
        self,
        preview: dict[str, Any],
        active_model: dict[str, Any] | None,
        *,
        learning_rules: list[dict[str, Any]],
        categories_by_id: dict[int, dict[str, Any]],
        accounts_by_id: dict[int, dict[str, Any]],
        is_manually_annotated: bool,
    ) -> dict[str, Any]:
        """Use the active trainable model to produce green/blue preview recommendations."""
        if not active_model:
            return {}
        if self._learning_exact_rule_matches_preview(preview, learning_rules):
            return {}

        metrics_payload = dict(active_model.get("metrics") or {})
        if str(metrics_payload.get("feature_schema_version") or "") != FEATURE_SCHEMA_VERSION:
            return {}
        if str(metrics_payload.get("policy_version") or "") != POLICY_VERSION:
            return {}
        model_parameters = metrics_payload.get("model_parameters")
        if not isinstance(model_parameters, dict):
            return {}

        feature_row = self._build_learning_model_feature_row(preview)
        prediction = predict_dual_head(model_parameters, build_feature_payload(feature_row))
        if not prediction:
            return {}

        semantic_result = parse_semantic_label(prediction.semantic_label)
        route_result = parse_route_label(prediction.route_label)
        recommended_type = str(semantic_result.get("type") or "").strip()
        category_id = self._coerce_optional_positive_int(semantic_result.get("category_id"))
        source_account_id = self._coerce_optional_positive_int(route_result.get("source_account_id"))
        destination_account_id = self._coerce_optional_positive_int(
            route_result.get("destination_account_id")
        )
        if not recommended_type and not category_id and not source_account_id and not destination_account_id:
            return {}
        if category_id is not None and category_id not in categories_by_id:
            return {}
        if source_account_id is not None and source_account_id not in accounts_by_id:
            return {}
        if destination_account_id is not None and destination_account_id not in accounts_by_id:
            return {}

        confirmation_key = f"{prediction.semantic_label}||{prediction.route_label}"
        confirmation_counts = dict(metrics_payload.get("joint_label_confirmation_counts") or {})
        confirmation_count = int(confirmation_counts.get(confirmation_key) or 0)
        conflict_reasons = ["manual_annotation"] if is_manually_annotated else []
        policy_decision = evaluate_learning_policy(
            prediction,
            confirmation_count=confirmation_count,
            conflict_reasons=conflict_reasons,
        )
        if policy_decision.mode == "none":
            return {}

        category = categories_by_id.get(int(category_id)) if category_id else None
        source_account = accounts_by_id.get(int(source_account_id)) if source_account_id else None
        destination_account = accounts_by_id.get(int(destination_account_id)) if destination_account_id else None
        recommendation = {
            "rule_id": None,
            "source": "model",
            "mode": policy_decision.mode,
            "auto_apply": policy_decision.auto_apply,
            "score": policy_decision.score,
            "confidence": policy_decision.confidence,
            "margin": policy_decision.margin,
            "confirmations": confirmation_count,
            "level": policy_decision.level,
            "model_key": MODEL_KEY,
            "model_version": str(active_model.get("model_version") or ""),
            "dataset_snapshot_id": active_model.get("dataset_snapshot_id"),
            "feature_schema_version": FEATURE_SCHEMA_VERSION,
            "policy_version": POLICY_VERSION,
            "recommended_type": recommended_type,
            "category_id": category_id,
            "source_account_id": source_account_id,
            "destination_account_id": destination_account_id,
            "main_category": str(category.get("main_category") or "") if category else "",
            "sub_category": str(category.get("sub_category") or "") if category else "",
            "source_account_name": str(source_account.get("name") or "") if source_account else "",
            "destination_account_name": str(destination_account.get("name") or "") if destination_account else "",
        }
        recommendation["summary"] = self._build_learning_model_result_summary(
            recommendation,
            categories_by_id,
            accounts_by_id,
        )
        reason_parts = [
            "model:dual_head",
            f"version:{recommendation['model_version']}",
            f"semantic:{prediction.semantic_confidence:.2f}/{prediction.semantic_margin:.2f}",
            f"route:{prediction.route_confidence:.2f}/{prediction.route_margin:.2f}",
            f"confirmations:{confirmation_count}",
        ]
        if policy_decision.rejection_reasons:
            reason_parts.append("gate:" + "/".join(policy_decision.rejection_reasons))
        recommendation["reason"] = ", ".join(reason_parts)
        return recommendation

    def _build_learning_similarity_signal_from_preview(
        self,
        preview: dict[str, Any],
        learning_rules: list[dict[str, Any]],
        *,
        categories_by_id: dict[int, dict[str, Any]],
        accounts_by_id: dict[int, dict[str, Any]],
    ) -> dict[str, Any]:
        """基于长期学习规则为预览账单生成相似度推荐信号。"""
        if not learning_rules:
            return {}

        preview_features = self.db.build_composite_match_features(
            parser_id=preview.get("preview_parser_id", ""),
            counterparty=preview.get("preview_counterparty", ""),
            description=preview.get("preview_description", ""),
            payment_method=preview.get("preview_payment_method", ""),
        )
        if not preview_features:
            return {}

        preview_composite_hash = self.db.build_composite_match_hash(
            parser_id=preview.get("preview_parser_id", ""),
            counterparty=preview.get("preview_counterparty", ""),
            description=preview.get("preview_description", ""),
            payment_method=preview.get("preview_payment_method", ""),
        )
        if preview_composite_hash and any(
            str(rule.get("composite_match_hash") or "") == preview_composite_hash for rule in learning_rules
        ):
            return {}

        candidates: list[dict[str, Any]] = []
        preview_parser_id = preview_features.get("parser_id", "")
        for rule in learning_rules:
            rule_features = self._deserialize_learning_match_features(rule)
            if not rule_features:
                continue

            rule_parser_id = rule_features.get("parser_id", "")
            if preview_parser_id and rule_parser_id and preview_parser_id != rule_parser_id:
                continue

            score_payload = self._score_learning_rule_similarity(preview_features, rule_features)
            if not score_payload:
                continue

            candidates.append(
                {
                    "rule": rule,
                    "score": score_payload["score"],
                    "matched_fields": score_payload["matched_fields"],
                    "reason_parts": score_payload["reason_parts"],
                }
            )

        if not candidates:
            return {}

        candidates.sort(
            key=lambda candidate: (
                float(candidate["score"]),
                len(candidate["matched_fields"]),
                int(candidate["rule"].get("applied_count", 0) or 0),
                int(candidate["rule"].get("id", 0) or 0),
            ),
            reverse=True,
        )
        best_candidate = candidates[0]
        second_score = float(candidates[1]["score"]) if len(candidates) > 1 else 0.0
        margin = round(float(best_candidate["score"]) - second_score, 2)

        if float(best_candidate["score"]) < 0.72:
            return {}
        if len(candidates) > 1 and margin < 0.08:
            return {}

        if float(best_candidate["score"]) >= 0.9:
            level = "high"
        elif float(best_candidate["score"]) >= 0.82:
            level = "medium"
        else:
            level = "low"

        best_rule = best_candidate["rule"]
        summary = self._build_learning_rule_result_summary(best_rule, categories_by_id, accounts_by_id)
        reason_parts = [*best_candidate["reason_parts"], f"margin:{margin:.2f}"]

        self.logger.debug(
            "[长期学习相似推荐] preview_id=%s, rule_id=%s, score=%.2f, level=%s, reasons=%s",
            preview.get("id"),
            best_rule.get("id"),
            float(best_candidate["score"]),
            level,
            ", ".join(reason_parts),
        )
        return {
            "rule_id": best_rule.get("id"),
            "score": float(best_candidate["score"]),
            "level": level,
            "reason": ", ".join(reason_parts),
            "recommended_type": str(best_rule.get("learned_type") or "").strip(),
            "summary": summary,
        }

    def _build_transfer_suggestion_from_preview(self, preview: dict[str, Any]) -> dict[str, Any]:
        """根据预览账单生成疑似转账推荐。

        该推荐是一个轻量级 M2 切片：
        - 不修改数据库结构
        - 仅在读取预览时基于现有字段计算推荐分数
        - 主要用于提醒用户将可疑的收支账单快速切换为转账类型
        """
        preview_type = str(preview.get("preview_type", "") or "").strip().lower()
        if preview_type in ["转账", "transfer", "4", "投资", "investment", "5"]:
            return {}

        score = 0.0
        reasons: list[str] = []

        source_account_id = preview.get("preview_source_account_id")
        destination_account_id = preview.get("preview_destination_account_id")
        destination_amount = float(preview.get("preview_destination_amount", 0) or 0)
        dedup_type = str(preview.get("dedup_type", "") or "").strip().lower()

        category_text = " ".join(
            filter(
                None,
                [
                    str(preview.get("preview_main_category", "") or "").strip(),
                    str(preview.get("preview_sub_category", "") or "").strip(),
                ],
            )
        ).lower()
        text_blob = " ".join(
            filter(
                None,
                [
                    str(preview.get("preview_counterparty", "") or "").strip(),
                    str(preview.get("preview_payment_method", "") or "").strip(),
                    str(preview.get("preview_description", "") or "").strip(),
                    category_text,
                ],
            )
        ).lower()

        transfer_keywords = ["转账", "转入", "转出", "还款", "充值", "提现", "存取", "存款", "取款", "划转"]

        if dedup_type == "transfer":
            score += 0.75
            reasons.append("dedup_pair")

        if source_account_id and destination_account_id and str(source_account_id) != str(destination_account_id):
            score += 0.35
            reasons.append("dual_account")

        if destination_amount > 0:
            score += 0.20
            reasons.append("destination_amount")

        if any(keyword in category_text for keyword in ["转账", "存取", "还款"]):
            score += 0.20
            reasons.append("transfer_category")

        matched_keywords = [keyword for keyword in transfer_keywords if keyword in text_blob]
        if matched_keywords:
            score += min(0.30, 0.12 * len(matched_keywords))
            reasons.append("keyword:" + "/".join(matched_keywords[:3]))

        score = min(score, 1.0)
        if score < 0.55:
            return {}

        if score >= 0.8:
            level = "high"
        elif score >= 0.65:
            level = "medium"
        else:
            level = "low"

        reason_text = ", ".join(reasons)
        self.logger.debug(
            "[预览转账推荐] preview_id=%s, score=%.2f, level=%s, reasons=%s",
            preview.get("id"),
            score,
            level,
            reason_text,
        )
        return {"suggested_preview_type": "转账", "score": round(score, 2), "level": level, "reason": reason_text}

    def _build_investment_signal_from_preview(
        self, preview: dict[str, Any], keyword_config: dict[str, list[str]] | None = None
    ) -> dict[str, Any]:
        """Project canonical preview investment classification into matching state."""
        _ = keyword_config
        preview_type = str(preview.get("preview_type", "") or "").strip().lower()
        if preview_type not in ["投资", "investment", "5"]:
            return {}

        return {
            "score": 1.0,
            "level": "high",
            "reason": "category_rule",
            "platform": "",
            "product": "",
        }
