import { computed, ref } from 'vue';

import { TransactionType } from '@/core/transaction.ts';
import type { ImportTransaction } from '@/models/imported_transaction.ts';

import {
    collectImportTransactionSelectionSummary,
    type ImportTransactionSelectionSummary
} from '../checkDataSelection.ts';

type MatchingAnnotationPayload = Record<string, unknown> | string | null | undefined;

export interface ImportCheckDataAnnotationsOptions {
    translate: (key: string) => string;
    isTransactionCategoryAccepted: (item: ImportTransaction) => boolean;
    isKnownAccountId: (accountId: string | number | null | undefined) => boolean;
    requiresDestinationAccount: (item: ImportTransaction) => boolean;
    getTrackedTransactionsForSelection: () => ImportTransaction[];
    getDisplayDateTime: (item: ImportTransaction) => string;
}

export function useImportCheckDataAnnotations(options: ImportCheckDataAnnotationsOptions) {
    const importTransactionSelectionRevision = ref(0);

    function hasMissingCategoryIssue(item: ImportTransaction): boolean {
        return item.type !== TransactionType.ModifyBalance
            && !options.isTransactionCategoryAccepted(item);
    }

    function hasMissingSourceAccountIssue(item: ImportTransaction): boolean {
        return !options.isKnownAccountId(item.sourceAccountId);
    }

    function hasMissingDestinationAccountIssue(item: ImportTransaction): boolean {
        return options.requiresDestinationAccount(item)
            && !options.isKnownAccountId(item.destinationAccountId);
    }

    function hasTransferAccountReviewIssue(item: ImportTransaction): boolean {
        return options.requiresDestinationAccount(item)
            && options.isKnownAccountId(item.sourceAccountId)
            && options.isKnownAccountId(item.destinationAccountId)
            && item.sourceAccountId === item.destinationAccountId;
    }

    function collectAnnotationIssues(item: ImportTransaction): string[] {
        const reasons: string[] = [];
        if (hasMissingCategoryIssue(item)) {
            reasons.push(options.translate('Missing Category'));
        }
        if (hasMissingSourceAccountIssue(item)) {
            reasons.push(options.translate('Missing Source Account'));
        }
        if (hasMissingDestinationAccountIssue(item)) {
            reasons.push(options.translate('Missing Destination Account'));
        }
        if (hasTransferAccountReviewIssue(item)) {
            reasons.push(options.translate('Review Transfer Accounts'));
        }
        return reasons;
    }

    const importTransactionSelectionSummary = computed<ImportTransactionSelectionSummary>(() => {
        void importTransactionSelectionRevision.value;
        return collectImportTransactionSelectionSummary(
            options.getTrackedTransactionsForSelection(),
            collectAnnotationIssues
        );
    });

    function refreshImportTransactionSelectionSummary(): void {
        importTransactionSelectionRevision.value += 1;
    }

    function getAnnotationIssues(item: ImportTransaction): string[] {
        return importTransactionSelectionSummary.value.annotationIssuesByIndex[item.index]
            || collectAnnotationIssues(item);
    }

    function needsAnnotation(item: ImportTransaction): boolean {
        return getAnnotationIssues(item).length > 0;
    }

    function getMatchingAnnotationPayload(item: ImportTransaction): MatchingAnnotationPayload {
        return item.matching?.annotation as MatchingAnnotationPayload;
    }

    function getAnnotationText(annotation: MatchingAnnotationPayload): string {
        if (typeof annotation === 'string') {
            return annotation.trim();
        }
        if (annotation && typeof annotation === 'object') {
            for (const key of ['status', 'type', 'review_status', 'level', 'reason']) {
                const value = annotation[key];
                if (typeof value === 'string' && value.trim() !== '') {
                    return value.trim();
                }
            }
        }
        return '';
    }

    function getAnnotationType(annotation: MatchingAnnotationPayload): string {
        if (typeof annotation === 'string') {
            return annotation.trim().toLowerCase();
        }
        if (annotation && typeof annotation === 'object') {
            return getAnnotationText(annotation).trim().toLowerCase();
        }
        return '';
    }

    function hasCurrentPersistedMatchingAnnotationIssue(item: ImportTransaction): boolean {
        const annotation = getMatchingAnnotationPayload(item);
        if (getAnnotationText(annotation) === '') {
            return false;
        }

        const annotationType = getAnnotationType(annotation);
        if (['category', 'category_missing', 'missing_category', 'missing-category', 'missing_classification'].includes(annotationType)) {
            return hasMissingCategoryIssue(item);
        }
        if (['account', 'missing_account', 'source_account', 'source_account_missing', 'missing_source_account', 'missing-source-account'].includes(annotationType)) {
            return hasMissingSourceAccountIssue(item);
        }
        if (['destination_account', 'destination_account_missing', 'missing_destination_account', 'missing-destination-account'].includes(annotationType)) {
            return hasMissingDestinationAccountIssue(item);
        }
        if (['transfer_account_direction', 'transfer_accounts', 'review_transfer_accounts', 'same_transfer_accounts'].includes(annotationType)) {
            return hasMissingSourceAccountIssue(item)
                || hasMissingDestinationAccountIssue(item)
                || hasTransferAccountReviewIssue(item);
        }
        return getAnnotationText(annotation) !== '';
    }

    function hasCurrentAnnotationIssue(item: ImportTransaction): boolean {
        return collectAnnotationIssues(item).length > 0
            || hasCurrentPersistedMatchingAnnotationIssue(item);
    }

    function hasBaselineAnnotationIssue(item: ImportTransaction): boolean {
        return hasCurrentAnnotationIssue(item);
    }

    function getAnnotationSummary(item: ImportTransaction): string {
        return getAnnotationIssues(item).join(' · ');
    }

    function getAnnotationListTitle(item: ImportTransaction): string {
        const description = item.comment
            || item.counterparty
            || item.paymentMethod
            || options.translate('No description');
        return `${options.getDisplayDateTime(item)} · ${description}`;
    }

    return {
        getAnnotationIssues,
        getAnnotationListTitle,
        getAnnotationSummary,
        hasBaselineAnnotationIssue,
        hasCurrentAnnotationIssue,
        hasCurrentPersistedMatchingAnnotationIssue,
        hasMissingCategoryIssue,
        hasMissingDestinationAccountIssue,
        hasMissingSourceAccountIssue,
        hasTransferAccountReviewIssue,
        importTransactionSelectionSummary,
        needsAnnotation,
        refreshImportTransactionSelectionSummary
    };
}
