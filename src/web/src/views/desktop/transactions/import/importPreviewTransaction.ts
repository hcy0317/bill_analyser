import { TransactionType } from '@/core/transaction.ts';
import { getTimezoneOffsetMinutes } from '@/lib/datetime.ts';
import { ImportTransaction, type ImportTransactionResponse } from '@/models/imported_transaction.ts';

import {
    resolveImportPreviewCategoryId,
    resolveImportPreviewCategoryPath,
    resolveImportPreviewDefaultTransferCategoryId,
    type ImportPreviewCategoryLike,
    type ImportPreviewCategoryMap,
    type ImportPreviewRecord
} from './importPreview.ts';
import { normalizeStrictAbsoluteCents } from './strictCents.ts';

export type ImportPreviewTransactionDraft = ImportTransaction & {
    _previewId?: number;
    _shouldClearTransferDecision?: boolean;
    _shouldClearLearningDecision?: boolean;
    _shouldClearLlmDecision?: boolean;
};

export interface BuildImportTransactionFromPreviewRecordOptions {
    categoriesById: ImportPreviewCategoryMap;
    transferCategories?: ImportPreviewCategoryLike[];
    cashTransferCategoryId?: number | string | null;
    timeZone: string;
    nowInSeconds?: () => number;
}

export function getImportPreviewTransactionTypeNumber(previewType?: string): TransactionType | undefined {
    const normalizedPreviewType = (previewType || '').trim().toLowerCase();

    switch (normalizedPreviewType) {
        case '收入':
        case 'income':
        case 'refund':
        case '退款':
        case '2':
            return TransactionType.Income;
        case '支出':
        case 'expense':
        case '3':
            return TransactionType.Expense;
        case '转账':
        case 'transfer':
        case '4':
            return TransactionType.Transfer;
        case '投资':
        case 'investment':
        case '5':
            return TransactionType.Investment;
        default:
            return undefined;
    }
}

export function getImportPreviewTransactionTypeLabel(transactionType: number): string {
    switch (transactionType) {
        case TransactionType.Income:
            return '收入';
        case TransactionType.Transfer:
            return '转账';
        case TransactionType.Investment:
            return '投资';
        case TransactionType.Expense:
        default:
            return '支出';
    }
}

function getDefaultPreviewCategoryId(
    transactionType: number,
    options: BuildImportTransactionFromPreviewRecordOptions
): string {
    if (transactionType !== TransactionType.Transfer) {
        return '';
    }

    return resolveImportPreviewDefaultTransferCategoryId(
        options.categoriesById,
        options.transferCategories,
        options.cashTransferCategoryId
    );
}

function getPreviewTimeInSeconds(
    previewDate: string | undefined,
    nowInSeconds: (() => number) | undefined
): number {
    const time = new Date(previewDate || '').getTime() / 1000;
    if (!Number.isNaN(time)) {
        return time;
    }

    return nowInSeconds ? nowInSeconds() : Date.now() / 1000;
}

function normalizePreviewAccountId(rawAccountId: unknown): string {
    if (typeof rawAccountId === 'number' && Number.isFinite(rawAccountId) && rawAccountId > 0) {
        return String(Math.trunc(rawAccountId));
    }

    if (typeof rawAccountId === 'string') {
        const normalizedAccountId = rawAccountId.trim();
        return /^[1-9]\d*$/.test(normalizedAccountId) ? normalizedAccountId : '';
    }

    return '';
}

function rawPreviewIdentityText(rawValue: unknown): string {
    if (typeof rawValue === 'string') {
        return rawValue.trim();
    }

    if (typeof rawValue === 'number' && Number.isFinite(rawValue)) {
        return String(Math.trunc(rawValue));
    }

    return '';
}

function rawPreviewCategoryText(item: ImportPreviewRecord): string {
    return item.preview_sub_category
        || item.preview_main_category
        || rawPreviewIdentityText(item.category_id ?? item.categoryId);
}

export function buildImportTransactionFromPreviewRecord(
    item: ImportPreviewRecord,
    index: number,
    options: BuildImportTransactionFromPreviewRecordOptions
): ImportTransaction {
    const type = getImportPreviewTransactionTypeNumber(item.preview_type) ?? TransactionType.Expense;
    const amountInCents = normalizeStrictAbsoluteCents(item.preview_amount_cents, 0);
    const destAmountInCents = normalizeStrictAbsoluteCents(item.preview_destination_amount_cents, 0);
    const categoryId = resolveImportPreviewCategoryId(item, options.categoriesById)
        || getDefaultPreviewCategoryId(type, options);
    const categoryPath = resolveImportPreviewCategoryPath(categoryId, options.categoriesById);
    const sourceAccountId = normalizePreviewAccountId(item.preview_source_account_id);
    const destinationAccountId = normalizePreviewAccountId(item.preview_destination_account_id);
    const parserId = item.preview_parser_id || item.matching?.parser?.id || '';
    const parserTags = Array.isArray(item.preview_parser_tags)
        ? item.preview_parser_tags
        : (item.matching?.parser?.tags || []);
    const matchingPayload = item.matching || parserId || parserTags.length > 0
        ? {
            ...(item.matching || {}),
            parser: {
                ...(item.matching?.parser || {}),
                id: parserId,
                tags: parserTags
            }
        } as ImportTransactionResponse['matching']
        : undefined;

    const responseItem: ImportTransactionResponse = {
        type,
        categoryId,
        originalCategoryName: categoryPath?.displayCategory || rawPreviewCategoryText(item),
        time: getPreviewTimeInSeconds(item.preview_date, options.nowInSeconds),
        utcOffset: getTimezoneOffsetMinutes(options.timeZone),
        sourceAccountId,
        originalSourceAccountName: rawPreviewIdentityText(item.preview_source_account_id),
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId,
        originalDestinationAccountName: rawPreviewIdentityText(item.preview_destination_account_id),
        sourceAmountCents: amountInCents,
        destinationAmountCents: (type === TransactionType.Transfer || type === TransactionType.Investment)
            ? destAmountInCents || amountInCents
            : amountInCents,
        tagIds: [],
        originalTagNames: [],
        comment: item.preview_description || '',
        counterparty: item.preview_counterparty || '',
        paymentMethod: item.preview_payment_method || '',
        suggestedType: getImportPreviewTransactionTypeNumber(item.suggested_preview_type),
        transferSuggestionScore: Number(item.transfer_suggestion_score || 0),
        transferSuggestionLevel: item.transfer_suggestion_level || '',
        transferSuggestionReason: item.transfer_suggestion_reason || '',
        investmentSignalScore: Number(item.investment_signal_score || 0),
        investmentSignalLevel: item.investment_signal_level || '',
        investmentSignalReason: item.investment_signal_reason || '',
        learningRecommendationScore: Number(item.learning_recommendation_score || 0),
        learningRecommendationLevel: item.learning_recommendation_level || '',
        learningRecommendationReason: item.learning_recommendation_reason || '',
        learningRecommendationType: item.learning_recommendation_type || '',
        learningRecommendationSummary: item.learning_recommendation_summary || '',
        investmentPlatform: item.investment_platform || '',
        investmentProduct: item.investment_product || '',
        recurringTemplateId: item.preview_recurring_id ? String(item.preview_recurring_id) : '',
        recurringTemplateName: item.preview_recurring_name || '',
        recurringCandidateCount: Number(item.preview_recurring_candidate_count || 0),
        recurringMatchScore: Number(item.preview_recurring_match_score || 0),
        recurringMatchReasons: item.preview_recurring_match_reasons || '',
        recurringMatchedDate: item.preview_recurring_matched_date || '',
        dedupType: item.dedup_type || '',
        dedupSourceIds: item.dedup_source_ids || [],
        matching: matchingPayload,
        previewState: item.preview_state,
        isManuallyAnnotated: !!item.preview_is_manually_annotated,
        selected: !!(item.preview_selected ?? item.selected)
    };

    const previewIndex = typeof item.id === 'number' ? item.id : index;
    const transaction = ImportTransaction.of(responseItem, previewIndex) as ImportPreviewTransactionDraft;
    transaction._previewId = item.id;

    return transaction;
}
