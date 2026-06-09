import { describe, expect, test } from '@jest/globals';

import {
    buildBillMatchingViewState,
    getBillMatchingCandidateBillAmountCents,
    getBillMatchingCandidateBillCategoryLabel,
    getBillMatchingCandidateBillSubtitleParts,
    getBillMatchingCandidateBillTitle,
    isBillMatchingCandidateReviewActionSupported,
    normalizeBillMatchingCandidatesResponse,
    normalizeBillMatchingFeedbackResponse
} from '@/models/bill_matching.ts';

describe('bill_matching model helpers', () => {
    test('normalizes bill-scoped matching candidates and linkedPair payloads', () => {
        const response = normalizeBillMatchingCandidatesResponse({
            billId: 101,
            linkedPair: {
                id: 55,
                pairType: 'investment',
                source: 'manual',
                leftBillId: 101,
                rightBillId: 202,
                otherBillId: 202
            },
            candidates: [
                {
                    candidateId: 'bill:101:investment:202',
                    kind: 'investment',
                    billId: 202,
                    score: 0.91,
                    level: 'high',
                    reason: 'investment_keyword',
                    bill: {
                        id: 202,
                        type: '投资',
                        amount: 166,
                        date: '2026-07-14 12:02:00',
                        description: '蚂蚁财富 手工投资配对 卖出',
                        counterparty: '蚂蚁财富',
                        paymentMethod: '支付宝'
                    }
                },
                {
                    candidateId: 'bill:101:learning:9:2',
                    kind: 'learning',
                    score: 0.72,
                    level: 'medium',
                    reason: 'composite_match',
                    ruleId: 9,
                    recommendedType: '支出',
                    summary: '餐饮 | 咖啡',
                    suppressed: false
                }
            ]
        });

        expect(response.billId).toBe(101);
        expect(response.linkedPair).toStrictEqual({
            id: 55,
            pairType: 'investment',
            source: 'manual',
            leftBillId: 101,
            rightBillId: 202,
            otherBillId: 202
        });
        expect(response.candidates).toHaveLength(2);
        expect(response.candidates[0]).toMatchObject({
            candidateId: 'bill:101:investment:202',
            kind: 'investment',
            score: 0.91,
            billId: 202
        });
        expect(response.candidates[0]!.bill).toMatchObject({
            id: 202,
            description: '蚂蚁财富 手工投资配对 卖出',
            mainCategory: '',
            subCategory: '',
            sourceAccountId: 0,
            destinationAccountId: 0
        });
        expect(response.candidates[1]).toMatchObject({
            candidateId: 'bill:101:learning:9:2',
            kind: 'learning',
            ruleId: 9,
            recommendedType: '支出',
            summary: '餐饮 | 咖啡'
        });
    });

    test('normalizes feedback events into a stable read model', () => {
        const response = normalizeBillMatchingFeedbackResponse({
            billId: 101,
            events: [
                {
                    id: 88,
                    candidateId: 'bill:101:investment:202',
                    action: 'accept',
                    createdAt: '2026-07-14T12:03:00',
                    payload: {
                        pair: {
                            id: 55,
                            pairType: 'investment'
                        }
                    }
                }
            ]
        });

        expect(response.billId).toBe(101);
        expect(response.events).toStrictEqual([
            {
                id: 88,
                candidateId: 'bill:101:investment:202',
                action: 'accept',
                createdAt: '2026-07-14T12:03:00',
                payload: {
                    pair: {
                        id: 55,
                        pairType: 'investment'
                    }
                }
            }
        ]);
    });

    test('builds linked mode view state when linkedPair exists', () => {
        const state = buildBillMatchingViewState(normalizeBillMatchingCandidatesResponse({
            billId: 101,
            linkedPair: {
                id: 55,
                pairType: 'transfer',
                source: 'manual',
                leftBillId: 101,
                rightBillId: 202,
                otherBillId: 202
            },
            candidates: []
        }));

        expect(state.mode).toBe('linked');
        expect(state.hasLinkedPair).toBe(true);
        expect(state.hasCandidates).toBe(false);
        expect(state.showCandidateActions).toBe(false);
        expect(state.showDeletePairAction).toBe(true);
        expect(state.primaryCandidate).toBeNull();
    });

    test('builds candidate mode view state and picks the highest-score candidate', () => {
        const state = buildBillMatchingViewState(normalizeBillMatchingCandidatesResponse({
            billId: 101,
            linkedPair: null,
            candidates: [
                {
                    candidateId: 'bill:101:transfer:202',
                    kind: 'transfer',
                    billId: 202,
                    score: 0.78,
                    level: 'medium',
                    reason: 'amount_match'
                },
                {
                    candidateId: 'bill:101:investment:303',
                    kind: 'investment',
                    billId: 303,
                    score: 0.91,
                    level: 'high',
                    reason: 'investment_keyword'
                }
            ]
        }));

        expect(state.mode).toBe('candidates');
        expect(state.hasLinkedPair).toBe(false);
        expect(state.hasCandidates).toBe(true);
        expect(state.candidateCount).toBe(2);
        expect(state.showCandidateActions).toBe(true);
        expect(state.showDeletePairAction).toBe(false);
        expect(state.primaryCandidate).toMatchObject({
            candidateId: 'bill:101:investment:303',
            kind: 'investment',
            score: 0.91
        });
    });

    test('builds empty mode view state when there is no linkedPair and no candidates', () => {
        const state = buildBillMatchingViewState(normalizeBillMatchingCandidatesResponse({
            billId: 101,
            linkedPair: null,
            candidates: []
        }));

        expect(state.mode).toBe('empty');
        expect(state.hasLinkedPair).toBe(false);
        expect(state.hasCandidates).toBe(false);
        expect(state.candidateCount).toBe(0);
        expect(state.primaryCandidate).toBeNull();
    });

    test('builds bill-format candidate display values before raw matching fields', () => {
        const response = normalizeBillMatchingCandidatesResponse({
            billId: 101,
            linkedPair: null,
            candidates: [{
                candidateId: 'bill:101:reconciliation_duplicate:202',
                kind: 'reconciliation_duplicate',
                score: 0.98,
                level: 'high',
                reason: 'same_amount',
                summary: '原始信息：支付宝收款',
                bill: {
                    id: 202,
                    type: '收入',
                    amount: 88.5,
                    date: '2026-05-20',
                    description: '支付宝收款',
                    counterparty: '张三',
                    paymentMethod: '支付宝',
                    mainCategory: '经营',
                    subCategory: '销售',
                    sourceAccountId: 12,
                    destinationAccountId: 0
                }
            }]
        });
        const candidate = response.candidates[0]!;

        expect(getBillMatchingCandidateBillTitle(candidate)).toBe('支付宝收款');
        expect(getBillMatchingCandidateBillCategoryLabel(candidate.bill)).toBe('经营 / 销售');
        expect(getBillMatchingCandidateBillSubtitleParts(candidate)).toEqual([
            '2026-05-20',
            '收入',
            '经营 / 销售'
        ]);
        expect(getBillMatchingCandidateBillAmountCents(candidate)).toBe(8850);
    });

    test('normalizes snake-case bill fields and rounds yuan display amounts to cents', () => {
        const response = normalizeBillMatchingCandidatesResponse({
            billId: 101,
            linkedPair: null,
            candidates: [{
                candidateId: 'bill:101:transfer:202',
                kind: 'transfer',
                score: '0.82',
                level: 'medium',
                reason: 'manual_pair',
                billId: '202',
                bill: {
                    id: '202',
                    type: 4,
                    amount: '88.505',
                    date: '2026-05-21',
                    description: '内部转账',
                    counterparty: '银行卡',
                    payment_method: '网银',
                    main_category: '转账',
                    sub_category: '内部',
                    source_account_id: '12',
                    destination_account_id: '34'
                }
            }]
        });
        const candidate = response.candidates[0]!;

        expect(candidate).toMatchObject({
            candidateId: 'bill:101:transfer:202',
            kind: 'transfer',
            score: 0.82,
            billId: 202
        });
        expect(candidate.bill).toMatchObject({
            id: 202,
            type: '4',
            amount: 88.505,
            paymentMethod: '网银',
            mainCategory: '转账',
            subCategory: '内部',
            sourceAccountId: 12,
            destinationAccountId: 34
        });
        expect(getBillMatchingCandidateBillAmountCents(candidate)).toBe(8851);
    });

    test('keeps historical duplicate candidates actionable with persisted reconciliation actions', () => {
        const response = normalizeBillMatchingCandidatesResponse({
            billId: 101,
            linkedPair: null,
            candidates: [
                {
                    candidateId: 'bill:101:duplicate:202',
                    kind: 'duplicate',
                    billId: 202,
                    score: 1,
                    level: 'high',
                    reason: 'same_bill_fields'
                },
                {
                    candidateId: 'reconcile:import:duplicate:bill:101:preview-3',
                    kind: 'reconciliation_duplicate',
                    score: 0.98,
                    level: 'high',
                    reason: 'same_date_amount_counterparty'
                }
            ]
        });

        expect(isBillMatchingCandidateReviewActionSupported(response.candidates[0]!)).toBe(true);
        expect(isBillMatchingCandidateReviewActionSupported(response.candidates[1]!)).toBe(true);

        const state = buildBillMatchingViewState(response);
        expect(state.mode).toBe('candidates');
        expect(state.showCandidateActions).toBe(true);
    });
});
