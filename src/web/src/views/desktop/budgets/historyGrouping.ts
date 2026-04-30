import {
    BudgetPeriodType,
    type BudgetHistoryItem
} from '@/models/budget.ts';

export type HistoricalAggregationType = BudgetPeriodType | 'fiscal_year';

export interface HistoricalBudgetPeriodGroupItem {
    id: string;
    budgetId: string;
    displayCategory: string;
    category: string;
    subCategory: string;
    budgetAmount: number;
    spentAmount: number;
    remainingAmount: number;
    executionRate: number;
    status: string;
    periodStart: string;
    periodEnd: string;
}

export interface HistoricalBudgetPeriodGroup {
    key: string;
    label: string;
    startDate: string;
    endDate: string;
    totalBudget: number;
    totalSpent: number;
    totalExecutionRate: number;
    itemCount: number;
    items: HistoricalBudgetPeriodGroupItem[];
}

function parseDateOnly(text: string): Date | null {
    if (!text) {
        return null;
    }

    const parsed = new Date(`${text}T00:00:00`);
    return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function formatDateOnly(date: Date): string {
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

function normalizeAmount(value: number): number {
    const amount = Number(value);
    return Number.isFinite(amount) ? Math.max(0, amount) : 0;
}

function buildFiscalYearPeriod(targetDate: Date, fiscalYearStartMonth: number, fiscalYearStartDay: number) {
    const currentYearFiscalStart = new Date(targetDate.getFullYear(), fiscalYearStartMonth - 1, fiscalYearStartDay);
    const fiscalStartYear = targetDate >= currentYearFiscalStart
        ? targetDate.getFullYear()
        : targetDate.getFullYear() - 1;
    const startDate = new Date(fiscalStartYear, fiscalYearStartMonth - 1, fiscalYearStartDay);
    const endDate = new Date(fiscalStartYear + 1, fiscalYearStartMonth - 1, fiscalYearStartDay - 1);
    const endYear = endDate.getFullYear();

    return {
        key: `FY${endYear}`,
        label: `FY${endYear}`,
        startDate: formatDateOnly(startDate),
        endDate: formatDateOnly(endDate)
    };
}

function resolveHistoricalPeriodByDate(
    aggregationType: HistoricalAggregationType,
    targetDate: Date,
    fiscalYearStartMonth: number,
    fiscalYearStartDay: number
) {
    if (aggregationType === BudgetPeriodType.Monthly) {
        const key = `${targetDate.getFullYear()}-${String(targetDate.getMonth() + 1).padStart(2, '0')}`;
        return {
            key,
            label: key,
            startDate: formatDateOnly(new Date(targetDate.getFullYear(), targetDate.getMonth(), 1)),
            endDate: formatDateOnly(new Date(targetDate.getFullYear(), targetDate.getMonth() + 1, 0))
        };
    }

    if (aggregationType === BudgetPeriodType.Quarterly) {
        const quarterStartMonth = Math.floor(targetDate.getMonth() / 3) * 3;
        const quarter = Math.floor(targetDate.getMonth() / 3) + 1;
        return {
            key: `${targetDate.getFullYear()}-Q${quarter}`,
            label: `${targetDate.getFullYear()}-Q${quarter}`,
            startDate: formatDateOnly(new Date(targetDate.getFullYear(), quarterStartMonth, 1)),
            endDate: formatDateOnly(new Date(targetDate.getFullYear(), quarterStartMonth + 3, 0))
        };
    }

    if (aggregationType === BudgetPeriodType.Yearly) {
        return {
            key: `${targetDate.getFullYear()}`,
            label: `${targetDate.getFullYear()}`,
            startDate: `${targetDate.getFullYear()}-01-01`,
            endDate: `${targetDate.getFullYear()}-12-31`
        };
    }

    return buildFiscalYearPeriod(targetDate, fiscalYearStartMonth, fiscalYearStartDay);
}

export function buildHistoricalBudgetPeriodGroups({
    items,
    aggregationType,
    fiscalYearStartMonth,
    fiscalYearStartDay,
    uncategorizedLabel
}: {
    items: BudgetHistoryItem[];
    aggregationType: HistoricalAggregationType;
    fiscalYearStartMonth: number;
    fiscalYearStartDay: number;
    uncategorizedLabel: string;
}): HistoricalBudgetPeriodGroup[] {
    const grouped = new Map<string, HistoricalBudgetPeriodGroup>();

    for (const item of items) {
        const itemDate = parseDateOnly(item.periodStart);
        if (!itemDate) {
            continue;
        }

        const resolvedPeriod = resolveHistoricalPeriodByDate(
            aggregationType,
            itemDate,
            fiscalYearStartMonth,
            fiscalYearStartDay
        );

        if (!grouped.has(resolvedPeriod.key)) {
            grouped.set(resolvedPeriod.key, {
                key: resolvedPeriod.key,
                label: resolvedPeriod.label,
                startDate: resolvedPeriod.startDate,
                endDate: resolvedPeriod.endDate,
                totalBudget: 0,
                totalSpent: 0,
                totalExecutionRate: 0,
                itemCount: 0,
                items: []
            });
        }

        const group = grouped.get(resolvedPeriod.key)!;
        const category = String(item.category || '').trim() || uncategorizedLabel;
        const subCategory = String(item.subCategory || '').trim();
        const budgetAmount = normalizeAmount(item.budgetAmount);
        const spentAmount = normalizeAmount(item.spentAmount);
        const remainingAmount = normalizeAmount(item.remainingAmount);

        group.totalBudget += budgetAmount;
        group.totalSpent += spentAmount;
        group.itemCount += 1;
        group.items.push({
            id: item.id,
            budgetId: item.budgetId,
            displayCategory: subCategory ? `${category} / ${subCategory}` : category,
            category,
            subCategory,
            budgetAmount,
            spentAmount,
            remainingAmount,
            executionRate: Number.isFinite(item.executionRate) ? item.executionRate : 0,
            status: item.status,
            periodStart: item.periodStart,
            periodEnd: item.periodEnd
        });
    }

    return Array.from(grouped.values())
        .map(group => ({
            ...group,
            totalExecutionRate: group.totalBudget > 0
                ? Number(((group.totalSpent / group.totalBudget) * 100).toFixed(1))
                : 0,
            items: [...group.items].sort((left, right) => {
                const categoryCompare = left.category.localeCompare(right.category, 'zh-CN');
                if (categoryCompare !== 0) {
                    return categoryCompare;
                }

                const subCategoryCompare = left.subCategory.localeCompare(right.subCategory, 'zh-CN');
                if (subCategoryCompare !== 0) {
                    return subCategoryCompare;
                }

                return left.displayCategory.localeCompare(right.displayCategory, 'zh-CN');
            })
        }))
        .sort((left, right) => right.startDate.localeCompare(left.startDate, 'zh-CN'));
}
