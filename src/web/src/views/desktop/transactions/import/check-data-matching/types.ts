import type { ImportHistoryBillSummaryPayload, ImportMatchingSourcePayload } from '@/models/import_matching.ts';

export interface ImportCheckMatchingContextState {
    parserId?: string;
    parserTags?: string[];
    dedupType?: string;
    dedupSourceIds?: Array<number | string>;
    isManuallyAnnotated?: boolean;
}

export interface ImportCheckMatchingContextSummary {
    parserId: string;
    parserTags: string[];
    dedupType: string;
    dedupSourceIds: Array<number | string>;
    isManuallyAnnotated: boolean;
}

export interface ImportCheckMatchingSourceRow {
    id: number | string;
    parserId?: string;
    parserTags?: string[];
}

export interface ImportCheckMatchingSourceContext {
    parserId?: string;
    parserTags?: string[];
}

export interface ImportCheckMatchingDedupTitleOptions {
    matchLabel?: string;
    currentParserId?: string;
    parserLabels?: Record<string, string>;
    sourceRows?: ImportCheckMatchingSourceRow[];
    sourceRowLookup?: ReadonlyMap<string, string | ImportCheckMatchingSourceContext>;
    dedupLabels?: Record<string, string>;
    sourceRoleLabels?: Record<string, string>;
    infoLabels?: Partial<ImportPreviewSignalInfoLabels>;
    formatAmountWithCurrency?: (amountCents: number, currency: string) => string;
}

export type ImportPreviewSignalStatus = 'pending' | 'accepted' | 'rejected' | 'skipped';
export type ImportPreviewRawSignalStatus = ImportPreviewSignalStatus | string | null | undefined;
export type ImportPreviewRawFlag = boolean | number | string | null | undefined;
export type ImportPreviewLearningMode = 'green' | 'blue' | '';

export interface ImportPreviewSignalDecision {
    decision: 'accept' | 'reject' | 'clear';
    labelKey: string;
    color: string;
}

export interface ImportPreviewSignalParserView {
    parserId: string;
    label: string;
    color: string;
    title: string;
    detailLines: string[];
}

export interface ImportPreviewSignalDedupView {
    dedupType: string;
    labelKey: string;
    label: string;
    title: string;
    color: string;
    sourceCount: number;
    detailLines: string[];
}

export interface ImportPreviewSignalReviewView {
    status: ImportPreviewSignalStatus;
    labelKey: string;
    title: string;
    color: string;
    profileText?: string;
    summary?: string;
    actions: ImportPreviewSignalDecision[];
    detailLines: string[];
    historyBillId?: number;
}

export interface ImportPreviewSignalRecurringView {
    hasMatch: boolean;
    title: string;
    candidateCount: number;
    primaryReason: string;
}

export interface ImportPreviewSignalViewModel {
    parser: ImportPreviewSignalParserView | null;
    dedup: ImportPreviewSignalDedupView | null;
    isManuallyAnnotated: boolean;
    historyRewrite: ImportPreviewSignalReviewView | null;
    transferSuggestion: ImportPreviewSignalReviewView | null;
    investment: ImportPreviewSignalReviewView | null;
    learning: ImportPreviewSignalReviewView | null;
    llm: ImportPreviewSignalReviewView | null;
    recurring: ImportPreviewSignalRecurringView | null;
    hasAnySignal: boolean;
}

export const IMPORT_PREVIEW_VISIBLE_SIGNAL_FILTERS = [
    'parser',
    'platform_duplicate',
    'transfer',
    'history',
    'learning',
    'llm'
] as const;

export type ImportPreviewVisibleSignalFilterValue = typeof IMPORT_PREVIEW_VISIBLE_SIGNAL_FILTERS[number];

export interface ImportPreviewHistoryRewriteAcknowledgementOperation {
    preview_id: number;
    operation_id: string;
    planned_operation: string;
    history_bill_id: number;
    history_bill_version: number;
    acknowledgement_token: string;
}

export interface ImportPreviewHistoryRewriteAcknowledgement {
    acknowledged: true;
    selected_preview_ids: number[];
    operations: ImportPreviewHistoryRewriteAcknowledgementOperation[];
    selection_scope: Record<string, unknown>;
}

export interface ImportPreviewSignalState extends ImportCheckMatchingContextState {
    transferStatus?: ImportPreviewRawSignalStatus;
    transferTitle?: string;
    transferLearningLevel?: string;
    transferPairOrder?: string;
    transferSourceChain?: ImportMatchingSourcePayload[];
    investmentStatus?: ImportPreviewSignalStatus | null;
    investmentTitle?: string;
    investmentProfileText?: string;
    learningStatus?: ImportPreviewRawSignalStatus;
    learningTitle?: string;
    learningSummary?: string;
    learningMode?: ImportPreviewLearningMode | string;
    learningLifecycleStatus?: string;
    learningSignalState?: string;
    learningAutoApplied?: ImportPreviewRawFlag;
    learningSuppressed?: ImportPreviewRawFlag;
    learningStatusAuthoritative?: boolean;
    learningRuleId?: number | string | null;
    learningScore?: number | string;
    learningConfidence?: number | string;
    learningMargin?: number | string;
    learningAcceptedCount?: number | string;
    learningRejectedCount?: number | string;
    learningAutoAppliedCount?: number | string;
    llmStatus?: ImportPreviewRawSignalStatus;
    llmTitle?: string;
    llmSummary?: string;
    llmConfidence?: number | string;
    llmSuggestedCategoryId?: number | string;
    llmCategoryPath?: string;
    llmSourceAccount?: string;
    llmDestinationAccount?: string;
    llmLifecycleStatus?: string;
    llmSignalState?: string;
    llmSuppressed?: ImportPreviewRawFlag;
    llmStatusAuthoritative?: boolean;
    historyStatus?: ImportPreviewSignalStatus | null;
    dedupSourceCount?: number;
    dedupSourceLabels?: string[];
    dedupSources?: ImportMatchingSourcePayload[];
    parserIdChain?: ImportMatchingSourcePayload[];
    reconciliationType?: string;
    reconciliationStatus?: string;
    reconciliationTitle?: string;
    reconciliationSourceChain?: ImportMatchingSourcePayload[];
    reconciliationPlannedOperation?: string;
    reconciliationHistoryBillId?: number | string | null;
    reconciliationHistoryBillVersion?: number | string | null;
    reconciliationHistoryRole?: string;
    reconciliationGroupKey?: string;
    reconciliationOperationId?: string;
    reconciliationAcknowledgementToken?: string;
    reconciliationDestructiveAckRequired?: boolean;
    reconciliationNotice?: string;
    reconciliationHistorySummary?: ImportHistoryBillSummaryPayload | null;
    hasRecurringMatch?: boolean;
    recurringTitle?: string;
    recurringCandidateCount?: number;
    recurringPrimaryReason?: string;
}

export interface ImportPreviewSignalViewModelOptions extends ImportCheckMatchingDedupTitleOptions {
    parserColors?: Record<string, string>;
}

export interface ImportPreviewSignalInfoLabels {
    sourceLabel: string;
    duplicateSourcesLabel: string;
    recommendedCategoryLabel: string;
    accountRouteLabel: string;
}

export interface ImportPreviewTypeColumnViewModel {
    type: number;
    signalKeys: string[];
}
