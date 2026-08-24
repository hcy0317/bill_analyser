import { describe, expect, test } from '@jest/globals';

import {
    buildAnomalyActionTarget,
    normalizeActionCenterAnomalyResponse,
    summarizeActionCenter
} from '@/views/base/action-center/actionCenterModel.ts';

describe('action center model', () => {
    test('normalizes typed anomaly items and rejects malformed identifiers and amounts', () => {
        const result = normalizeActionCenterAnomalyResponse({
            anomalies: [
                {
                    type: 'large_transaction',
                    severity: 'warning',
                    billId: 41,
                    date: '2026-08-12',
                    amountCents: 12_345,
                    category: '餐饮',
                    message: 'large'
                },
                {
                    type: 'duplicate_charge',
                    severity: 'info',
                    billIds: [42, '43', -1, 'bad'],
                    dates: ['2026-08-10', '2026-08-11'],
                    amountCents: '5000',
                    counterparty: 'Coffee',
                    message: 'duplicate'
                },
                {
                    type: 'category_spike',
                    severity: 'warning',
                    month: '2026-08',
                    currentAmountCents: 88_000,
                    category: '交通',
                    message: 'spike'
                }
            ],
            totalCount: 3,
            analyzedBills: 120,
            analyzedMonths: 6,
            startDate: '2026-03-01',
            endDate: '2026-08-12'
        });

        expect(result.items).toEqual([
            expect.objectContaining({
                key: 'large_transaction:41',
                type: 'large_transaction',
                billIds: [41],
                amountCents: 12_345,
                occurredOn: '2026-08-12'
            }),
            expect.objectContaining({
                key: 'duplicate_charge:42:43',
                type: 'duplicate_charge',
                billIds: [42, 43],
                amountCents: 5_000,
                occurredOn: '2026-08-11'
            }),
            expect.objectContaining({
                key: 'category_spike:2026-08:交通',
                type: 'category_spike',
                billIds: [],
                amountCents: 88_000,
                occurredOn: '2026-08'
            })
        ]);
        expect(result.totalCount).toBe(3);
        expect(result.analyzedBills).toBe(120);
    });

    test('builds platform-specific review targets without inventing unsupported bill filters', () => {
        const large = normalizeActionCenterAnomalyResponse({
            anomalies: [{
                type: 'large_transaction', billId: 41, date: '2026-08-12',
                amountCents: 12_345, description: 'Coffee'
            }]
        }).items[0]!;
        const duplicate = normalizeActionCenterAnomalyResponse({
            anomalies: [{
                type: 'duplicate_charge', billIds: [42, 43], dates: ['2026-08-10', '2026-08-11'],
                amountCents: 5_000, counterparty: 'Coffee'
            }]
        }).items[0]!;
        const spike = normalizeActionCenterAnomalyResponse({
            anomalies: [{
                type: 'category_spike', month: '2026-08', category: '交通', currentAmountCents: 88_000
            }]
        }).items[0]!;

        expect(buildAnomalyActionTarget(large, 'desktop')).toEqual({
            path: '/transaction/list',
            query: { keyword: 'Coffee' }
        });
        expect(buildAnomalyActionTarget(duplicate, 'mobile')).toBe('/transaction/list?keyword=Coffee');
        expect(buildAnomalyActionTarget(spike, 'desktop')).toEqual({
            path: '/statistics/transaction'
        });
    });

    test('summarizes actionable anomaly and recurring work', () => {
        expect(summarizeActionCenter({ anomalyCount: 4, recurringCount: 3 })).toEqual({
            total: 7,
            anomalyCount: 4,
            recurringCount: 3,
            hasWork: true
        });
        expect(summarizeActionCenter({ anomalyCount: -1, recurringCount: Number.NaN }).hasWork).toBe(false);
    });
});
