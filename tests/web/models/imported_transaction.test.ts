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
            reason: 'dedup_pair',
            review_status: 'pending',
            reviewed_type: '',
            suppressed: false
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
        expect(transaction.hasMatchingContextSummary()).toBe(true);
        expect(transaction.getMatchingDedupSummary()).toBe('transfer | 1|2');
        expect(transaction.getMatchingParserTagText()).toBe('parser:wechat · channel:wallet');
        expect(transaction.actualCategoryName).toBe('餐饮');
        expect(transaction.actualSourceAccountName).toBe('微信零钱');
        expect(transaction.index).toBe(3);
        expect(transaction.selected).toBe(false);
        expect(transaction.valid).toBe(true);
    });

    test('ImportTransaction.of preserves backend preview selected state', () => {
        const selected = ImportTransaction.of({
            ...BASE_RESPONSE,
            selected: true
        }, 4);
        const unselected = ImportTransaction.of({
            ...BASE_RESPONSE,
            selected: false
        }, 5);

        expect(selected.selected).toBe(true);
        expect(unselected.selected).toBe(false);
    });

    test('partial matching payloads are normalized without throwing', () => {
        const parserOnly = ImportTransaction.of({
            ...BASE_RESPONSE,
            parserSource: '',
            parserTags: [],
            matching: {
                parser: {
                    parser_id: 'alipay',
                    parser_tags: ['parser:alipay']
                },
                dedup: {
                    type: 'remaining',
                    source_ids: []
                }
            } as unknown as ImportTransactionResponse['matching']
        }, 4);
        const dedupOnly = ImportTransaction.of({
            ...BASE_RESPONSE,
            matching: {
                dedup: {
                    type: 'duplicate',
                    source_ids: '7, 8'
                }
            } as unknown as ImportTransactionResponse['matching']
        }, 5);

        expect(parserOnly.parserSource).toBe('alipay');
        expect(parserOnly.parserTags).toStrictEqual(['parser:alipay']);
        expect(parserOnly.matching?.transfer.candidate_type).toBe('');
        expect(parserOnly.matching?.learning.rule_id).toBeNull();
        expect(parserOnly.hasTransferSuggestion()).toBe(false);

        expect(dedupOnly.dedupType).toBe('duplicate');
        expect(dedupOnly.dedupSourceIds).toStrictEqual([7, 8]);
        expect(dedupOnly.matching?.annotation.is_manually_annotated).toBe(false);
    });

    test('transfer-protected skipped learning feedback is hidden from recommendations', () => {
        const transaction = ImportTransaction.of({
            ...BASE_RESPONSE,
            learningRecommendationReason: 'transfer preview is protected from learning type/category overrides',
            matching: {
                ...BASE_RESPONSE.matching!,
                learning: {
                    rule_id: null,
                    score: 0,
                    level: '',
                    review_status: 'skipped',
                    reason: 'transfer preview is protected from learning type/category overrides',
                    recommended_type: '',
                    summary: '',
                    auto_apply: false,
                    source: 'import_learning_rules'
                }
            }
        }, 6);

        expect(transaction.hasLearningRecommendation()).toBe(false);
        expect(transaction.isLearningRecommendationSkipped()).toBe(true);
        expect(transaction.isTransferProtectedLearningSkip()).toBe(true);
        expect(transaction.hasPendingLearningRecommendation()).toBe(false);
    });

    test('transfer typed rows without transfer match keep default skipped learning visibility', () => {
        const transaction = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.Transfer,
            dedupType: 'remaining',
            learningRecommendationReason: 'transfer preview is protected from learning type/category overrides',
            matching: {
                ...BASE_RESPONSE.matching!,
                transfer: {
                    ...BASE_RESPONSE.matching!.transfer,
                    candidate_type: '',
                    review_status: ''
                },
                dedup: {
                    type: 'remaining',
                    source_ids: []
                },
                learning: {
                    rule_id: null,
                    score: 0,
                    level: '',
                    review_status: 'skipped',
                    reason: 'transfer preview is protected from learning type/category overrides',
                    recommended_type: '',
                    summary: '',
                    auto_apply: false,
                    source: 'import_learning_rules'
                }
            }
        }, 7);

        expect(transaction.hasLearningRecommendation()).toBe(true);
        expect(transaction.isLearningRecommendationSkipped()).toBe(true);
        expect(transaction.isTransferProtectedLearningSkip()).toBe(false);
        expect(transaction.hasPendingLearningRecommendation()).toBe(false);
    });

    test('cash transfer matching payloads are treated as transfer-protected signals', () => {
        const transaction = ImportTransaction.of({
            ...BASE_RESPONSE,
            suggestedType: undefined,
            transferSuggestionScore: 0,
            learningRecommendationReason: 'transfer preview is protected from learning type/category overrides',
            matching: {
                ...BASE_RESPONSE.matching!,
                transfer: {
                    ...BASE_RESPONSE.matching!.transfer,
                    candidate_type: 'cash_transfer',
                    review_status: 'pending'
                },
                dedup: {
                    type: '',
                    source_ids: []
                },
                learning: {
                    rule_id: null,
                    score: 0,
                    level: '',
                    review_status: 'skipped',
                    reason: 'transfer preview is protected from learning type/category overrides',
                    recommended_type: '',
                    summary: '',
                    auto_apply: false,
                    source: 'import_learning_rules'
                }
            }
        }, 7);

        expect(transaction.suggestedType).toBe(TransactionType.Transfer);
        expect(transaction.hasLearningRecommendation()).toBe(false);
        expect(transaction.isTransferProtectedLearningSkip()).toBe(true);
    });

    test('non-transfer skipped learning feedback remains visible without review actions', () => {
        const transaction = ImportTransaction.of({
            ...BASE_RESPONSE,
            dedupType: 'remaining',
            matching: {
                ...BASE_RESPONSE.matching!,
                transfer: {
                    ...BASE_RESPONSE.matching!.transfer,
                    candidate_type: '',
                    review_status: ''
                },
                dedup: {
                    type: 'remaining',
                    source_ids: []
                },
                learning: {
                    rule_id: 8,
                    score: 0,
                    level: '',
                    review_status: 'skipped',
                    reason: 'learned category type is incompatible with preview type',
                    recommended_type: '',
                    summary: '',
                    auto_apply: false,
                    source: 'import_learning_rules'
                }
            }
        }, 7);

        expect(transaction.hasLearningRecommendation()).toBe(true);
        expect(transaction.isLearningRecommendationSkipped()).toBe(true);
        expect(transaction.isTransferProtectedLearningSkip()).toBe(false);
        expect(transaction.hasPendingLearningRecommendation()).toBe(false);
    });

    test('missing matching payload keeps flat fields and decision reset safe', () => {
        const transaction = ImportTransaction.of({
            ...BASE_RESPONSE,
            matching: undefined,
            suggestedType: TransactionType.Transfer,
            transferSuggestionScore: 0.8,
            dedupType: 'duplicate',
            dedupSourceIds: '9, 10'
        }, 6);

        expect(transaction.matching).toBeUndefined();
        expect(transaction.parserSource).toBe('wechat');
        expect(transaction.parserTags).toStrictEqual(['parser:wechat', 'channel:wallet']);
        expect(transaction.dedupType).toBe('duplicate');
        expect(transaction.dedupSourceIds).toStrictEqual([9, 10]);
        expect(() => transaction.resetTransferSuggestionDecisionState()).not.toThrow();
        expect(transaction.hasMatchingContextSummary()).toBe(true);
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
                    reason: 'dedup_pair',
                    review_status: 'pending',
                    reviewed_type: '',
                    suppressed: false
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

    test('transfer review decisions suppress rejected suggestions and keep accepted decisions clearable', () => {
        const rejected = ImportTransaction.of({
            ...BASE_RESPONSE,
            matching: {
                ...BASE_RESPONSE.matching!,
                transfer: {
                    ...BASE_RESPONSE.matching!.transfer,
                    review_status: 'rejected',
                    reviewed_type: '',
                    suppressed: true
                }
            }
        }, 7);
        const accepted = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.Transfer,
            destinationAccountId: '202',
            destinationAmount: 1200,
            matching: {
                ...BASE_RESPONSE.matching!,
                transfer: {
                    ...BASE_RESPONSE.matching!.transfer,
                    review_status: 'accepted',
                    reviewed_type: '转账',
                    suppressed: false
                }
            }
        }, 8);

        expect(rejected.getTransferSuggestionReviewStatus()).toBe('rejected');
        expect(rejected.isTransferSuggestionRejected()).toBe(true);
        expect(rejected.isTransferSuggestionSuppressed()).toBe(true);
        expect(rejected.hasTransferSuggestion()).toBe(false);
        expect(rejected.canClearTransferSuggestionDecision()).toBe(true);

        expect(accepted.getTransferSuggestionReviewStatus()).toBe('accepted');
        expect(accepted.isTransferSuggestionAccepted()).toBe(true);
        expect(accepted.hasTransferSuggestion()).toBe(false);
        expect(accepted.canClearTransferSuggestionDecision()).toBe(true);

        rejected.resetTransferSuggestionDecisionState();
        accepted.resetTransferSuggestionDecisionState();

        expect(rejected.getTransferSuggestionReviewStatus()).toBe('pending');
        expect(rejected.isTransferSuggestionRejected()).toBe(false);
        expect(rejected.isTransferSuggestionSuppressed()).toBe(false);
        expect(rejected.hasTransferSuggestion()).toBe(true);
        expect(rejected.canClearTransferSuggestionDecision()).toBe(false);

        expect(accepted.getTransferSuggestionReviewStatus()).toBe('pending');
        expect(accepted.isTransferSuggestionAccepted()).toBe(false);
        expect(accepted.canClearTransferSuggestionDecision()).toBe(false);
    });

    test('learning review helpers expose pending, accepted, and rejected states', () => {
        const pending = ImportTransaction.of({
            ...BASE_RESPONSE,
            matching: {
                ...BASE_RESPONSE.matching!,
                learning: {
                    ...BASE_RESPONSE.matching!.learning,
                    rule_id: 12,
                    score: 0.73,
                    level: 'medium',
                    reason: 'composite_match',
                    recommended_type: '支出',
                    summary: '餐饮 | 午餐',
                    review_status: 'pending',
                    suppressed: false
                }
            }
        }, 9);
        const accepted = ImportTransaction.of({
            ...BASE_RESPONSE,
            matching: {
                ...BASE_RESPONSE.matching!,
                learning: {
                    ...BASE_RESPONSE.matching!.learning,
                    rule_id: 13,
                    score: 0.85,
                    level: 'high',
                    reason: 'composite_match',
                    recommended_type: '支出',
                    summary: '餐饮 | 晚餐',
                    review_status: 'accepted',
                    suppressed: false
                }
            }
        }, 10);
        const rejected = ImportTransaction.of({
            ...BASE_RESPONSE,
            matching: {
                ...BASE_RESPONSE.matching!,
                learning: {
                    ...BASE_RESPONSE.matching!.learning,
                    rule_id: 14,
                    score: 0.61,
                    level: 'medium',
                    reason: 'composite_match',
                    recommended_type: '支出',
                    summary: '餐饮 | 咖啡',
                    review_status: 'rejected',
                    suppressed: true
                }
            }
        }, 11);

        expect(pending.hasLearningRecommendation()).toBe(true);
        expect(pending.hasPendingLearningRecommendation()).toBe(true);
        expect(pending.getLearningRecommendationReviewStatus()).toBe('pending');
        expect(pending.isLearningRecommendationAccepted()).toBe(false);
        expect(pending.isLearningRecommendationRejected()).toBe(false);
        expect(pending.canClearLearningRecommendationDecision()).toBe(false);

        expect(accepted.hasLearningRecommendation()).toBe(true);
        expect(accepted.hasPendingLearningRecommendation()).toBe(false);
        expect(accepted.isLearningRecommendationAccepted()).toBe(true);
        expect(accepted.canClearLearningRecommendationDecision()).toBe(true);

        expect(rejected.hasLearningRecommendation()).toBe(true);
        expect(rejected.hasPendingLearningRecommendation()).toBe(false);
        expect(rejected.isLearningRecommendationRejected()).toBe(true);
        expect(rejected.isLearningRecommendationSuppressed()).toBe(true);
        expect(rejected.canClearLearningRecommendationDecision()).toBe(true);
    });

    test('investment review helpers expose pending, accepted, and rejected states', () => {
        const pending = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.Investment,
            destinationAccountId: '203',
            destinationAmount: 1200,
            matching: {
                ...BASE_RESPONSE.matching!,
                investment: {
                    ...BASE_RESPONSE.matching!.investment,
                    score: 0.81,
                    level: 'high',
                    reason: 'investment_keyword',
                    platform: '蚂蚁财富',
                    product: '黄金ETF',
                    review_status: 'pending',
                    suppressed: false
                }
            }
        }, 15);
        const accepted = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.Investment,
            destinationAccountId: '204',
            destinationAmount: 1200,
            matching: {
                ...BASE_RESPONSE.matching!,
                investment: {
                    ...BASE_RESPONSE.matching!.investment,
                    score: 0.83,
                    level: 'high',
                    reason: 'investment_keyword',
                    platform: '蚂蚁财富',
                    product: '黄金ETF',
                    review_status: 'accepted',
                    suppressed: false
                }
            }
        }, 16);
        const rejected = ImportTransaction.of({
            ...BASE_RESPONSE,
            type: TransactionType.Investment,
            destinationAccountId: '205',
            destinationAmount: 1200,
            matching: {
                ...BASE_RESPONSE.matching!,
                investment: {
                    ...BASE_RESPONSE.matching!.investment,
                    score: 0.79,
                    level: 'medium',
                    reason: 'investment_keyword',
                    platform: '蚂蚁财富',
                    product: '黄金ETF',
                    review_status: 'rejected',
                    suppressed: true
                }
            }
        }, 17);

        expect(pending.hasInvestmentSignal()).toBe(true);
        expect(pending.hasPendingInvestmentSignal()).toBe(true);
        expect(pending.getInvestmentSignalReviewStatus()).toBe('pending');
        expect(pending.isInvestmentSignalAccepted()).toBe(false);
        expect(pending.isInvestmentSignalRejected()).toBe(false);
        expect(pending.isInvestmentSignalSuppressed()).toBe(false);

        expect(accepted.hasInvestmentSignal()).toBe(true);
        expect(accepted.hasPendingInvestmentSignal()).toBe(false);
        expect(accepted.isInvestmentSignalAccepted()).toBe(true);
        expect(accepted.isInvestmentSignalRejected()).toBe(false);
        expect(accepted.isInvestmentSignalSuppressed()).toBe(false);

        expect(rejected.hasInvestmentSignal()).toBe(true);
        expect(rejected.hasPendingInvestmentSignal()).toBe(false);
        expect(rejected.isInvestmentSignalAccepted()).toBe(false);
        expect(rejected.isInvestmentSignalRejected()).toBe(true);
        expect(rejected.isInvestmentSignalSuppressed()).toBe(true);
    });

    test('learning input fingerprint tracks parser-aware text fields', () => {
        const base = ImportTransaction.of(BASE_RESPONSE, 12);
        const same = ImportTransaction.of({
            ...BASE_RESPONSE,
            comment: '工作日午餐',
            counterparty: '兰州拉面',
            paymentMethod: '微信支付',
            parserSource: 'wechat'
        }, 13);
        const changed = ImportTransaction.of({
            ...BASE_RESPONSE,
            comment: '周末午餐'
        }, 14);

        expect(base.getLearningRecommendationInputFingerprint()).toBe(same.getLearningRecommendationInputFingerprint());
        expect(base.getLearningRecommendationInputFingerprint()).not.toBe(changed.getLearningRecommendationInputFingerprint());
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
        const missingCategory = ImportTransaction.of({
            ...BASE_RESPONSE,
            categoryId: ''
        }, 6);
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
        expect(missingCategory.valid).toBe(false);
        expect(missingSource.valid).toBe(false);
        expect(blankSource.valid).toBe(false);
        expect(missingDestination.valid).toBe(false);
        expect(invalidTags.valid).toBe(false);
        expect(validTransfer.valid).toBe(true);
    });
});
