import { describe, expect, test } from '@jest/globals';

import { cloneImportPreviewDraftTransaction } from '@/views/desktop/transactions/import/importPreviewDrafts.ts';

function withStructuredCloneMock<T>(mock: typeof structuredClone, run: () => T): T {
    const originalDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'structuredClone');
    Object.defineProperty(globalThis, 'structuredClone', {
        configurable: true,
        writable: true,
        value: mock
    });
    try {
        return run();
    } finally {
        if (originalDescriptor) {
            Object.defineProperty(globalThis, 'structuredClone', originalDescriptor);
        } else {
            Reflect.deleteProperty(globalThis, 'structuredClone');
        }
    }
}

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
                inputFingerprint: '{"parserId":"alipay"}',
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

    test('drops runtime-only values that structuredClone cannot copy', () => {
        const transaction = {
            matching: {
                transfer: {
                    review_status: 'accepted',
                    reason: 'matched'
                },
                learning: {
                    review_status: 'skipped',
                    reason: 'transfer preview is protected from learning type/category overrides'
                },
                runtime: {
                    windowRef: typeof window === 'undefined' ? undefined : window,
                    callback: () => 'not cloneable'
                }
            }
        };

        expect(() => cloneImportPreviewDraftTransaction(transaction)).not.toThrow();

        const cloned = cloneImportPreviewDraftTransaction(transaction);
        expect(cloned.matching.transfer.review_status).toBe('accepted');
        expect(cloned.matching.learning.reason).toBe(
            'transfer preview is protected from learning type/category overrides'
        );
        expect('callback' in cloned.matching.runtime).toBe(false);
        expect('windowRef' in cloned.matching.runtime).toBe(typeof window === 'undefined');
    });

    test('falls back for non-DOM data clone errors and keeps plain cyclic data', () => {
        const cyclicMatching: {
            self?: unknown;
            dates: Date[];
            keepUndefined?: undefined;
            dropSymbol?: symbol;
            callback?: () => string;
        } = {
            dates: [new Date('2026-05-18T00:00:00.000Z')],
            keepUndefined: undefined,
            dropSymbol: Symbol('runtime'),
            callback: () => 'not cloneable'
        };
        cyclicMatching.self = cyclicMatching;

        const cloned = withStructuredCloneMock(() => {
            throw { name: 'DataCloneError' };
        }, () => cloneImportPreviewDraftTransaction({
            matching: cyclicMatching,
            geoLocation: null
        }));

        const clonedMatching = cloned.matching;
        expect(clonedMatching).not.toBe(cyclicMatching);
        expect(clonedMatching.self).toBe(clonedMatching);
        expect(clonedMatching.dates[0]).toEqual(new Date('2026-05-18T00:00:00.000Z'));
        expect(clonedMatching.dates[0]).not.toBe(cyclicMatching.dates[0]);
        expect('keepUndefined' in clonedMatching).toBe(true);
        expect('dropSymbol' in clonedMatching).toBe(false);
        expect('callback' in clonedMatching).toBe(false);
        expect(cloned.geoLocation).toBeNull();
    });

    test('rethrows unexpected structuredClone failures', () => {
        const failure = new Error('unexpected clone failure');

        withStructuredCloneMock(() => {
            throw failure;
        }, () => {
            expect(() => cloneImportPreviewDraftTransaction({
                matching: { value: 'plain' }
            })).toThrow(failure);
        });
    });
});
