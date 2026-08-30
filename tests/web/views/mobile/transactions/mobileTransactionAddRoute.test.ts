import { describe, expect, test } from '@jest/globals';

import { TransactionType } from '@/core/transaction.ts';
import { buildMobileTransactionAddPath } from '@/views/mobile/transactions/list-page/addRoute.ts';

describe('mobile transaction add route', () => {
    test('projects the current filters and clamps a past date range', () => {
        expect(buildMobileTransactionAddPath({
            maxTime: 1_000,
            minTime: 500,
            type: TransactionType.Expense,
            categoryIds: 'category-1',
            accountIds: 'account-1',
            tagIds: 'tag-1,tag-2'
        }, 1, 1, 2_000)).toBe(
            '/transaction/add?time=1000&type=3&categoryId=category-1&accountId=account-1&tagIds=tag-1,tag-2'
        );
    });

    test('clamps a future range and omits ambiguous or unsupported defaults', () => {
        expect(buildMobileTransactionAddPath({
            maxTime: 3_000,
            minTime: 2_500,
            type: TransactionType.ModifyBalance,
            categoryIds: 'category-1,category-2',
            accountIds: 'account-1,account-2'
        }, 2, 2, 2_000)).toBe('/transaction/add?time=2500');
    });

    test('keeps the current time when it is already inside the filter range', () => {
        expect(buildMobileTransactionAddPath({
            maxTime: 3_000,
            minTime: 1_000,
            type: TransactionType.Income
        }, 0, 0, 2_000)).toBe('/transaction/add?type=2');
    });
});
