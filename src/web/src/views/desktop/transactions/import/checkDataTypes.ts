import type { TransactionType } from '@/core/transaction.ts';
import type { ImportTransaction } from '@/models/imported_transaction.ts';

import type { ImportCheckAnnotationFilterValue } from './checkDataAnnotation.ts';
import type { ImportCheckDataFilterLike } from './checkDataFilters.ts';
import type { ImportCheckLearningDecisionBaseline } from './checkDataLearning.ts';
import type { ImportPreviewVisibleSignalFilterValue } from './checkDataMatching.ts';

export interface ImportTransactionCheckDataFilter extends ImportCheckDataFilterLike {
    transactionType: TransactionType | null;
    signal: ImportPreviewVisibleSignalFilterValue | null;
    annotation: ImportCheckAnnotationFilterValue;
}

export interface ImportTransactionCheckDataMenuGroup {
    title: string;
    summary?: string;
    items: ImportTransactionCheckDataMenu[];
}

export interface ImportTransactionCheckDataMenu {
    prependIcon?: string;
    title: string;
    subTitle?: string;
    appendIcon?: string;
    disabled?: boolean;
    divider?: boolean;
    items?: ImportTransactionCheckDataMenu[];
    onClick?: () => void;
}

export interface RecurringCandidateItem {
    id: string;
    name?: string;
    matchScore?: number;
    matchReasons?: string[];
    matchedOccurrenceDate?: string;
}

export interface MatchingSessionCandidateItem {
    candidate_id?: string;
    details?: {
        rule_id?: number | null;
        score?: number;
        level?: string;
        reason?: string;
        recommended_type?: string;
        summary?: string;
        review_status?: string;
        suppressed?: boolean;
        source?: string;
        mode?: string;
        auto_apply?: boolean;
        model_version?: string;
    };
}

export interface TransferDecisionPreviewBaseline {
    type: number;
    categoryId: string;
    sourceAccountId: string;
    destinationAccountId: string;
    recurringTemplateId: string;
    recurringTemplateName: string;
    recurringCandidateCount: number;
    recurringMatchScore: number;
    recurringMatchReasons: string;
    recurringMatchedDate: string;
    reviewStatus: string;
    reviewedType: string;
    suppressed: boolean;
}

export type ImportTransactionWithPreviewState = ImportTransaction & {
    _previewId?: number;
    _previewDecisionBaseline?: TransferDecisionPreviewBaseline;
    _shouldClearTransferDecision?: boolean;
    _shouldClearLearningDecision?: boolean;
    _shouldClearLlmDecision?: boolean;
    _learningDecisionBaseline?: ImportCheckLearningDecisionBaseline;
};
