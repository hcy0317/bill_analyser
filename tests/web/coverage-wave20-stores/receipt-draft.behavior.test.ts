import { describe, expect, test } from '@jest/globals';

import {
    buildRecognizeReceiptImageError,
    mapReceiptImageErrorCode,
    normalizeReceiptTransactionDraft
} from '@/stores/transaction/receiptDraft.ts';

function field(value: unknown, overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        value,
        confidence: 0.8,
        reason: 'receipt evidence',
        evidence: ['line one'],
        ...overrides
    };
}

describe('receipt draft normalization behavior', () => {
    test.each([
        ['timeout', 500, 'timeout'],
        ['provider_unconfigured', 500, 'provider_unconfigured'],
        ['unknown', 501, 'provider_unconfigured'],
        [undefined, 504, 'timeout'],
        ['not-a-code', 422, 'parse_error'],
        [undefined, 499, 'cancelled'],
        [undefined, 429, 'rate_limited'],
        [undefined, 500, 'unknown']
    ] as const)('maps raw code %p and HTTP %p to %p', (rawCode, status, expected) => {
        expect(mapReceiptImageErrorCode(rawCode, status)).toBe(expected);
    });

    test('builds a stable recognition error with and without an original exception', () => {
        const originalError = new Error('provider failed');
        expect(buildRecognizeReceiptImageError('timeout', 'Timed out', 504, originalError)).toEqual({
            errorCode: 'timeout',
            message: 'Timed out',
            status: 504,
            originalError
        });
        expect(buildRecognizeReceiptImageError('unknown', 'Failed', 500)).toEqual({
            errorCode: 'unknown',
            message: 'Failed',
            status: 500,
            originalError: undefined
        });
    });

    test.each([
        ['type', field(2)],
        ['amount', field(1250)],
        ['time', field('2026-07-16T10:00:00+08:00')],
        ['description', field('Lunch')],
        ['category_id', field('food')],
        ['source_account_id', field('cash')],
        ['destination_account_id', field('merchant')],
        ['tag_ids', field(['work', 'work', 7, '', null])]
    ] as const)('keeps a snake-case auto-fill draft whose only usable field is %s', (key, value) => {
        const result = normalizeReceiptTransactionDraft({ auto_fill: { [key]: value } });

        expect(result).toBeDefined();
        expect(Object.values(result!.autoFill).filter(Boolean)).toHaveLength(1);
        if (key === 'tag_ids') {
            expect(result!.autoFill.tagIds?.value).toEqual(['work', '7']);
        }
    });

    test.each([
        ['type', field('expense')],
        ['amount', field(1250)],
        ['time', field(1710000000)],
        ['description', field('Lunch')],
        ['categoryId', field('food')],
        ['sourceAccountId', field('cash')],
        ['destinationAccountId', field('merchant')],
        ['tagIds', field(['work'])]
    ] as const)('keeps a camel-case candidate draft whose only usable list is %s', (key, value) => {
        const result = normalizeReceiptTransactionDraft({ candidates: { [key]: [null, value] } });

        expect(result).toBeDefined();
        expect(Object.values(result!.candidates).filter(Boolean)).toHaveLength(1);
    });

    test('normalizes metadata, label and unit variants without accepting hostile values', () => {
        const result = normalizeReceiptTransactionDraft({
            autoFill: {
                type: field('expense', { confidence: Number.NaN, reason: 5, evidence: ['valid', 9] }),
                amount: field(12.5, { label: 'Amount', unit: 'CNY' }),
                time: field(1710000000, { unit: 'seconds' }),
                description: field(42),
                categoryId: field('food', { label: '', unit: '' }),
                sourceAccountId: field('cash', { confidence: Infinity }),
                destinationAccountId: field('merchant'),
                tagIds: field(['tag', 'tag', 9])
            },
            candidates: {
                type: [field(1), field(Number.NaN), field({})],
                amount: [field(0), field(Infinity), 'bad'],
                time: [field('now'), field('')],
                description: [field(123), field(null)],
                categoryId: [field('food')],
                sourceAccountId: [field('cash')],
                destinationAccountId: [field('merchant')],
                tagIds: [field(['tag', '', null]), field('not-an-array')]
            }
        });

        expect(result!.autoFill.type).toEqual(expect.objectContaining({
            value: 'expense', confidence: 0, reason: '', evidence: ['valid']
        }));
        expect(result!.autoFill.amount).toEqual(expect.objectContaining({ label: 'Amount', unit: 'CNY' }));
        expect(result!.autoFill.time).toEqual(expect.objectContaining({ value: '1710000000', unit: 'seconds' }));
        expect(result!.autoFill.description?.value).toBe('42');
        expect(result!.autoFill.tagIds?.value).toEqual(['tag', '9']);
        expect(result!.candidates.type?.map(item => item.value)).toEqual([1]);
        expect(result!.candidates.amount?.map(item => item.value)).toEqual([0]);
        expect(result!.candidates.tagIds?.[0]?.value).toEqual(['tag']);
    });

    test.each([
        null,
        undefined,
        [],
        'bad',
        {},
        { auto_fill: null, autoFill: null, candidates: null },
        { auto_fill: { amount: field(Number.NaN), tag_ids: field([]) } },
        { candidates: { amount: 'not-a-list', tag_ids: [field([])] } }
    ])('rejects a draft with no usable field: %p', raw => {
        expect(normalizeReceiptTransactionDraft(raw)).toBeUndefined();
    });
});
