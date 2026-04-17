import { describe, expect, test } from '@jest/globals';

import {
    buildImportCheckLearningPreviewTextSyncPayload,
    convertImportPreviewAmountToCents,
    hasImportCheckLearningExpectedStateDrift,
    hasImportCheckLearningTextDrift,
    type ImportCheckLearningDecisionBaseline
} from '@/views/desktop/transactions/import/checkDataLearning.ts';

describe('checkDataLearning helpers', () => {
    test('returns null when preview id is missing or invalid', () => {
        expect(buildImportCheckLearningPreviewTextSyncPayload({})).toBeNull();
        expect(buildImportCheckLearningPreviewTextSyncPayload({ previewId: 0 })).toBeNull();
        expect(buildImportCheckLearningPreviewTextSyncPayload({ previewId: -1 })).toBeNull();
    });

    test('builds preview text sync payload for learning decision auto-sync', () => {
        expect(buildImportCheckLearningPreviewTextSyncPayload({
            previewId: 42,
            counterparty: '兰州拉面',
            paymentMethod: '微信支付',
            comment: '工作日午餐',
            selected: false
        })).toStrictEqual({
            id: 42,
            counterparty: '兰州拉面',
            paymentMethod: '微信支付',
            description: '工作日午餐',
            isSelected: false
        });
    });

    test('distinguishes text drift from expected-state drift', () => {
        const baseline: ImportCheckLearningDecisionBaseline = {
            inputFingerprint: 'fingerprint-1',
            type: 3,
            categoryId: 'cat-1',
            recurringTemplateId: 'rec-1',
            sourceAccountId: 'src-1',
            destinationAccountId: 'dst-1'
        };

        expect(hasImportCheckLearningTextDrift(baseline, {
            ...baseline,
            inputFingerprint: 'fingerprint-2'
        })).toBe(true);
        expect(hasImportCheckLearningExpectedStateDrift(baseline, {
            ...baseline,
            inputFingerprint: 'fingerprint-2'
        })).toBe(false);

        expect(hasImportCheckLearningTextDrift(baseline, {
            ...baseline,
            categoryId: 'cat-2'
        })).toBe(false);
        expect(hasImportCheckLearningExpectedStateDrift(baseline, {
            ...baseline,
            categoryId: 'cat-2'
        })).toBe(true);
    });

    test('converts preview amounts from yuan to cents with fallback preservation', () => {
        expect(convertImportPreviewAmountToCents(12.34, 999)).toBe(1234);
        expect(convertImportPreviewAmountToCents(-8.01, 999)).toBe(801);
        expect(convertImportPreviewAmountToCents(undefined, 999)).toBe(999);
    });
});