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
    baseline?: ImportPreviewUpdateBaseline;
    validAccountIds?: ReadonlySet<string>;
    clearTransferDecision?: boolean;
    clearLearningDecision?: boolean;
    clearLlmDecision?: boolean;
    includeSuggestionDecisionClears?: boolean;
}

export interface ImportPreviewUpdateBaseline {
    selected: boolean;
    type: number;
    categoryId: string;
    sourceAmountCents: number;
    destinationAmountCents: number;
    sourceAccountId: string;
    destinationAccountId: string;
    tagIds: string[];
    counterparty: string;
    paymentMethod: string;
    comment: string;
    isManuallyAnnotated: boolean;
    recurringTemplateId: string;
    recurringTemplateName: string;
    recurringCandidateCount: number;
    recurringMatchScore: number;
    recurringMatchReasons: string;
    recurringMatchedDate: string;
}

export type ImportPreviewUpdatePayload = ImportPreviewPatchPayload & {
    clear_learning_decision?: boolean;
    clear_llm_decision?: boolean;
    clear_actionable_suggestions?: string[];
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
    const baseline = options.baseline;
    if (baseline) {
        const update: ImportPreviewUpdatePayload = {
            id: previewId,
            expected_row_version: expectedRowVersion
        };
        let hasManualEdit = false;
        if (baseline.type !== transaction.type) {
            update.preview_type = getImportPreviewTransactionTypeLabel(transaction.type);
            hasManualEdit = true;
        }
        if (baseline.sourceAmountCents !== transaction.sourceAmountCents) {
            update.preview_amount_cents = requireStrictIntegerCents(
                transaction.sourceAmountCents,
                'sourceAmountCents'
            );
            hasManualEdit = true;
        }
        if (baseline.destinationAmountCents !== transaction.destinationAmountCents) {
            update.preview_destination_amount_cents = requireStrictIntegerCents(
                transaction.destinationAmountCents,
                'destinationAmountCents'
            );
            hasManualEdit = true;
        }
        if (baseline.categoryId !== String(transaction.categoryId || '')) {
            update.category_id = categoryPath ? parseInt(categoryPath.id, 10) : null;
            update.preview_main_category = categoryPath?.mainCategory || '';
            update.preview_sub_category = categoryPath?.subCategory || '';
            hasManualEdit = true;
        }
        if (baseline.sourceAccountId !== String(transaction.sourceAccountId || '')) {
            update.preview_source_account_id = parseOptionalInteger(
                transaction.sourceAccountId,
                options.validAccountIds
            );
            hasManualEdit = true;
        }
        if (baseline.destinationAccountId !== String(transaction.destinationAccountId || '')) {
            update.preview_destination_account_id = parseOptionalInteger(
                transaction.destinationAccountId,
                options.validAccountIds
            );
            hasManualEdit = true;
        }
        if (baseline.counterparty !== String(transaction.counterparty || '')) {
            update['preview_counterparty'] = transaction.counterparty || '';
            hasManualEdit = true;
        }
        if (baseline.paymentMethod !== String(transaction.paymentMethod || '')) {
            update['preview_payment_method'] = transaction.paymentMethod || '';
            hasManualEdit = true;
        }
        if (baseline.comment !== String(transaction.comment || '')) {
            update['preview_description'] = transaction.comment || '';
            hasManualEdit = true;
        }
        if (baseline.recurringTemplateId !== String(transaction.recurringTemplateId || '')) {
            update.preview_recurring_id = parseOptionalInteger(transaction.recurringTemplateId);
        }
        if (baseline.recurringTemplateName !== String(transaction.recurringTemplateName || '')) {
            update.preview_recurring_name = transaction.recurringTemplateName || '';
        }
        if (baseline.recurringCandidateCount !== Number(transaction.recurringCandidateCount || 0)) {
            update.preview_recurring_candidate_count = transaction.recurringCandidateCount || 0;
        }
        if (baseline.recurringMatchScore !== Number(transaction.recurringMatchScore || 0)) {
            update.preview_recurring_match_score = transaction.recurringMatchScore || 0;
        }
        if (baseline.recurringMatchReasons !== String(transaction.recurringMatchReasons || '')) {
            update.preview_recurring_match_reasons = transaction.recurringMatchReasons || '';
        }
        if (baseline.recurringMatchedDate !== String(transaction.recurringMatchedDate || '')) {
            update.preview_recurring_matched_date = transaction.recurringMatchedDate || '';
        }
        if (baseline.selected !== transaction.selected) {
            update.selected = transaction.selected;
        }
        if (hasManualEdit && transaction.isManuallyAnnotated) {
            update.is_manually_annotated = true;
        }
        appendSuggestionDecisionClears(update, {
            clearTransferDecision,
            clearLearningDecision,
            clearLlmDecision,
            clearActionableSuggestions,
            includeSuggestionDecisionClears: !!options.includeSuggestionDecisionClears
        });
        return update;
    }

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
    appendSuggestionDecisionClears(update, {
        clearTransferDecision,
        clearLearningDecision,
        clearLlmDecision,
        clearActionableSuggestions,
        includeSuggestionDecisionClears: !!options.includeSuggestionDecisionClears
    });

    return update;
}

function appendSuggestionDecisionClears(
    update: ImportPreviewUpdatePayload,
    options: {
        clearTransferDecision: boolean;
        clearLearningDecision: boolean;
        clearLlmDecision: boolean;
        clearActionableSuggestions: string[];
        includeSuggestionDecisionClears: boolean;
    }
): void {
    if (options.clearTransferDecision) {
        update.clear_transfer_decision = true;
    }
    if (!options.includeSuggestionDecisionClears) {
        return;
    }
    if (options.clearLearningDecision) {
        update.clear_learning_decision = true;
    }
    if (options.clearLlmDecision) {
        update.clear_llm_decision = true;
    }
    if (options.clearActionableSuggestions.length > 0) {
        update.clear_actionable_suggestions = options.clearActionableSuggestions;
    }
}
