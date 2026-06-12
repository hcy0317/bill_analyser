import { describe, expect, test } from '@jest/globals';

import { normalizeSuggestionsResponse } from '@/models/recurring_suggestion.ts';

describe('recurring suggestion model helpers', () => {
    test('normalizes explicit integer cents from recurring suggestions', () => {
        const response = normalizeSuggestionsResponse({
            total: '1',
            items: [{
                id: '7',
                patternHash: 'hash',
                name: '房租',
                type: 'expense',
                amount_cents: '123456',
                sampleBillIds: ['1', 2],
                status: ''
            }]
        });

        expect(response.total).toBe(1);
        expect(response.items[0]).toMatchObject({
            id: 7,
            amountCents: 123456,
            sampleBillIds: [1, 2],
            status: 'pending'
        });
    });

    test.each([
        ['decimal number', 12.34],
        ['decimal string', '12.34'],
        ['boolean', true],
        ['object', {}]
    ])('rejects non-integer recurring suggestion cents: %s', (_name, value) => {
        const response = normalizeSuggestionsResponse({
            items: [{
                id: 1,
                amountCents: value
            }]
        });

        expect(response.items[0]!.amountCents).toBe(0);
    });
});
