import { describe, expect, test } from '@jest/globals';

import { CategoryType } from '@/core/category.ts';
import { TransactionType } from '@/core/transaction.ts';
import {
    buildImportTransactionFromPreviewRecord,
    getImportPreviewTransactionTypeLabel,
    getImportPreviewTransactionTypeNumber,
    type ImportPreviewTransactionDraft
} from '@/views/desktop/transactions/import/importPreviewTransaction.ts';
import type {
    ImportPreviewCategoryLike,
    ImportPreviewCategoryMap,
    ImportPreviewRecord
} from '@/views/desktop/transactions/import/importPreview.ts';

const categoriesById: ImportPreviewCategoryMap = {
    expenseParent: {
        id: 'expenseParent',
        name: '餐饮',
        parentId: '0',
        type: CategoryType.Expense
    },
    expenseSub: {
        id: 'expenseSub',
        name: '咖啡',
        parentId: 'expenseParent',
        type: CategoryType.Expense
    },
    transferParent: {
        id: 'transferParent',
        name: '账户互转',
        parentId: '0',
        type: CategoryType.Transfer
    },
    transferSub: {
        id: 'transferSub',
        name: '默认互转',
        parentId: 'transferParent',
        type: CategoryType.Transfer
    },
    investmentParent: {
        id: 'investmentParent',
        name: '投资',
        parentId: '0',
        type: CategoryType.Investment
    }
};

const transferCategories: ImportPreviewCategoryLike[] = [
    {
        id: 'transferParent',
        name: '账户互转',
        parentId: '0',
        type: CategoryType.Transfer,
        subCategories: [
            {
                id: 'transferSub',
                name: '默认互转',
                parentId: 'transferParent',
                type: CategoryType.Transfer
            }
        ]
    }
];

describe('import preview transaction helper', () => {
    test('normalizes preview type labels from backend text, numeric aliases, and refund rows', () => {
        expect(getImportPreviewTransactionTypeNumber('收入')).toBe(TransactionType.Income);
        expect(getImportPreviewTransactionTypeNumber('income')).toBe(TransactionType.Income);
        expect(getImportPreviewTransactionTypeNumber('退款')).toBe(TransactionType.Income);
        expect(getImportPreviewTransactionTypeNumber('2')).toBe(TransactionType.Income);
        expect(getImportPreviewTransactionTypeNumber('支出')).toBe(TransactionType.Expense);
        expect(getImportPreviewTransactionTypeNumber('transfer')).toBe(TransactionType.Transfer);
        expect(getImportPreviewTransactionTypeNumber('投资')).toBe(TransactionType.Investment);
        expect(getImportPreviewTransactionTypeNumber('unknown')).toBeUndefined();

        expect(getImportPreviewTransactionTypeLabel(TransactionType.Income)).toBe('收入');
        expect(getImportPreviewTransactionTypeLabel(TransactionType.Expense)).toBe('支出');
        expect(getImportPreviewTransactionTypeLabel(TransactionType.Transfer)).toBe('转账');
        expect(getImportPreviewTransactionTypeLabel(TransactionType.Investment)).toBe('投资');
        expect(getImportPreviewTransactionTypeLabel(TransactionType.ModifyBalance)).toBe('支出');
    });

    test('builds a transfer ImportTransaction from preview data with default category and parser context', () => {
        const transaction = buildImportTransactionFromPreviewRecord({
            id: 42,
            preview_type: '转账',
            suggested_preview_type: '投资',
            preview_date: '2026-06-01T00:00:00Z',
            preview_amount_cents: -1234,
            preview_destination_amount_cents: 0,
            preview_source_account_id: 101,
            preview_destination_account_id: 202,
            preview_description: '账户互转备注',
            preview_counterparty: 'Alice',
            preview_payment_method: '招商银行',
            transfer_suggestion_score: 0.91,
            transfer_suggestion_level: 'strong',
            transfer_suggestion_reason: '双边匹配',
            preview_recurring_id: 88,
            preview_recurring_name: '房租',
            preview_recurring_candidate_count: 2,
            preview_recurring_match_score: 0.75,
            preview_recurring_match_reasons: '周期相似',
            preview_recurring_matched_date: '2026-06-01',
            dedup_type: 'platform_duplicate',
            dedup_source_ids: '1, preview:2',
            preview_parser_id: 'wechat',
            preview_parser_tags: ['parser:wechat'],
            preview_is_manually_annotated: true,
            preview_selected: true
        }, 7, {
            categoriesById,
            transferCategories,
            cashTransferCategoryId: 'transferSub',
            timeZone: 'Asia/Shanghai'
        }) as ImportPreviewTransactionDraft;

        expect(transaction.type).toBe(TransactionType.Transfer);
        expect(transaction.categoryId).toBe('transferSub');
        expect(transaction.originalCategoryName).toBe('默认互转');
        expect(transaction.time).toBe(new Date('2026-06-01T00:00:00Z').getTime() / 1000);
        expect(transaction.utcOffset).toBe(480);
        expect(transaction.sourceAccountId).toBe('101');
        expect(transaction.destinationAccountId).toBe('202');
        expect(transaction.sourceAmountCents).toBe(1234);
        expect(transaction.destinationAmountCents).toBe(1234);
        expect(transaction.comment).toBe('账户互转备注');
        expect(transaction.counterparty).toBe('Alice');
        expect(transaction.paymentMethod).toBe('招商银行');
        expect(transaction.suggestedType).toBe(TransactionType.Investment);
        expect(transaction.transferSuggestionScore).toBe(0.91);
        expect(transaction.recurringTemplateId).toBe('88');
        expect(transaction.dedupSourceIds).toStrictEqual([1, 'preview:2']);
        expect(transaction.parserId).toBe('wechat');
        expect(transaction.parserTags).toStrictEqual(['parser:wechat']);
        expect(transaction.isManuallyAnnotated).toBe(true);
        expect(transaction.selected).toBe(true);
        expect(transaction._previewId).toBe(42);
        expect(transaction.index).toBe(42);
    });

    test('keeps structured transfer matching feedback visible as a signal', () => {
        const transaction = buildImportTransactionFromPreviewRecord({
            id: 43,
            preview_type: '转账',
            preview_date: '2026-06-01T00:00:00Z',
            preview_amount_cents: -1234,
            preview_destination_amount_cents: 1234,
            preview_source_account_id: 101,
            preview_destination_account_id: 202,
            matching: {
                transfer: {
                    candidate_type: 'cash_transfer',
                    score: 0,
                    level: '',
                    reason: '同金额双边匹配',
                    review_status: 'pending',
                    reviewed_type: '',
                    suppressed: false
                }
            } as unknown as ImportPreviewRecord['matching']
        }, 8, {
            categoriesById,
            transferCategories,
            cashTransferCategoryId: 'transferSub',
            timeZone: 'Asia/Shanghai'
        }) as ImportPreviewTransactionDraft;

        expect(transaction.type).toBe(TransactionType.Transfer);
        expect(transaction.transferSuggestionReason).toBe('同金额双边匹配');
        expect(transaction.getTransferSuggestionReviewStatus()).toBe('pending');
        expect(transaction.hasTransferSuggestion()).toBe(true);
    });

    test('preserves invalid-date fallback and expense category/name mapping', () => {
        const transaction = buildImportTransactionFromPreviewRecord({
            id: 9,
            category_id: 'expenseSub',
            preview_type: '支出',
            preview_date: 'not-a-date',
            preview_amount_cents: 235,
            preview_destination_amount_cents: 9900,
            preview_main_category: '餐饮',
            preview_sub_category: '咖啡',
            matching: {
                parser: {
                    id: 'alipay',
                    tags: ['parser:alipay'],
                    source_chain: []
                }
            } as unknown as ImportPreviewRecord['matching']
        }, 3, {
            categoriesById,
            transferCategories,
            cashTransferCategoryId: 'transferSub',
            timeZone: 'UTC',
            nowInSeconds: () => 12345
        });

        expect(transaction.type).toBe(TransactionType.Expense);
        expect(transaction.categoryId).toBe('expenseSub');
        expect(transaction.originalCategoryName).toBe('咖啡');
        expect(transaction.time).toBe(12345);
        expect(transaction.utcOffset).toBe(0);
        expect(transaction.sourceAmountCents).toBe(235);
        expect(transaction.destinationAmountCents).toBe(235);
        expect(transaction.parserId).toBe('alipay');
        expect(transaction.parserTags).toStrictEqual(['parser:alipay']);
    });

    test('keeps illegal category and account raw text out of canonical ids', () => {
        const transaction = buildImportTransactionFromPreviewRecord({
            id: 10,
            category_id: '/',
            preview_type: '支出',
            preview_date: '2026-06-01T00:00:00Z',
            preview_amount_cents: 235,
            preview_main_category: '/',
            preview_sub_category: '民生银行储蓄卡(6332)',
            preview_source_account_id: '民生银行储蓄卡(6332)',
            preview_destination_account_id: '/',
            preview_payment_method: '民生银行储蓄卡(6332)'
        }, 4, {
            categoriesById,
            transferCategories,
            cashTransferCategoryId: 'transferSub',
            timeZone: 'UTC'
        });

        expect(transaction.categoryId).toBe('');
        expect(transaction.originalCategoryName).toBe('民生银行储蓄卡(6332)');
        expect(transaction.sourceAccountId).toBe('');
        expect(transaction.destinationAccountId).toBe('');
        expect(transaction.originalSourceAccountName).toBe('民生银行储蓄卡(6332)');
        expect(transaction.originalDestinationAccountName).toBe('/');
        expect(transaction.paymentMethod).toBe('民生银行储蓄卡(6332)');
    });

    test('does not use payment-method placeholders as account or category identity fallbacks', () => {
        const transaction = buildImportTransactionFromPreviewRecord({
            id: 11,
            category_id: null,
            preview_type: '支出',
            preview_date: '2026-06-01T00:00:00Z',
            preview_amount_cents: 235,
            preview_main_category: '',
            preview_sub_category: '',
            preview_source_account_id: null,
            preview_destination_account_id: null,
            preview_payment_method: 'POS机'
        }, 5, {
            categoriesById,
            transferCategories,
            cashTransferCategoryId: 'transferSub',
            timeZone: 'UTC'
        });

        expect(transaction.categoryId).toBe('');
        expect(transaction.originalCategoryName).toBe('');
        expect(transaction.sourceAccountId).toBe('');
        expect(transaction.originalSourceAccountName).toBe('');
        expect(transaction.paymentMethod).toBe('POS机');
    });

    test('uses explicit investment destination amount instead of source fallback', () => {
        const transaction = buildImportTransactionFromPreviewRecord({
            id: 77,
            category_id: 'investmentParent',
            preview_type: '投资',
            preview_amount_cents: 1000,
            preview_destination_amount_cents: 1250,
            preview_date: '2026-06-02T00:00:00Z'
        }, 0, {
            categoriesById,
            timeZone: 'UTC'
        });

        expect(transaction.type).toBe(TransactionType.Investment);
        expect(transaction.sourceAmountCents).toBe(1000);
        expect(transaction.destinationAmountCents).toBe(1250);
    });

    test.each([
        ['decimal number', 12.34],
        ['decimal string', '12.34'],
        ['boolean', true],
        ['object', {}]
    ])('does not truncate invalid preview cents from backend payloads: %s', (_name, value) => {
        const transaction = buildImportTransactionFromPreviewRecord({
            id: 78,
            category_id: 'investmentParent',
            preview_type: '投资',
            preview_amount_cents: value as number,
            preview_destination_amount_cents: value as number,
            preview_date: '2026-06-02T00:00:00Z'
        }, 0, {
            categoriesById,
            timeZone: 'UTC'
        });

        expect(transaction.sourceAmountCents).toBe(0);
        expect(transaction.destinationAmountCents).toBe(0);
    });
});
