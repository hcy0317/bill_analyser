impl SmartDeduplicationEngine {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn process(&self, bills: Vec<DedupBill>) -> DeduplicationResult {
        let original_count = bills.len();
        let mut bills = bills;
        let mut duplicate_groups = Vec::new();
        let mut transfer_pairs = Vec::new();
        let mut split_groups = Vec::new();

        duplicate_groups.extend(find_exact_duplicates(&mut bills));
        duplicate_groups.extend(find_platform_bank_duplicates(&mut bills));
        duplicate_groups.extend(find_same_batch_duplicates(&mut bills));
        transfer_pairs.extend(find_transfer_pairs(&mut bills));
        duplicate_groups.extend(find_similar_duplicates(&mut bills));
        split_groups.extend(find_split_bills(&mut bills));

        let kept_bills: Vec<DedupBill> = bills
            .into_iter()
            .filter(|bill| !bill.removed)
            .map(clean_runtime_markers)
            .collect();
        let removed_count = original_count.saturating_sub(kept_bills.len());

        DeduplicationResult {
            original_count,
            kept_bills,
            removed_count,
            duplicate_groups,
            transfer_pairs,
            split_groups,
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn find_database_duplicates(
    imported_bills: &mut [DedupBill],
    existing_bills: &[DedupBill],
) -> Vec<DatabaseDuplicateMatch> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "find_database_duplicates",
        "business operation entered"
    );
    let mut matches = Vec::new();

    for (imported_index, imported_bill) in imported_bills.iter_mut().enumerate() {
        if imported_bill.removed {
            continue;
        }
        let Some(imported_dt) = bill_datetime(imported_bill) else {
            continue;
        };
        for (existing_index, existing_bill) in existing_bills.iter().enumerate() {
            let Some(existing_dt) = bill_datetime(existing_bill) else {
                continue;
            };
            if !same_day(imported_dt, existing_dt)
                || !time_close(imported_dt, existing_dt, DATABASE_TIME_TOLERANCE_SECONDS)
            {
                continue;
            }

            let is_platform_bank_pair = is_platform_source(&imported_bill.source_type())
                != is_platform_source(&existing_bill.source_type());
            let amount_match = if is_platform_bank_pair {
                abs_amount_close(imported_bill.amount, existing_bill.amount)
            } else {
                amount_equal_same_direction(imported_bill.amount, existing_bill.amount)
            };
            if !amount_match {
                continue;
            }

            if duplicate_text_match(imported_bill, existing_bill, is_platform_bank_pair) {
                let reason_type = if is_platform_bank_pair {
                    "平台-银行跨文件重复"
                } else {
                    "与数据库已有账单重复"
                };
                imported_bill.removed = true;
                imported_bill.dedup_type =
                    Some(DeduplicationType::DatabaseDuplicate.as_str().to_string());
                imported_bill.duplicate_of_db_id = existing_bill.id.clone();
                matches.push(DatabaseDuplicateMatch {
                    imported_index,
                    existing_index,
                    existing_bill_id: existing_bill.id.clone(),
                    is_platform_bank_pair,
                    dedup_type: DeduplicationType::DatabaseDuplicate,
                    reason: format!(
                        "{reason_type} (ID={}, 日期={}, 金额={})",
                        existing_bill.id.as_deref().unwrap_or_default(),
                        existing_bill.date,
                        existing_bill.amount.to_yuan_string()
                    ),
                });
                break;
            }
        }
    }

    matches
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn find_cross_batch_transfer_pairs(
    imported_bills: &mut [DedupBill],
    existing_bills: &[DedupBill],
) -> Vec<CrossBatchTransferMatch> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "find_cross_batch_transfer_pairs",
        "business operation entered"
    );
    let mut matches = Vec::new();
    let mut matched_existing_ids = HashSet::new();

    for (imported_index, imported_bill) in imported_bills.iter_mut().enumerate() {
        if imported_bill.removed || imported_bill.dedup_type.as_deref() == Some("transfer") {
            continue;
        }
        let Some(imported_dt) = bill_datetime(imported_bill) else {
            continue;
        };
        for (existing_index, existing_bill) in existing_bills.iter().enumerate() {
            let existing_match_key = existing_bill
                .id
                .clone()
                .unwrap_or_else(|| existing_index.to_string());
            if matched_existing_ids.contains(&existing_match_key) {
                continue;
            }

            let Some(existing_dt) = bill_datetime(existing_bill) else {
                continue;
            };
            if !same_day(imported_dt, existing_dt)
                || !time_close(imported_dt, existing_dt, DATABASE_TIME_TOLERANCE_SECONDS)
            {
                continue;
            }
            if !amount_opposite(imported_bill.amount, existing_bill.amount) {
                continue;
            }

            if !has_distinct_reconciliation_transfer_sources(imported_bill, existing_bill) {
                continue;
            }

            matched_existing_ids.insert(existing_match_key);
            imported_bill.transaction_type = "转账".to_string();
            imported_bill.dedup_type = Some("transfer_cross_batch".to_string());
            imported_bill.cross_batch_db_id = existing_bill.id.clone();
            imported_bill.transfer_pair_order = Some("outgoing_import".to_string());
            imported_bill.transfer_pair_sources = vec![
                build_transfer_source_snapshot(imported_bill, "outgoing"),
                build_transfer_source_snapshot(existing_bill, "incoming"),
            ];
            imported_bill.destination_parser_id = existing_bill.parser_id.clone();
            imported_bill.destination_payment_method = existing_bill.payment_method.clone();
            imported_bill.destination_counterparty = existing_bill.counterparty.clone();
            imported_bill.destination_account_id =
                Some(existing_bill.source_account_id.clone()).filter(|value| !value.is_empty());
            imported_bill.destination_account_name = first_non_empty([
                existing_bill.account_name.as_str(),
                existing_bill.payment_method.as_str(),
            ]);
            imported_bill.counterparty =
                merge_field_values(&imported_bill.counterparty, &existing_bill.counterparty);
            imported_bill.payment_method =
                merge_field_values(&imported_bill.payment_method, &existing_bill.payment_method);
            imported_bill.description =
                merge_field_values(&imported_bill.description, &existing_bill.description);
            matches.push(CrossBatchTransferMatch {
                imported_index,
                existing_index,
                existing_bill_id: existing_bill.id.clone(),
                dedup_type: "transfer_cross_batch".to_string(),
            });
            break;
        }
    }

    matches
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn find_import_reconciliation_candidates(
    imported_bills: &[DedupBill],
    existing_bills: &[DedupBill],
) -> Vec<ImportReconciliationCandidate> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "find_import_reconciliation_candidates",
        "business operation entered"
    );
    let mut candidates = Vec::new();
    let mut matched_pairs = HashSet::new();

    for imported_bill in imported_bills {
        if imported_bill.skip_reconciliation_candidate {
            continue;
        }
        let Some(imported_dt) = bill_datetime(imported_bill) else {
            continue;
        };
        let import_bill_key = build_reconciliation_import_key(imported_bill);
        for existing_bill in existing_bills {
            let Some(existing_dt) = bill_datetime(existing_bill) else {
                continue;
            };
            if !same_day(imported_dt, existing_dt) {
                continue;
            }
            let time_diff_seconds = (imported_dt - existing_dt).num_seconds().abs();
            if time_diff_seconds > DATABASE_TIME_TOLERANCE_SECONDS {
                continue;
            }

            let Some((candidate_type, reason)) =
                resolve_reconciliation_candidate_type(imported_bill, existing_bill)
            else {
                continue;
            };
            let tolerance_seconds = reconciliation_candidate_time_tolerance_seconds(candidate_type);
            if time_diff_seconds > tolerance_seconds {
                continue;
            }
            let existing_bill_id = existing_bill.id.clone();
            let pair_key = format!(
                "{}|{}|{}",
                candidate_type.as_str(),
                existing_bill_id.as_deref().unwrap_or_default(),
                import_bill_key
            );
            if !matched_pairs.insert(pair_key) {
                continue;
            }

            let amount_abs = Money::from_cents(abs_cents(imported_bill.amount) as i64);
            let day_key = imported_dt.date().format("%Y-%m-%d").to_string();
            let group_key = format!(
                "import_reconciliation:{}:bill:{}:amount:{}:{}",
                candidate_type.as_str(),
                existing_bill_id.as_deref().unwrap_or_default(),
                amount_abs.to_yuan_string(),
                day_key
            );
            let time_score_percent =
                100 - ((time_diff_seconds * 100) / tolerance_seconds.max(1)).clamp(0, 100) as u8;
            let score_percent = 80 + ((19 * u16::from(time_score_percent)) / 100) as u8;
            candidates.push(ImportReconciliationCandidate {
                candidate_id: build_reconciliation_candidate_id(
                    candidate_type,
                    existing_bill_id.as_deref().unwrap_or_default(),
                    &import_bill_key,
                ),
                candidate_type,
                import_bill_key: import_bill_key.clone(),
                existing_bill_id,
                group_key,
                amount_abs,
                time_diff_seconds,
                score_percent,
                level: if time_diff_seconds <= TIME_TOLERANCE_SECONDS {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
                reason,
            });
        }
    }

    candidates
}
