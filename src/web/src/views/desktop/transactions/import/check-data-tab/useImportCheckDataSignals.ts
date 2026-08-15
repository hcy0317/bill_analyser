import { computed, type ComputedRef } from 'vue';

import type { ImportMatchingSourcePayload } from '@/models/import_matching.ts';
import type { ImportTransaction } from '@/models/imported_transaction.ts';

import type { ImportPreviewLLMMatchingPayload } from '../importPreview.ts';
import {
    buildImportPreviewHistoryRewriteOperationAcknowledgement,
    buildImportPreviewSignalViewModel,
    normalizeImportPreviewSignalStatusAlias,
    resolveImportPreviewSignalStatus,
    type ImportCheckMatchingSourceContext,
    type ImportPreviewHistoryRewriteAcknowledgementOperation,
    type ImportPreviewSignalStatus,
    type ImportPreviewSignalViewModel,
    type ImportPreviewSignalViewModelOptions
} from '../checkDataMatching.ts';
import {
    getImportPreviewTransferSignalStatus,
    getImportPreviewTransferSignalTitle
} from '../importPreviewSignalAdapter.ts';

const PARSER_LABELS: Record<string, string> = {
    wechat: '微信',
    alipay: '支付宝',
    icbc: '工商银行',
    cmbc: '民生银行',
    abc: '农业银行',
    ccb: '建设银行',
    generic: '通用'
};

const PARSER_COLORS: Record<string, string> = {
    wechat: 'green',
    alipay: 'blue',
    icbc: 'red',
    cmbc: 'orange',
    abc: 'teal',
    ccb: 'indigo',
    generic: 'grey'
};

export function resolveSignalStatusAuthority(...values: unknown[]): {
    status: ImportPreviewSignalStatus | string | null | undefined;
    authoritative: boolean;
} {
    const status = normalizeImportPreviewSignalStatusAlias(...values);
    if (status !== null) {
        return { status, authoritative: true };
    }

    const hasExplicitNull = values.some(value => value === null);
    return {
        status: hasExplicitNull ? null : undefined,
        authoritative: hasExplicitNull
    };
}

export function serializeSignalCacheValue(value: unknown): string {
    if (value === null) {
        return '<null>';
    }
    if (typeof value === 'undefined') {
        return '<undefined>';
    }
    return String(value);
}

export interface ImportCheckDataSignalsOptions {
    importTransactions: ComputedRef<ImportTransaction[]>;
    translate: (key: string) => string;
    getPreviewId: (item: ImportTransaction) => number | null;
    getLLMMatchingPayload: (item: ImportTransaction) => ImportPreviewLLMMatchingPayload;
    getLLMSignalStatus: (item: ImportTransaction) => ImportPreviewSignalStatus | null;
    getLLMSignalCategoryPath: (payload: ImportPreviewLLMMatchingPayload) => string;
    buildLLMSignalSummary: (payload: ImportPreviewLLMMatchingPayload) => string;
    getRecurringMatchSummary: (item: ImportTransaction) => string;
    getPrimaryRecurringReason: (item: ImportTransaction) => string;
    formatAmountWithCurrency: (amountCents: number, currency: string) => string;
}

export function useImportCheckDataSignals(options: ImportCheckDataSignalsOptions) {
    function getLearningSignalStatus(item: ImportTransaction): ImportPreviewSignalStatus | null {
        if (!item.hasLearningRecommendation()) {
            return null;
        }
        if (item.hasPendingLearningRecommendation()) {
            return 'pending';
        }
        if (item.isLearningRecommendationAccepted()) {
            return 'accepted';
        }
        if (item.isLearningRecommendationRejected()) {
            return 'rejected';
        }
        if (item.isLearningRecommendationSkipped()) {
            return 'skipped';
        }
        return null;
    }

    const importPreviewSignalSourceContext = computed<{
        sourceRowLookup: Map<string, ImportCheckMatchingSourceContext>;
        version: string;
    }>(() => {
        const sourceRowLookup = new Map<string, ImportCheckMatchingSourceContext>();
        const versionParts: string[] = [];

        for (const item of options.importTransactions.value) {
            const parserId = (item.parserId || '').trim();
            const parserTags = Array.isArray(item.parserTags) ? item.parserTags : [];
            if (!parserId && parserTags.length < 1) {
                continue;
            }

            const previewId = options.getPreviewId(item);
            const sourceContext = { parserId, parserTags };
            sourceRowLookup.set(String(item.index), sourceContext);
            if (previewId !== null) {
                sourceRowLookup.set(String(previewId), sourceContext);
            }
            versionParts.push([
                item.index,
                previewId ?? '',
                parserId,
                parserTags.join('|')
            ].join('::'));
        }

        return {
            sourceRowLookup,
            version: versionParts.join('\u001f')
        };
    });

    const importPreviewSignalSharedContext = computed<{
        options: Omit<ImportPreviewSignalViewModelOptions, 'currentParserId' | 'sourceRowLookup'>;
        version: string;
    }>(() => {
        const sharedOptions = {
            matchLabel: options.translate('Matching'),
            parserLabels: PARSER_LABELS,
            parserColors: PARSER_COLORS,
            dedupLabels: {
                'Transfer Match': options.translate('Transfer Match'),
                'Platform Duplicate': options.translate('Platform Duplicate'),
                'Platform-Bank Duplicate': options.translate('Platform-Bank Duplicate'),
                'Similar Duplicate': options.translate('Similar Duplicate'),
                'Split-Merge Duplicate': options.translate('Split-Merge Duplicate'),
                'Cross-Batch Transfer': options.translate('Cross-Batch Transfer')
            },
            sourceRoleLabels: {
                outgoing: options.translate('Outgoing'),
                incoming: options.translate('Incoming'),
                debit: options.translate('Outgoing'),
                credit: options.translate('Incoming')
            },
            infoLabels: {
                sourceLabel: options.translate('Source'),
                duplicateSourcesLabel: options.translate('Duplicate Sources'),
                recommendedCategoryLabel: options.translate('Recommended Category'),
                accountRouteLabel: options.translate('Account Route')
            }
        } satisfies Omit<ImportPreviewSignalViewModelOptions, 'currentParserId' | 'sourceRowLookup'>;

        return {
            options: sharedOptions,
            version: [
                sharedOptions.matchLabel,
                sharedOptions.dedupLabels['Transfer Match'],
                sharedOptions.dedupLabels['Platform Duplicate'],
                sharedOptions.dedupLabels['Platform-Bank Duplicate'],
                sharedOptions.dedupLabels['Similar Duplicate'],
                sharedOptions.dedupLabels['Split-Merge Duplicate'],
                sharedOptions.dedupLabels['Cross-Batch Transfer'],
                sharedOptions.sourceRoleLabels.outgoing,
                sharedOptions.sourceRoleLabels.incoming,
                sharedOptions.infoLabels.sourceLabel,
                sharedOptions.infoLabels.duplicateSourcesLabel,
                sharedOptions.infoLabels.recommendedCategoryLabel,
                sharedOptions.infoLabels.accountRouteLabel
            ].join('\u001f')
        };
    });

    const viewModelCache = new WeakMap<ImportTransaction, {
        signature: string;
        viewModel: ImportPreviewSignalViewModel;
    }>();

    function serializeSourceChain(sources: ImportMatchingSourcePayload[] | undefined): string {
        return (sources || []).map(source => [
            source.role || '',
            source.parser_id || '',
            source.parser_label || '',
            source.label || '',
            source.position || ''
        ].join('::')).join('||');
    }

    function buildCacheSignature(item: ImportTransaction): string {
        const dedupSourceIds = Array.isArray(item.dedupSourceIds) ? item.dedupSourceIds : [];
        const dedupSourceLabels = Array.isArray(item.matching?.dedup.source_labels) ? item.matching?.dedup.source_labels : [];
        const parserTags = Array.isArray(item.parserTags) ? item.parserTags : [];
        const learningPayload = item.matching?.learning;
        const llmPayload = options.getLLMMatchingPayload(item);

        return [
            importPreviewSignalSourceContext.value.version,
            importPreviewSignalSharedContext.value.version,
            item.index,
            options.getPreviewId(item) ?? '',
            (item.parserId || '').trim(),
            parserTags.join('|'),
            item.dedupType || '',
            dedupSourceIds.join('|'),
            Number(item.matching?.dedup.source_count || 0),
            dedupSourceLabels.join('|'),
            serializeSourceChain(item.matching?.dedup.sources),
            serializeSourceChain(item.matching?.parser.source_chain),
            item.matching?.reconciliation?.candidate_type || '',
            item.matching?.reconciliation?.status || '',
            item.matching?.reconciliation?.signal_label || '',
            serializeSourceChain(item.matching?.reconciliation?.source_chain),
            item.matching?.reconciliation?.planned_operation || '',
            item.matching?.reconciliation?.history_bill_id || '',
            item.matching?.reconciliation?.history_bill_version || '',
            item.matching?.reconciliation?.history_role || '',
            item.matching?.reconciliation?.group_key || '',
            item.matching?.reconciliation?.operation_id || '',
            item.matching?.reconciliation?.acknowledgement_token || '',
            String(!!item.matching?.reconciliation?.destructive_ack_required),
            item.matching?.reconciliation?.notice || '',
            item.matching?.annotation?.history_rewrite_notice || '',
            JSON.stringify(item.matching?.reconciliation?.history_summary || null),
            String(!!item.isManuallyAnnotated),
            getImportPreviewTransferSignalStatus(item) || '',
            getImportPreviewTransferSignalTitle(item),
            item.matching?.transfer.candidate_type || '',
            item.transferSuggestionLevel || '',
            String(item.matching?.transfer.suppressed ?? ''),
            item.matching?.transfer.pair_order || '',
            serializeSourceChain(item.matching?.transfer.source_chain),
            serializeSignalCacheValue(learningPayload?.review_status),
            serializeSignalCacheValue(learningPayload?.status),
            serializeSignalCacheValue(learningPayload?.lifecycle_status),
            serializeSignalCacheValue(learningPayload?.signal_state),
            String(learningPayload?.reason ?? item.learningRecommendationReason ?? ''),
            String(learningPayload?.summary ?? item.learningRecommendationSummary ?? ''),
            String(learningPayload?.mode ?? ''),
            String(learningPayload?.rule_id ?? ''),
            String(learningPayload?.score ?? item.learningRecommendationScore ?? ''),
            String(learningPayload?.confidence ?? ''),
            String(learningPayload?.margin ?? ''),
            String(learningPayload?.accepted_count ?? ''),
            String(learningPayload?.rejected_count ?? ''),
            String(learningPayload?.auto_applied_count ?? ''),
            String(learningPayload?.suppressed ?? ''),
            String(learningPayload?.auto_apply ?? ''),
            String(learningPayload?.recommendation_key ?? ''),
            options.getLLMSignalStatus(item) || '',
            serializeSignalCacheValue(llmPayload.review_status),
            serializeSignalCacheValue(llmPayload.status),
            serializeSignalCacheValue(llmPayload.lifecycle_status),
            serializeSignalCacheValue(llmPayload.signal_state),
            String(llmPayload.suggested_type || ''),
            String(llmPayload.suggested_category_id ?? ''),
            String(llmPayload.suggested_main_category || ''),
            String(llmPayload.suggested_sub_category || ''),
            String(llmPayload.suggested_source_account || ''),
            String(llmPayload.suggested_destination_account || ''),
            String(llmPayload.reason || ''),
            String(llmPayload.confidence ?? ''),
            String(llmPayload.suppressed ?? ''),
            String(!!item.hasRecurringMatch()),
            options.getRecurringMatchSummary(item),
            Number(item.recurringCandidateCount || 0),
            options.getPrimaryRecurringReason(item)
        ].join('\u001f');
    }

    function getImportPreviewSignalViewModel(item: ImportTransaction): ImportPreviewSignalViewModel {
        const signature = buildCacheSignature(item);
        const cached = viewModelCache.get(item);
        if (cached?.signature === signature) {
            return cached.viewModel;
        }

        const learningPayload = item.matching?.learning;
        const llmPayload = options.getLLMMatchingPayload(item);
        const learningStatusAuthority = resolveSignalStatusAuthority(
            learningPayload?.review_status,
            learningPayload?.status,
            learningPayload?.lifecycle_status,
            learningPayload?.signal_state
        );
        const llmStatusAuthority = resolveSignalStatusAuthority(
            llmPayload.review_status,
            llmPayload.status,
            llmPayload.lifecycle_status,
            llmPayload.signal_state
        );
        const viewModel = buildImportPreviewSignalViewModel({
            parserId: item.parserId,
            parserTags: item.parserTags,
            dedupType: item.dedupType,
            dedupSourceIds: item.dedupSourceIds,
            dedupSourceCount: item.matching?.dedup.source_count,
            dedupSourceLabels: item.matching?.dedup.source_labels,
            dedupSources: item.matching?.dedup.sources,
            parserIdChain: item.matching?.parser.source_chain,
            reconciliationType: item.matching?.reconciliation?.candidate_type,
            reconciliationStatus: resolveImportPreviewSignalStatus(item.matching?.reconciliation),
            reconciliationTitle: item.matching?.reconciliation?.signal_label,
            reconciliationSourceChain: item.matching?.reconciliation?.source_chain,
            reconciliationPlannedOperation: item.matching?.reconciliation?.planned_operation,
            reconciliationHistoryBillId: item.matching?.reconciliation?.history_bill_id,
            reconciliationHistoryBillVersion: item.matching?.reconciliation?.history_bill_version,
            reconciliationHistorySummary: item.matching?.reconciliation?.history_summary,
            reconciliationHistoryRole: item.matching?.reconciliation?.history_role,
            reconciliationGroupKey: item.matching?.reconciliation?.group_key,
            reconciliationOperationId: item.matching?.reconciliation?.operation_id,
            reconciliationAcknowledgementToken: item.matching?.reconciliation?.acknowledgement_token,
            reconciliationDestructiveAckRequired: !!item.matching?.reconciliation?.destructive_ack_required,
            reconciliationNotice: item.matching?.reconciliation?.notice || item.matching?.annotation?.history_rewrite_notice,
            isManuallyAnnotated: item.isManuallyAnnotated,
            transferStatus: getImportPreviewTransferSignalStatus(item),
            transferTitle: getImportPreviewTransferSignalTitle(item),
            transferCandidateType: item.matching?.transfer.candidate_type,
            transferLearningLevel: item.transferSuggestionLevel,
            transferSuppressed: item.matching?.transfer.suppressed,
            transferPairOrder: item.matching?.transfer.pair_order,
            transferSourceChain: item.matching?.transfer.source_chain,
            learningStatus: learningStatusAuthority.authoritative
                ? learningStatusAuthority.status
                : getLearningSignalStatus(item),
            learningTitle: String(learningPayload?.reason ?? item.learningRecommendationReason ?? ''),
            learningSummary: String(learningPayload?.summary ?? item.learningRecommendationSummary ?? ''),
            learningMode: String(learningPayload?.mode || ''),
            learningLifecycleStatus: String(learningPayload?.lifecycle_status || ''),
            learningSignalState: String(learningPayload?.signal_state || ''),
            learningAutoApplied: learningPayload?.auto_apply,
            learningSuppressed: learningPayload?.suppressed,
            learningStatusAuthoritative: learningStatusAuthority.authoritative,
            learningRuleId: learningPayload?.rule_id,
            learningScore: learningPayload?.score ?? item.learningRecommendationScore,
            learningConfidence: learningPayload?.confidence,
            learningMargin: learningPayload?.margin,
            learningAcceptedCount: learningPayload?.accepted_count,
            learningRejectedCount: learningPayload?.rejected_count,
            learningAutoAppliedCount: learningPayload?.auto_applied_count,
            llmStatus: llmStatusAuthority.status,
            llmTitle: String(llmPayload.reason || ''),
            llmSummary: options.buildLLMSignalSummary(llmPayload),
            llmConfidence: llmPayload.confidence,
            llmSuggestedCategoryId: llmPayload.suggested_category_id,
            llmCategoryPath: options.getLLMSignalCategoryPath(llmPayload),
            llmSourceAccount: String(llmPayload.suggested_source_account || ''),
            llmDestinationAccount: String(llmPayload.suggested_destination_account || ''),
            llmLifecycleStatus: String(llmPayload.lifecycle_status || ''),
            llmSignalState: String(llmPayload.signal_state || ''),
            llmSuppressed: llmPayload.suppressed,
            llmStatusAuthoritative: llmStatusAuthority.authoritative,
            hasRecurringMatch: item.hasRecurringMatch(),
            recurringTitle: options.getRecurringMatchSummary(item),
            recurringCandidateCount: item.recurringCandidateCount,
            recurringPrimaryReason: options.getPrimaryRecurringReason(item)
        }, {
            ...importPreviewSignalSharedContext.value.options,
            formatAmountWithCurrency: options.formatAmountWithCurrency,
            currentParserId: item.parserId,
            sourceRowLookup: importPreviewSignalSourceContext.value.sourceRowLookup
        });

        viewModelCache.set(item, { signature, viewModel });
        return viewModel;
    }

    function getImportPreviewHistoryRewriteOperation(
        item: ImportTransaction
    ): ImportPreviewHistoryRewriteAcknowledgementOperation | null {
        const previewId = options.getPreviewId(item);
        if (previewId === null) {
            return null;
        }
        return buildImportPreviewHistoryRewriteOperationAcknowledgement(previewId, {
            reconciliationPlannedOperation: item.matching?.reconciliation?.planned_operation,
            reconciliationHistoryBillId: item.matching?.reconciliation?.history_bill_id,
            reconciliationHistoryBillVersion: item.matching?.reconciliation?.history_bill_version,
            reconciliationOperationId: item.matching?.reconciliation?.operation_id,
            reconciliationAcknowledgementToken: item.matching?.reconciliation?.acknowledgement_token,
            reconciliationDestructiveAckRequired: !!item.matching?.reconciliation?.destructive_ack_required
        });
    }

    function getImportTransactionRowKey(item: ImportTransaction): string {
        const previewId = options.getPreviewId(item);
        return previewId !== null ? `preview:${previewId}` : `row:${item.index}`;
    }

    return {
        getImportPreviewHistoryRewriteOperation,
        getImportPreviewSignalViewModel,
        getImportTransactionRowKey
    };
}
