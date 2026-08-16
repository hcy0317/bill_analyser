import {
    buildImportPreviewSignalViewModelFromSnapshot,
    type ImportPreviewSignalViewModel,
    type ImportPreviewSignalViewModelOptions,
} from './checkDataMatching.ts';
import type { ImportPreviewRecord } from './importPreview.ts';

// Canonical API-row projection shared by compact/mobile consumers. Matching is
// retained for display detail; the snapshot owns all six signal memberships.
export function buildImportPreviewSignalViewModelFromRecord(
    record: ImportPreviewRecord,
    options: ImportPreviewSignalViewModelOptions = {},
): ImportPreviewSignalViewModel {
    const matching = record.matching;
    const llm = matching?.llm;
    return buildImportPreviewSignalViewModelFromSnapshot({
        parserId: record.preview_parser_id || matching?.parser?.id,
        parserTags: record.preview_parser_tags || matching?.parser?.tags,
        dedupType: record.dedup_type || matching?.dedup?.type,
        dedupSourceIds: Array.isArray(record.dedup_source_ids)
            ? record.dedup_source_ids
            : matching?.dedup?.source_ids,
        dedupSourceCount: matching?.dedup?.source_count,
        dedupSourceLabels: matching?.dedup?.source_labels,
        dedupSources: matching?.dedup?.sources,
        parserIdChain: matching?.parser?.source_chain,
        reconciliationType: matching?.reconciliation?.candidate_type,
        reconciliationStatus: matching?.reconciliation?.status,
        reconciliationTitle: matching?.reconciliation?.signal_label,
        reconciliationSourceChain: matching?.reconciliation?.source_chain,
        reconciliationPlannedOperation: matching?.reconciliation?.planned_operation,
        reconciliationHistoryBillId: matching?.reconciliation?.history_bill_id,
        reconciliationHistoryBillVersion: matching?.reconciliation?.history_bill_version,
        reconciliationHistorySummary: matching?.reconciliation?.history_summary,
        reconciliationOperationId: matching?.reconciliation?.operation_id,
        reconciliationAcknowledgementToken: matching?.reconciliation?.acknowledgement_token,
        reconciliationDestructiveAckRequired: !!matching?.reconciliation?.destructive_ack_required,
        reconciliationNotice: matching?.reconciliation?.notice
            || matching?.annotation?.history_rewrite_notice,
        transferStatus: matching?.transfer?.review_status,
        transferTitle: record.transfer_suggestion_reason || matching?.transfer?.reason,
        transferCandidateType: matching?.transfer?.candidate_type,
        transferLearningLevel: matching?.transfer?.learning_level
            || matching?.transfer?.level
            || record.transfer_suggestion_level,
        transferSuppressed: matching?.transfer?.suppressed,
        transferPairOrder: matching?.transfer?.pair_order,
        transferSourceChain: matching?.transfer?.source_chain,
        learningStatus: matching?.learning?.review_status,
        learningTitle: record.learning_recommendation_reason || matching?.learning?.reason,
        learningSummary: record.learning_recommendation_summary || matching?.learning?.summary,
        learningMode: matching?.learning?.mode,
        learningLifecycleStatus: matching?.learning?.lifecycle_status,
        learningSignalState: matching?.learning?.signal_state,
        learningAutoApplied: matching?.learning?.auto_apply,
        learningSuppressed: matching?.learning?.suppressed,
        learningRuleId: matching?.learning?.rule_id,
        learningScore: matching?.learning?.score ?? record.learning_recommendation_score,
        learningConfidence: matching?.learning?.confidence,
        learningMargin: matching?.learning?.margin,
        learningAcceptedCount: matching?.learning?.accepted_count,
        learningRejectedCount: matching?.learning?.rejected_count,
        learningAutoAppliedCount: matching?.learning?.auto_applied_count,
        llmStatus: llm?.review_status,
        llmTitle: llm?.reason,
        llmConfidence: llm?.confidence,
        llmSuggestedCategoryId: llm?.suggested_category_id,
        llmCategoryPath: [llm?.suggested_main_category, llm?.suggested_sub_category]
            .filter(Boolean)
            .join('/'),
        llmSourceAccount: llm?.suggested_source_account,
        llmDestinationAccount: llm?.suggested_destination_account,
        llmLifecycleStatus: llm?.lifecycle_status,
        llmSignalState: llm?.signal_state,
        llmSuppressed: llm?.suppressed,
        hasRecurringMatch: !!record.preview_recurring_id,
        recurringCandidateCount: Number(
            record.preview_recurring_candidate_count
            || matching?.recurring?.candidate_count
            || 0,
        ),
        recurringTitle: record.preview_recurring_name || matching?.recurring?.name || '',
        recurringPrimaryReason: record.preview_recurring_match_reasons
            || matching?.recurring?.match_reasons
            || '',
    }, record.preview_state, options);
}
