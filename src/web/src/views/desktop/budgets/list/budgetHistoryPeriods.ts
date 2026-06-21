import { BudgetPeriodType, type BudgetHistoryRequest } from '@/models/budget.ts';

export type HistoricalAggregationType = BudgetPeriodType | 'fiscal_year';

export interface HistoricalPeriodRange {
    key: string;
    label: string;
    startDate: string;
    endDate: string;
}

export const HISTORICAL_FISCAL_YEAR = 'fiscal_year' as const;

export function parseDateOnly(text: string): Date | null {
    if (!text) {
        return null;
    }

    const parsed = new Date(`${text}T00:00:00`);
    return Number.isNaN(parsed.getTime()) ? null : parsed;
}

export function normalizeHistoryAmountCents(value: number): number {
    const amount = Number(value);
    return Number.isFinite(amount) ? Math.max(0, amount) : 0;
}

export function addMonths(sourceDate: Date, months: number): Date {
    return new Date(sourceDate.getFullYear(), sourceDate.getMonth() + months, 1);
}

export function isValidHistoricalUnixTime(value: number): boolean {
    return Number.isFinite(value) && value > 0;
}

/**
 * 构建历史预算请求签名，用于避免不同筛选条件共享旧缓存。
 */
export function buildBudgetHistoryRequestSignature(req: BudgetHistoryRequest): string {
    return JSON.stringify({
        type: req.type ?? null,
        periodType: req.periodType ?? null,
        year: req.year ?? null,
        month: req.month ?? null,
        quarter: req.quarter ?? null,
        startDate: req.startDate ?? null,
        endDate: req.endDate ?? null,
        categoryId: req.categoryId ?? null,
        accountIds: req.accountIds ?? [],
        tagIds: req.tagIds ?? []
    });
}
