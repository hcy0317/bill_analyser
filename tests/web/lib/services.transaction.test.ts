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
            amountFilter: '',
            keyword: ''
        });

        expect(query).not.toContain('max_time=0');
        expect(query).not.toContain('min_time=0');
        expect(query).toContain('type=0');
        expect(query).toContain('page_size=20');
        expect(query).toContain('with_count=true');
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
            amountFilter: 'gte:100',
            keyword: 'coffee shop'
        });

        expect(query).toContain('max_time=1778284800999');
        expect(query).toContain('min_time=1778198400000');
        expect(query).toContain('categoryIds=cat%2F1');
        expect(query).toContain('accountIds=acc%201');
        expect(query).toContain('keyword=coffee%20shop');
    });
});
