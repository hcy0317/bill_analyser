import { describe, expect, test } from '@jest/globals';

import { buildTransactionListQuery } from '@/lib/services/transaction.ts';

describe('transaction service adapters', () => {
    test('omits zero date bounds for the All transaction list filter', () => {
        const query = buildTransactionListQuery({
            maxTime: 0,
            minTime: 0,
            count: 20,
            page: 1,
            withCount: true,
            type: 0,
            categoryIds: '',
            accountIds: '',
            tagIds: '',
            tagFilterType: 0,
            amountFilterCents: '',
            keyword: ''
        });

        expect(query).not.toContain('max_time=0');
        expect(query).not.toContain('min_time=0');
        expect(query).toContain('type=0');
        expect(query).toContain('page_size=20');
        expect(query).toContain('with_count=true');
        expect(query).toBe('type=0&categoryIds=&accountIds=&tagIds=&tagFilterType=0&amountFilterCents=&keyword=&page_size=20&page=1&with_count=true');
    });

    test('keeps positive date bounds and encodes textual filters', () => {
        const query = buildTransactionListQuery({
            maxTime: 1778284800999,
            minTime: 1778198400000,
            count: 50,
            page: 2,
            withCount: false,
            type: 2,
            categoryIds: 'cat/1',
            accountIds: 'acc 1',
            tagIds: 'tag-a,tag-b',
            tagFilterType: 1,
            amountFilterCents: 'gte:100',
            keyword: 'coffee shop'
        });

        expect(query).toBe(
            'max_time=1778284800999&min_time=1778198400000&type=2&categoryIds=cat%2F1&accountIds=acc%201&tagIds=tag-a%2Ctag-b&tagFilterType=1&amountFilterCents=gte%3A100&keyword=coffee%20shop&page_size=50&page=2&with_count=false'
        );
    });
});
