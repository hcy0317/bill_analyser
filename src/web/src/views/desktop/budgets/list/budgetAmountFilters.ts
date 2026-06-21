import { AmountFilterType } from '@/core/numeral.ts';
import type { Budget } from '@/models/budget.ts';

export interface AmountFilterCents {
    type: string;
    value1Cents: number;
    value2Cents?: number;
}

/**
 * 判断预算有效期是否与当前筛选范围有交集。
 */
export function isDateInRange(budget: Budget, startDate: Date, endDate: Date): boolean {
    if (!budget.startDate || !budget.endDate) return true;
    const budgetStart = new Date(budget.startDate);
    const budgetEnd = new Date(budget.endDate);
    return budgetStart <= endDate && budgetEnd >= startDate;
}

/**
 * 解析金额筛选字符串，filterType 后的金额值均为整数分。
 */
export function parseAmountFilterCents(filter: string): AmountFilterCents | null {
    if (!filter) return null;
    const parts = filter.split(':');
    if (parts.length < 2) return null;

    const type = parts[0] || '';
    const value1Str = parts[1] || '';
    const value1Cents = parseInt(value1Str, 10);

    if (!Number.isInteger(value1Cents)) return null;

    if (parts.length >= 3) {
        const value2Str = parts[2] || '';
        const value2Cents = parseInt(value2Str, 10);
        if (!Number.isInteger(value2Cents)) return null;
        return { type, value1Cents, value2Cents };
    }

    return { type, value1Cents };
}

/**
 * 按整数分匹配预算金额筛选条件。
 */
export function matchAmountFilterCents(amountCents: number, filter: AmountFilterCents): boolean {
    switch (filter.type) {
        case 'gt':
            return amountCents > filter.value1Cents;
        case 'lt':
            return amountCents < filter.value1Cents;
        case 'eq':
            return amountCents === filter.value1Cents;
        case 'ne':
            return amountCents !== filter.value1Cents;
        case 'bt':
            return filter.value2Cents !== undefined && amountCents >= filter.value1Cents && amountCents <= filter.value2Cents;
        case 'nb':
            return filter.value2Cents !== undefined && (amountCents < filter.value1Cents || amountCents > filter.value2Cents);
        default:
            return true;
    }
}

export function getAmountFilterParameterCount(filterType: string): number {
    const filterTypeObj = AmountFilterType.valueOf(filterType);
    return filterTypeObj ? filterTypeObj.paramCount : 0;
}
