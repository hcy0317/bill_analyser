import { describe, expect, test } from '@jest/globals';

import { cloneImportPreviewDraftTransaction } from '@/views/desktop/transactions/import/importPreviewDrafts.ts';

describe('import preview draft cloning', () => {
    test('clones nested preview draft payloads deeply', () => {
        const transaction = {
            tagIds: ['1'],
            originalTagNames: ['餐饮'],
            parserTags: ['parser:alipay'],
            dedupSourceIds: [101, '102'],
            matching: {
                transfer: {
                    candidate_type: 'transfer',
                    score: 0.98,
                    level: 'high',
                    reason: 'matched',
                    review_status: 'accepted',
                    reviewed_type: 'transfer',
                    suppressed: false
                },
                investment: {
                    score: 0,
                    level: '',
                    reason: '',
                    platform: '',
                    product: '',
                    review_status: '',
                    suppressed: false
                },
                learning: {
                    rule_id: 7,
                    score: 0.88,
                    level: 'medium',
                    reason: 'history',
                    recommended_type: '支出',
                    summary: '推荐：咖啡',
                    review_status: 'pending',
                    suppressed: false
                },
                recurring: {
                    id: 5,
                    name: '房租',
                    candidate_count: 2,
                    match_score: 0.91,
                    match_reasons: 'same amount',
                    matched_date: '2026-04-29'
                },
                dedup: {
                    type: 'transfer',
                    source_ids: [11, 12],
                    source_count: 2,
                    source_labels: ['支付宝', '民生银行']
                },
                parser: {
                    id: 'alipay',
                    tags: ['parser:alipay'],
                    source_chain: [{ position: 1, parser_id: 'alipay', label: '支付宝' }]
                },
                annotation: {
                    is_manually_annotated: false
                }
            },
            geoLocation: {
                latitude: 31.23,
                longitude: 121.47
            },
            _previewId: 42,
            _shouldClearTransferDecision: true,
            _previewDecisionBaseline: {
                type: 4,
                categoryId: '10',
                recurringTemplateId: '5',
                recurringTemplateName: '房租',
                recurringCandidateCount: 2,
                recurringMatchScore: 0.91,
                recurringMatchReasons: 'same amount',
                recurringMatchedDate: '2026-04-29',
                reviewStatus: 'accepted',
                reviewedType: 'transfer',
                suppressed: false
            },
            _learningDecisionBaseline: {
                inputFingerprint: '{"parserSource":"alipay"}',
                type: 3,
                categoryId: '10',
                recurringTemplateId: '',
                sourceAccountId: '100',
                destinationAccountId: ''
            }
        };

        expect(() => cloneImportPreviewDraftTransaction(transaction)).not.toThrow();

        const cloned = cloneImportPreviewDraftTransaction(transaction);

        expect(cloned).not.toBe(transaction);
        expect(cloned.matching).not.toBe(transaction.matching);
        expect(cloned.geoLocation).not.toBe(transaction.geoLocation);
        expect(cloned._previewDecisionBaseline).not.toBe(transaction._previewDecisionBaseline);
        expect(cloned._learningDecisionBaseline).not.toBe(transaction._learningDecisionBaseline);

        cloned.matching.transfer.review_status = 'rejected';
        expect(transaction.matching.transfer.review_status).toBe('accepted');
    });
});
