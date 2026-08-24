import type { ImportTransaction } from '@/models/imported_transaction.ts';
import type {
    ImportPreviewPatchPayload,
    ImportPreviewRecord,
} from '@/models/import_preview.ts';

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
    validAccountIds?: ReadonlySet<string>;
    clearTransferDecision?: boolean;
    clearLearningDecision?: boolean;
    clearLlmDecision?: boolean;
    includeSuggestionDecisionClears?: boolean;
}

export type ImportPreviewUpdatePayload = ImportPreviewPatchPayload & {
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

function parseOptionalInteger(
    value: string | number | null | undefined,
    validIds?: ReadonlySet<string>
): number | null {
    if (typeof value === 'number' && Number.isFinite(value)) {
        const normalizedValue = Math.trunc(value);
        if (normalizedValue <= 0 || (validIds && !validIds.has(String(normalizedValue)))) {
            return null;
        }
        return normalizedValue;
    }

    if (typeof value === 'string' && value.trim()) {
        const parsedValue = parseInt(value, 10);
        if (Number.isNaN(parsedValue) || parsedValue <= 0 || (validIds && !validIds.has(String(parsedValue)))) {
            return null;
        }
        return parsedValue;
    }

    return null;
}

export function getPreviewIdFromImportTransaction(transaction: ImportTransaction): number | null {
    const previewId = (transaction as ImportPreviewTransactionDraft)._previewId;
    return typeof previewId === 'number' && Number.isFinite(previewId) && previewId > 0 ? previewId : null;
}

export function getPreviewRowVersionFromImportTransaction(
    transaction: ImportTransaction
): number {
    const rowVersion = (transaction as ImportPreviewTransactionDraft)._rowVersion;
    if (Number.isInteger(rowVersion) && Number(rowVersion) > 0) {
        return Number(rowVersion);
    }
    throw new Error('Import preview row version is required.');
}

export function syncPreviewRowVersionToImportTransaction(
    transaction: ImportTransaction,
    preview: ImportPreviewRecord
): void {
    if (Number.isInteger(preview.row_version) && Number(preview.row_version) > 0) {
        (transaction as ImportPreviewTransactionDraft)._rowVersion = preview.row_version;
    }
}

export function rebaseImportPreviewTextSyncConflict(
    transaction: ImportTransaction,
    preview: ImportPreviewRecord
): void {
    transaction.counterparty = preview.preview_counterparty || '';
    transaction.paymentMethod = preview.preview_payment_method || '';
    transaction.comment = preview.preview_description || '';
    const selected = preview.preview_selected ?? preview.selected;
    if (typeof selected === 'boolean') {
        transaction.selected = selected;
    }
    syncPreviewRowVersionToImportTransaction(transaction, preview);
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
    const previewId = getPreviewIdFromImportTransaction(transaction);
    if (previewId === null) {
        throw new Error('Import preview row id is required.');
    }
    const expectedRowVersion = getPreviewRowVersionFromImportTransaction(transaction);
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
        id: previewId,
        expected_row_version: expectedRowVersion,
        preview_type: getImportPreviewTransactionTypeLabel(transaction.type),
        preview_amount_cents: requireStrictIntegerCents(transaction.sourceAmountCents, 'sourceAmountCents'),
        preview_destination_amount_cents: requireStrictIntegerCents(
            transaction.destinationAmountCents,
            'destinationAmountCents'
        ),
        preview_source_account_id: parseOptionalInteger(transaction.sourceAccountId, options.validAccountIds),
        preview_destination_account_id: parseOptionalInteger(transaction.destinationAccountId, options.validAccountIds),
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
    if (transaction.isManuallyAnnotated) {
        update['is_manually_annotated'] = true;
    }
    if (options.includeSuggestionDecisionClears) {
        if (clearLearningDecision) {
            update['clear_learning_decision'] = true;
        }
        if (clearLlmDecision) {
            update['clear_llm_decision'] = true;
        }
        if (clearActionableSuggestions.length > 0) {
            update['clear_actionable_suggestions'] = clearActionableSuggestions;
        }
    }

    return update;
}
