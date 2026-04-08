import { describe, expect, test } from '@jest/globals';

import { TransactionType } from '@/core/transaction.ts';
import {
    ImportTransaction,
    type ImportTransactionResponse
} from '@/models/imported_transaction.ts';

const BASE_RESPONSE: ImportTransactionResponse = {
    type: TransactionType.Expense,
    categoryId: '101',
    originalCategoryName: '餐饮',
    time: 1711785600,
    utcOffset: 480,
    sourceAccountId: '201',
    originalSourceAccountName: '微信零钱',
    originalSourceAccountCurrency: 'CNY',
    destinationAccountId: '',
    originalDestinationAccountName: '',
    originalDestinationAccountCurrency: '',
    sourceAmount: 1200,
    destinationAmount: 0,
    tagIds: ['301'],
    originalTagNames: ['午饭'],
    comment: '工作日午餐',
    counterparty: '兰州拉面',
    paymentMethod: '微信支付',
    parserSource: 'wechat',
    parserTags: ['parser:wechat', 'channel:wallet'],
    matching: {
        transfer: {
            candidate_type: '转账',
            score: 0.88,
            level: 'high',
            reason: 'dedup_pair'
        },
        investment: {
            score: 0,
            level: '',
            reason: '',
            platform: '',
            product: ''
        },
        learning: {
            rule_id: null,
            score: 0,
            level: '',
            reason: '',
            recommended_type: '',
            summary: ''
        },
        recurring: {
            id: null,
            name: '',
            candidate_count: 0,
            match_score: 0,
            match_reasons: '',
            matched_date: ''
        },
        dedup: {
            type: 'transfer',
            source_ids: [1, 2]
        },
        parser: {
            id: 'wechat',
            tags: ['parser:wechat', 'channel:wallet']
        },
        annotation: {
            is_manually_annotated: true
        }
    },
    isManuallyAnnotated: true
};

describe('ImportTransaction model', () => {
    test('ImportTransaction.of maps fallback fields and derived display state', () => {
        const transaction = ImportTransaction.of(BASE_RESPONSE, 3);

        expect(transaction.categoryId).toBe('101');
        expect(transaction.destinationAccountId).toBe('');
        expect(transaction.destinationAmount).toBe(0);
        expect(transaction.tagIds).toStrictEqual(['301']);
        expect(transaction.originalTagNames).toStrictEqual(['午饭']);
        expect(transaction.counterparty).toBe('兰州拉面');
        expect(transaction.paymentMethod).toBe('微信支付');
        expect(transaction.parserSource).toBe('wechat');
        expect(transaction.parserTags).toStrictEqual(['parser:wechat', 'channel:wallet']);
        expect(transaction.matching?.transfer.candidate_type).toBe('转账');
        expect(transaction.matching?.parser.tags).toStrictEqual(['parser:wechat', 'channel:wallet']);
        expect(transaction.matching?.annotation.is_manually_annotated).toBe(true);
        expect(transaction.isManuallyAnnotated).toBe(true);
        expect(transaction.actualCategoryName).toBe('餐饮');
        expect(transaction.actualSourceAccountName).toBe('微信零钱');
        expect(transaction.index).toBe(3);
        expect(transaction.selected).toBe(false);
        expect(transaction.valid).toBe(true);
    });

    test('destination-account requirements apply to transfer and investment transactions', () => {
        const transfer = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.Transfer,
            destinationAccountId: '202',
            destinationAmount: 1200
        }, 0);
        const investment = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.Investment,
            destinationAccountId: '203',
            destinationAmount: 1200
        }, 1);
        const expense = ImportTransaction.of(BASE_RESPONSE, 2);

        expect(transfer.requiresDestinationAccount()).toBe(true);
        expect(investment.requiresDestinationAccount()).toBe(true);
        expect(expense.requiresDestinationAccount()).toBe(false);
    });

    test('suggestion and recommendation helpers report their status correctly', () => {
        const transferCandidate = ImportTransaction.of({
            ...BASE_RESPONSE,
            suggestedType: TransactionType.Transfer,
            transferSuggestionScore: 0.9,
            investmentSignalScore: 1,
            learningRecommendationScore: 2,
            investmentPlatform: '天天基金',
            investmentProduct: '沪深300',
            recurringTemplateId: 'tpl-1',
            recurringTemplateName: '工资日午餐',
            recurringCandidateCount: 3,
            recurringMatchScore: 0.88,
            recurringMatchReasons: '时间和金额接近',
            recurringMatchedDate: '2026-03-30'
        }, 0);

        expect(transferCandidate.hasTransferSuggestion()).toBe(true);
        expect(transferCandidate.hasInvestmentSignal()).toBe(false);
        expect(transferCandidate.hasLearningRecommendation()).toBe(true);
        expect(transferCandidate.getInvestmentProfileText()).toBe('天天基金 · 沪深300');
        expect(transferCandidate.hasRecurringMatch()).toBe(true);

        transferCandidate.clearRecurringMatch(false);
        expect(transferCandidate.recurringTemplateId).toBe('');
        expect(transferCandidate.recurringCandidateCount).toBe(3);
        expect(transferCandidate.recurringMatchScore).toBe(0);
        expect(transferCandidate.recurringMatchReasons).toBe('');
        expect(transferCandidate.recurringMatchedDate).toBe('');

        const recurringReset = ImportTransaction.of({
            ...BASE_RESPONSE,
            recurringTemplateId: 'tpl-2',
            recurringCandidateCount: 4,
            recurringMatchScore: 0.75,
            recurringMatchReasons: '规则命中',
            recurringMatchedDate: '2026-03-01'
        }, 1);
        recurringReset.clearRecurringMatch();
        expect(recurringReset.recurringCandidateCount).toBe(0);
    });

    test('matching payload becomes the primary display source when flat preview fields are absent', () => {
        const transaction = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.Investment,
            suggestedType: undefined,
            transferSuggestionScore: 0,
            transferSuggestionLevel: '',
            transferSuggestionReason: '',
            investmentSignalScore: 0,
            investmentSignalLevel: '',
            investmentSignalReason: '',
            learningRecommendationScore: 0,
            learningRecommendationLevel: '',
            learningRecommendationReason: '',
            learningRecommendationType: '',
            learningRecommendationSummary: '',
            investmentPlatform: '',
            investmentProduct: '',
            recurringTemplateId: '',
            recurringTemplateName: '',
            recurringCandidateCount: 0,
            recurringMatchScore: 0,
            recurringMatchReasons: '',
            recurringMatchedDate: '',
            parserSource: '',
            parserTags: [],
            isManuallyAnnotated: false,
            matching: {
                transfer: {
                    candidate_type: 'transfer',
                    score: 0.93,
                    level: 'high',
                    reason: 'dedup_pair'
                },
                investment: {
                    score: 0.81,
                    level: 'high',
                    reason: 'investment_keyword',
                    platform: '蚂蚁财富',
                    product: '黄金ETF'
                },
                learning: {
                    rule_id: 8,
                    score: 0.77,
                    level: 'medium',
                    reason: 'parser_id:exact',
                    recommended_type: '投资',
                    summary: '投资 | 投资理财/基金'
                },
                recurring: {
                    id: 9,
                    name: '每月定投',
                    candidate_count: 2,
                    match_score: 0.9,
                    match_reasons: 'date|amount',
                    matched_date: '2026-04-01'
                },
                dedup: {
                    type: 'transfer',
                    source_ids: [101, '102']
                },
                parser: {
                    id: 'alipay',
                    tags: ['parser:alipay', 'channel:wallet']
                },
                annotation: {
                    is_manually_annotated: true
                }
            }
        }, 6);

        expect(transaction.suggestedType).toBe(TransactionType.Transfer);
        expect(transaction.hasTransferSuggestion()).toBe(true);
        expect(transaction.transferSuggestionScore).toBe(0.93);
        expect(transaction.transferSuggestionReason).toBe('dedup_pair');

        expect(transaction.hasInvestmentSignal()).toBe(true);
        expect(transaction.investmentSignalScore).toBe(0.81);
        expect(transaction.getInvestmentProfileText()).toBe('蚂蚁财富 · 黄金ETF');

        expect(transaction.hasLearningRecommendation()).toBe(true);
        expect(transaction.learningRecommendationSummary).toBe('投资 | 投资理财/基金');

        expect(transaction.hasRecurringMatch()).toBe(true);
        expect(transaction.recurringTemplateId).toBe('9');
        expect(transaction.recurringCandidateCount).toBe(2);
        expect(transaction.recurringMatchReasons).toBe('date|amount');

        expect(transaction.parserSource).toBe('alipay');
        expect(transaction.parserTags).toStrictEqual(['parser:alipay', 'channel:wallet']);
        expect(transaction.dedupType).toBe('transfer');
        expect(transaction.dedupSourceIds).toStrictEqual([101, 102]);
        expect(transaction.hasMatchingDedupContext()).toBe(true);
        expect(transaction.isManuallyAnnotated).toBe(true);
    });

    test('toCreateRequest zeros destination fields for non-transfer transactions', () => {
        const expense = ImportTransaction.of(BASE_RESPONSE, 0);
        const transfer = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.Transfer,
            destinationAccountId: '202',
            destinationAmount: 1200
        }, 1);

        expect(expense.toCreateRequest()).toMatchObject({
            type: TransactionType.Expense,
            destinationAccountId: '0',
            destinationAmount: 0,
            hideAmount: false,
            pictureIds: [],
            clientSessionId: ''
        });
        expect(transfer.toCreateRequest()).toMatchObject({
            type: TransactionType.Transfer,
            destinationAccountId: '202',
            destinationAmount: 1200
        });
    });

    test('isTransactionValid rejects missing category, accounts, destination accounts, and invalid tags', () => {
        const modifyBalance = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.ModifyBalance,
            categoryId: '0'
        }, 0);
        const missingSource = ImportTransaction.of({
            ...BASE_RESPONSE,
            sourceAccountId: '0'
        }, 1);
        const blankSource = ImportTransaction.of({
            ...BASE_RESPONSE,
            sourceAccountId: ''
        }, 2);
        const missingDestination = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.Transfer,
            destinationAccountId: '0'
        }, 3);
        const invalidTags = ImportTransaction.of({
            ...BASE_RESPONSE,
            tagIds: ['301', '0']
        }, 4);
        const validTransfer = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.Transfer,
            destinationAccountId: '202',
            destinationAmount: 1200,
            tagIds: []
        }, 5);

        expect(modifyBalance.valid).toBe(true);
        expect(modifyBalance.isTransactionValid()).toBe(true);
        expect(missingSource.valid).toBe(false);
        expect(blankSource.valid).toBe(false);
        expect(missingDestination.valid).toBe(false);
        expect(invalidTags.valid).toBe(false);
        expect(validTransfer.valid).toBe(true);
    });
});
