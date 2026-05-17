import { TransactionType } from '@/core/transaction.ts';
import type { ReceiptDraftField, ReceiptTransactionDraft, RecognizedReceiptImageResponse } from '@/models/large_language_model.ts';

export type ReceiptDraftCandidateKey =
    | 'type'
    | 'amount'
    | 'time'
    | 'description'
    | 'categoryId'
    | 'sourceAccountId'
    | 'destinationAccountId'
    | 'tagIds';

export interface ReceiptDraftCandidateHint {
    readonly id: string;
    readonly key: ReceiptDraftCandidateKey;
    readonly titleKey: string;
    readonly field: ReceiptDraftField;
}

export interface ReceiptDraftEditableTransaction {
    type: number;
    sourceAmount: number;
    time: number;
    comment: string;
    sourceAccountId: string;
    destinationAccountId: string;
    tagIds: string[];
    setCategoryId(categoryId: string): void;
}

const RECEIPT_DRAFT_CANDIDATE_TITLES: Record<ReceiptDraftCandidateKey, string> = {
    type: 'Transaction Type',
    amount: 'Amount',
    time: 'Transaction Time',
    description: 'Description',
    categoryId: 'Category',
    sourceAccountId: 'Source Account',
    destinationAccountId: 'Destination Account',
    tagIds: 'Tags'
};

export function receiptDraftFieldNumberValue(field: ReceiptDraftField | undefined): number | undefined {
    return typeof field?.value === 'number' && Number.isFinite(field.value) ? field.value : undefined;
}

export function receiptDraftFieldStringValue(field: ReceiptDraftField | undefined): string | undefined {
    if (typeof field?.value === 'string' && field.value) {
        return field.value;
    }

    if (typeof field?.value === 'number' && Number.isFinite(field.value)) {
        return String(field.value);
    }

    return undefined;
}

export function receiptDraftFieldStringArrayValue(field: ReceiptDraftField | undefined): string[] | undefined {
    if (!Array.isArray(field?.value)) {
        return undefined;
    }

    const values = field.value.filter((item): item is string => typeof item === 'string' && !!item);
    return values.length ? Array.from(new Set(values)) : undefined;
}

export function receiptDraftAmountToCents(field: ReceiptDraftField | undefined): number | undefined {
    const amount = receiptDraftFieldNumberValue(field);
    if (amount === undefined) {
        return undefined;
    }

    const unit = field?.unit?.toLowerCase();
    if (unit === 'cent' || unit === 'cents' || unit === 'fen') {
        return Math.round(amount);
    }

    return Math.round(amount * 100);
}

export function receiptDraftTimeToUnixSeconds(field: ReceiptDraftField | undefined): number | undefined {
    const value = receiptDraftFieldStringValue(field);
    if (!value) {
        return undefined;
    }

    const parsedMs = Date.parse(value);
    return Number.isNaN(parsedMs) ? undefined : Math.floor(parsedMs / 1000);
}

export function transactionTypeFromReceiptDraftValue(value: unknown): number | undefined {
    if (value === TransactionType.Expense || value === 'expense') {
        return TransactionType.Expense;
    }

    if (value === TransactionType.Income || value === 'income') {
        return TransactionType.Income;
    }

    if (value === TransactionType.Transfer || value === 'transfer') {
        return TransactionType.Transfer;
    }

    if (value === TransactionType.Investment || value === 'investment') {
        return TransactionType.Investment;
    }

    return undefined;
}

export function getReceiptDraftCandidateDisplayValue(field: ReceiptDraftField): string {
    if (field.label) {
        return field.label;
    }

    if (Array.isArray(field.value)) {
        return field.value.join(', ');
    }

    return String(field.value);
}

export function applyReceiptDraftFieldToTransaction(transaction: ReceiptDraftEditableTransaction, key: ReceiptDraftCandidateKey, field: ReceiptDraftField): boolean {
    if (key === 'type') {
        const draftType = transactionTypeFromReceiptDraftValue(field.value);
        if (draftType) {
            transaction.type = draftType;
            return true;
        }

        return false;
    }

    if (key === 'amount') {
        const amountInCents = receiptDraftAmountToCents(field);
        if (amountInCents !== undefined) {
            transaction.sourceAmount = amountInCents;
            return true;
        }

        return false;
    }

    if (key === 'time') {
        const time = receiptDraftTimeToUnixSeconds(field);
        if (time !== undefined) {
            transaction.time = time;
            return true;
        }

        return false;
    }

    if (key === 'description') {
        const description = receiptDraftFieldStringValue(field);
        if (description) {
            transaction.comment = description;
            return true;
        }

        return false;
    }

    if (key === 'categoryId') {
        const categoryId = receiptDraftFieldStringValue(field);
        if (categoryId) {
            transaction.setCategoryId(categoryId);
            return true;
        }

        return false;
    }

    if (key === 'sourceAccountId') {
        const accountId = receiptDraftFieldStringValue(field);
        if (accountId) {
            transaction.sourceAccountId = accountId;
            return true;
        }

        return false;
    }

    if (key === 'destinationAccountId') {
        const accountId = receiptDraftFieldStringValue(field);
        if (accountId) {
            transaction.destinationAccountId = accountId;
            return true;
        }

        return false;
    }

    const tagIds = receiptDraftFieldStringArrayValue(field);
    if (tagIds?.length) {
        transaction.tagIds = Array.from(new Set([...transaction.tagIds, ...tagIds]));
        return true;
    }

    return false;
}

export function applyReceiptDraftAutoFillToTransaction(transaction: ReceiptDraftEditableTransaction, result: RecognizedReceiptImageResponse): void {
    const autoFill = result.draft?.autoFill;

    if (autoFill?.type) {
        applyReceiptDraftFieldToTransaction(transaction, 'type', autoFill.type);
    }

    if (autoFill?.categoryId) {
        applyReceiptDraftFieldToTransaction(transaction, 'categoryId', autoFill.categoryId);
    }

    if (autoFill?.sourceAccountId) {
        applyReceiptDraftFieldToTransaction(transaction, 'sourceAccountId', autoFill.sourceAccountId);
    }

    if (autoFill?.destinationAccountId) {
        applyReceiptDraftFieldToTransaction(transaction, 'destinationAccountId', autoFill.destinationAccountId);
    }

    if (autoFill?.tagIds) {
        applyReceiptDraftFieldToTransaction(transaction, 'tagIds', autoFill.tagIds);
    }

    if (autoFill?.amount) {
        applyReceiptDraftFieldToTransaction(transaction, 'amount', autoFill.amount);
    } else if (typeof result.amount === 'number' && Number.isFinite(result.amount)) {
        transaction.sourceAmount = Math.round(result.amount * 100);
    }

    if (autoFill?.time) {
        applyReceiptDraftFieldToTransaction(transaction, 'time', autoFill.time);
    } else if (result.tradeTime) {
        const parsedMs = Date.parse(result.tradeTime);
        if (!Number.isNaN(parsedMs)) {
            transaction.time = Math.floor(parsedMs / 1000);
        }
    }

    if (autoFill?.description) {
        applyReceiptDraftFieldToTransaction(transaction, 'description', autoFill.description);
    } else if (result.description) {
        transaction.comment = result.description;
    }
}

export function buildReceiptDraftCandidateHints(draft: ReceiptTransactionDraft | undefined): ReceiptDraftCandidateHint[] {
    if (!draft) {
        return [];
    }

    const hints: ReceiptDraftCandidateHint[] = [];
    const append = (key: ReceiptDraftCandidateKey, fields: ReceiptDraftField[] | undefined): void => {
        if (!fields?.length) {
            return;
        }

        fields.forEach((field, index) => {
            hints.push({
                id: `${key}-${index}-${field.confidence}-${getReceiptDraftCandidateDisplayValue(field)}`,
                key,
                titleKey: RECEIPT_DRAFT_CANDIDATE_TITLES[key],
                field
            });
        });
    };

    append('type', draft.candidates.type);
    append('amount', draft.candidates.amount);
    append('time', draft.candidates.time);
    append('description', draft.candidates.description);
    append('categoryId', draft.candidates.categoryId);
    append('sourceAccountId', draft.candidates.sourceAccountId);
    append('destinationAccountId', draft.candidates.destinationAccountId);
    append('tagIds', draft.candidates.tagIds);

    return hints;
}
