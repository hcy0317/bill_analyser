import type { ImportTransaction } from '@/models/imported_transaction.ts';

import {
    type ImportPreviewResolvedCategoryPath
} from './importPreview.ts';
import {
    getImportPreviewTransactionTypeLabel,
    type ImportPreviewTransactionDraft
} from './importPreviewTransaction.ts';
import { requireStrictIntegerCents } from './strictCents.ts';

export interface BuildImportPreviewUpdateFromTransactionOptions {
    categoryPath: ImportPreviewResolvedCategoryPath | null;
    clearTransferDecision?: boolean;
    clearLearningDecision?: boolean;
    clearLlmDecision?: boolean;
    includeSuggestionDecisionClears?: boolean;
}

export type ImportPreviewUpdatePayload = Record<string, unknown> & {
    id?: number;
    preview_type: string;
    preview_amount_cents: number;
    preview_destination_amount_cents: number;
    preview_source_account_id: number | null;
    preview_destination_account_id: number | null;
    preview_recurring_id: number | null;
    preview_recurring_name: string;
    preview_recurring_candidate_count: number;
    preview_recurring_match_score: number;
    preview_recurring_match_reasons: string;
    preview_recurring_matched_date: string;
    category_id: number | null;
    preview_main_category: string;
    preview_sub_category: string;
    clear_transfer_decision: boolean;
    selected: boolean;
};

function parseOptionalInteger(value: string | number | null | undefined): number | null {
    if (typeof value === 'number' && Number.isFinite(value)) {
        return Math.trunc(value);
    }

    if (typeof value === 'string' && value.trim()) {
        const parsedValue = parseInt(value, 10);
        return Number.isNaN(parsedValue) ? null : parsedValue;
    }

    return null;
}

export function getPreviewIdFromImportTransaction(transaction: ImportTransaction): number | null {
    const previewId = (transaction as ImportPreviewTransactionDraft)._previewId;
    return typeof previewId === 'number' && Number.isFinite(previewId) && previewId > 0 ? previewId : null;
}

export function getPreviewUpdateId(update: Record<string, unknown>): number | null {
    const rawId = update['id'];
    const numericId = typeof rawId === 'number' ? rawId : Number(String(rawId || '').trim());
    return Number.isFinite(numericId) && numericId > 0 ? Math.trunc(numericId) : null;
}

export function buildImportPreviewUpdateFromTransaction(
    transaction: ImportTransaction,
    options: BuildImportPreviewUpdateFromTransactionOptions
): ImportPreviewUpdatePayload {
    const clearTransferDecision = !!options.clearTransferDecision;
    const clearLearningDecision = !!options.clearLearningDecision;
    const clearLlmDecision = !!options.clearLlmDecision;
    const clearActionableSuggestions = [
        clearTransferDecision ? 'transfer' : '',
        clearLearningDecision ? 'learning' : '',
        clearLlmDecision ? 'llm' : ''
    ].filter(Boolean);
    const categoryPath = options.categoryPath;
    const update: ImportPreviewUpdatePayload = {
        id: (transaction as ImportPreviewTransactionDraft)._previewId,
        preview_type: getImportPreviewTransactionTypeLabel(transaction.type),
        preview_amount_cents: requireStrictIntegerCents(transaction.sourceAmountCents, 'sourceAmountCents'),
        preview_destination_amount_cents: requireStrictIntegerCents(
            transaction.destinationAmountCents,
            'destinationAmountCents'
        ),
        preview_source_account_id: parseOptionalInteger(transaction.sourceAccountId),
        preview_destination_account_id: parseOptionalInteger(transaction.destinationAccountId),
        preview_recurring_id: parseOptionalInteger(transaction.recurringTemplateId),
        preview_recurring_name: transaction.recurringTemplateName || '',
        preview_recurring_candidate_count: transaction.recurringCandidateCount || 0,
        preview_recurring_match_score: transaction.recurringMatchScore || 0,
        preview_recurring_match_reasons: transaction.recurringMatchReasons || '',
        preview_recurring_matched_date: transaction.recurringMatchedDate || '',
        category_id: categoryPath ? parseInt(categoryPath.id, 10) : null,
        preview_main_category: categoryPath?.mainCategory || '',
        preview_sub_category: categoryPath?.subCategory || '',
        clear_transfer_decision: clearTransferDecision,
        selected: transaction.selected
    };

    if (options.includeSuggestionDecisionClears) {
        update['clear_learning_decision'] = clearLearningDecision;
        update['clear_llm_decision'] = clearLlmDecision;
        update['clear_actionable_suggestions'] = clearActionableSuggestions;
    }

    return update;
}
