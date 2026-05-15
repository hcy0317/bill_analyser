import { describe, expect, test } from '@jest/globals';

import {
    formatAmount,
    getPeriodColor,
    getPeriodName,
    getStatusColor,
    getUsagePercent,
    getUsedAmountClass,
    normalizeEnabled,
    normalizeExecutionBudget,
    summarizeBudgetStatuses
} from '@/components/budgetManagementDialogHelpers.ts';

describe('budget management dialog helpers', () => {
    test('formats labels, colors, amounts, and usage display', () => {
        expect(formatAmount(1234.5)).toContain('1,234.50');
        expect(getPeriodName('monthly')).toBe('每月');
        expect(getPeriodName('unknown')).toBe('unknown');
        expect(getPeriodColor('weekly')).toBe('green');
        expect(getPeriodColor('unknown')).toBe('grey');
        expect(getStatusColor('critical')).toBe('error');
        expect(getStatusColor()).toBe('grey');
        expect(getUsagePercent({ name: '预算', period_type: 'monthly', amount: 200, start_date: '2026-01-01', enabled: true, used: 51 })).toBe(26);
        expect(getUsedAmountClass({ name: '预算', period_type: 'monthly', amount: 100, start_date: '2026-01-01', enabled: true, status: 'warning' })).toContain('text-warning');
    });

    test('normalizes execution budget payloads and enabled flags', () => {
        const budget = normalizeExecutionBudget({
            id: 7,
            name: '餐饮预算',
            category: '餐饮',
            sub_category: '早餐',
            period_type: 'monthly',
            budget_amount: 100,
            spent_amount: 95,
            alert_threshold: 80,
            critical_threshold: 90,
            enabled: '0',
            start_date: '2026-01-01'
        });

        expect(budget).toMatchObject({
            id: 7,
            name: '餐饮预算',
            category: '餐饮/早餐',
            amount: 100,
            used: 95,
            enabled: false,
            status: 'critical',
            usage_ratio: 0.95
        });
        expect(normalizeEnabled(false)).toBe(false);
        expect(normalizeEnabled(0)).toBe(false);
        expect(normalizeEnabled('1')).toBe(true);
    });

    test('summarizes budget status buckets', () => {
        expect(summarizeBudgetStatuses([
            { name: '正常', period_type: 'monthly', amount: 100, start_date: '2026-01-01', enabled: true, status: 'normal' },
            { name: '预警', period_type: 'monthly', amount: 100, start_date: '2026-01-01', enabled: true, status: 'warning' },
            { name: '临界', period_type: 'monthly', amount: 100, start_date: '2026-01-01', enabled: true, status: 'critical' },
            { name: '超支', period_type: 'monthly', amount: 100, start_date: '2026-01-01', enabled: true, status: 'exceeded' },
            { name: '未知', period_type: 'monthly', amount: 100, start_date: '2026-01-01', enabled: true, status: 'other' }
        ])).toStrictEqual({
            normal: 2,
            warning: 1,
            critical: 1,
            exceeded: 1
        });
    });
});
