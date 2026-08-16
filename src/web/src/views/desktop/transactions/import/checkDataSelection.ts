import { TransactionType } from '@/core/transaction.ts';
import type { ImportTransaction } from '@/models/imported_transaction.ts';

export interface AnnotationReasonSummary {
    key: string;
    label: string;
    count: number;
}

export interface ImportTransactionSelectionSummary {
    selectedCount: number;
    selectedExpenseCount: number;
    selectedIncomeCount: number;
    selectedTransferCount: number;
    selectedRecurringMatchCount: number;
    selectedInvalidCount: number;
    annotationCount: number;
    selectedAnnotationCount: number;
    selectedAnnotationTransactions: ImportTransaction[];
    annotationReasonSummaries: AnnotationReasonSummary[];
}

export function collectImportTransactionSelectionSummary(
    transactions: ImportTransaction[],
    collectAnnotationIssues: (transaction: ImportTransaction) => string[]
): ImportTransactionSelectionSummary {
    const annotationReasonSummaryMap: Record<string, AnnotationReasonSummary> = {};
    const selectedAnnotationTransactions: ImportTransaction[] = [];
    let selectedCount = 0;
    let selectedExpenseCount = 0;
    let selectedIncomeCount = 0;
    let selectedTransferCount = 0;
    let selectedRecurringMatchCount = 0;
    let selectedInvalidCount = 0;
    let annotationCount = 0;
    let selectedAnnotationCount = 0;

    for (const transaction of transactions) {
        const annotationIssues = collectAnnotationIssues(transaction);
        const hasAnnotationIssues = annotationIssues.length > 0;

        if (hasAnnotationIssues) {
            annotationCount++;
        }

        if (!transaction.selected) {
            continue;
        }

        selectedCount++;

        if (transaction.type === TransactionType.Expense) {
            selectedExpenseCount++;
        } else if (transaction.type === TransactionType.Income) {
            selectedIncomeCount++;
        } else if (transaction.type === TransactionType.Transfer) {
            selectedTransferCount++;
        }

        if (transaction.hasRecurringMatch()) {
            selectedRecurringMatchCount++;
        }

        if (!transaction.valid) {
            selectedInvalidCount++;
        }

        if (!hasAnnotationIssues) {
            continue;
        }

        selectedAnnotationCount++;
        selectedAnnotationTransactions.push(transaction);

        for (const reason of annotationIssues) {
            if (!annotationReasonSummaryMap[reason]) {
                annotationReasonSummaryMap[reason] = {
                    key: reason,
                    label: reason,
                    count: 0
                };
            }

            annotationReasonSummaryMap[reason].count++;
        }
    }

    return {
        selectedCount,
        selectedExpenseCount,
        selectedIncomeCount,
        selectedTransferCount,
        selectedRecurringMatchCount,
        selectedInvalidCount,
        annotationCount,
        selectedAnnotationCount,
        selectedAnnotationTransactions,
        annotationReasonSummaries: Object.values(annotationReasonSummaryMap).sort((left, right) => right.count - left.count)
    };
}

export function collectTrackedImportTransactionsForSelection(
    serverPagedDrafts: Iterable<[number, ImportTransaction]>,
    currentTransactions: ImportTransaction[],
    getPreviewId: (transaction: ImportTransaction) => number | null
): ImportTransaction[] {
    const trackedTransactions = new Map<number, ImportTransaction>();
    for (const [previewId, transaction] of serverPagedDrafts) {
        trackedTransactions.set(previewId, transaction);
    }

    for (const transaction of currentTransactions) {
        const previewId = getPreviewId(transaction);
        if (previewId === null) {
            continue;
        }

        trackedTransactions.set(previewId, transaction);
    }

    return Array.from(trackedTransactions.values());
}
