import {
    BudgetPeriodType,
    type BudgetHistoryItem
} from '@/models/budget.ts';

export type HistoricalAggregationType = BudgetPeriodType | 'fiscal_year';

export type HistoricalBudgetLevelMode = 'primary' | 'secondary';

export interface HistoricalBudgetCategoryRow {
    key: string;
    primaryCategory: string;
    secondaryCategory: string;
    displayCategory: string;
    isPrimaryRow: boolean;
    budgetAmount: number;
    spentAmount: number;
    remainingAmount: number;
    executionRate: number;
    color: string;
    primaryColor: string;
    primaryIcon: string | undefined;
    primaryIconColor: string | undefined;
    secondaryIcon: string | undefined;
    secondaryIconColor: string | undefined;
    childRows: HistoricalBudgetCategoryRow[];
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
    rows: HistoricalBudgetCategoryRow[];
}

export interface HistoricalBudgetCategoryMeta {
    color?: string;
    icon?: string;
    iconColor?: string;
    displayOrder?: number;
    subCategories?: Record<string, HistoricalBudgetCategoryMeta>;
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

interface AggregatedSecondary {
    primaryCategory: string;
    secondaryCategory: string;
    budgetAmount: number;
    spentAmount: number;
    displayOrder: number;
}

interface AggregatedPrimary {
    primaryCategory: string;
    primaryBudgetAmount: number;
    primarySpentAmount: number;
    aggregatedSecondaryBudget: number;
    aggregatedSecondarySpent: number;
    displayOrder: number;
    secondaries: Map<string, AggregatedSecondary>;
}

interface PeriodAccumulator {
    key: string;
    label: string;
    startDate: string;
    endDate: string;
    primaries: Map<string, AggregatedPrimary>;
}

function getColor(value: string | undefined): string | undefined {
    if (!value) {
        return undefined;
    }
    return value.startsWith('#') ? value : `#${value}`;
}

export function buildHistoricalBudgetPeriodGroups({
    items,
    aggregationType,
    fiscalYearStartMonth,
    fiscalYearStartDay,
    uncategorizedLabel,
    levelMode,
    legendSelection,
    categoryMeta,
    fallbackPalette
}: {
    items: BudgetHistoryItem[];
    aggregationType: HistoricalAggregationType;
    fiscalYearStartMonth: number;
    fiscalYearStartDay: number;
    uncategorizedLabel: string;
    levelMode: HistoricalBudgetLevelMode;
    legendSelection: Record<string, boolean>;
    categoryMeta: Record<string, HistoricalBudgetCategoryMeta>;
    fallbackPalette: string[];
}): HistoricalBudgetPeriodGroup[] {
    const accumulator = new Map<string, PeriodAccumulator>();

    for (const item of items) {
        const itemDate = parseDateOnly(item.periodStart);
        if (!itemDate) {
            continue;
        }

        const period = resolveHistoricalPeriodByDate(
            aggregationType,
            itemDate,
            fiscalYearStartMonth,
            fiscalYearStartDay
        );

        if (!accumulator.has(period.key)) {
            accumulator.set(period.key, {
                key: period.key,
                label: period.label,
                startDate: period.startDate,
                endDate: period.endDate,
                primaries: new Map()
            });
        }

        const periodEntry = accumulator.get(period.key)!;
        const primaryCategory = String(item.category || '').trim() || uncategorizedLabel;
        const secondaryCategory = String(item.subCategory || '').trim();
        const budgetAmount = normalizeAmount(item.budgetAmount);
        const spentAmount = normalizeAmount(item.spentAmount);
        const primaryMeta = categoryMeta[primaryCategory];
        const primaryDisplayOrder = primaryMeta?.displayOrder ?? Number.MAX_SAFE_INTEGER;

        if (!periodEntry.primaries.has(primaryCategory)) {
            periodEntry.primaries.set(primaryCategory, {
                primaryCategory,
                primaryBudgetAmount: 0,
                primarySpentAmount: 0,
                aggregatedSecondaryBudget: 0,
                aggregatedSecondarySpent: 0,
                displayOrder: primaryDisplayOrder,
                secondaries: new Map()
            });
        }

        const primaryEntry = periodEntry.primaries.get(primaryCategory)!;

        if (!secondaryCategory) {
            primaryEntry.primaryBudgetAmount += budgetAmount;
            primaryEntry.primarySpentAmount += spentAmount;
            continue;
        }

        const secondaryDisplayOrder = primaryMeta?.subCategories?.[secondaryCategory]?.displayOrder ?? Number.MAX_SAFE_INTEGER;

        if (!primaryEntry.secondaries.has(secondaryCategory)) {
            primaryEntry.secondaries.set(secondaryCategory, {
                primaryCategory,
                secondaryCategory,
                budgetAmount: 0,
                spentAmount: 0,
                displayOrder: secondaryDisplayOrder
            });
        }

        const secondaryEntry = primaryEntry.secondaries.get(secondaryCategory)!;
        secondaryEntry.budgetAmount += budgetAmount;
        secondaryEntry.spentAmount += spentAmount;
        primaryEntry.aggregatedSecondaryBudget += budgetAmount;
        primaryEntry.aggregatedSecondarySpent += spentAmount;
    }

    const isLegendSelected = (key: string): boolean => {
        return legendSelection[key] !== false;
    };

    const sortByOrder = <T extends { displayOrder: number }>(left: T, right: T): number => {
        if (left.displayOrder !== right.displayOrder) {
            return left.displayOrder - right.displayOrder;
        }
        return 0;
    };

    let fallbackIdx = 0;
    const pickFallbackColor = (): string => {
        const color = fallbackPalette[fallbackIdx % fallbackPalette.length] ?? '#5470c6';
        fallbackIdx += 1;
        return color;
    };

    const groups: HistoricalBudgetPeriodGroup[] = [];

    for (const periodEntry of accumulator.values()) {
        const sortedPrimaries = Array.from(periodEntry.primaries.values()).sort((a, b) => {
            const order = sortByOrder(a, b);
            if (order !== 0) {
                return order;
            }
            return a.primaryCategory.localeCompare(b.primaryCategory, 'zh-CN');
        });

        const rows: HistoricalBudgetGroupRowAccumulator[] = [];

        for (const primary of sortedPrimaries) {
            const primaryMeta = categoryMeta[primary.primaryCategory];
            const primaryColor = getColor(primaryMeta?.color) ?? pickFallbackColor();
            const primaryIcon = primaryMeta?.icon;
            const primaryIconColor = primaryMeta?.iconColor;

            if (levelMode === 'primary') {
                const secondaryKeys = Array.from(primary.secondaries.keys()).map(secondary => `${primary.primaryCategory}::${secondary}`);
                const fallbackKey = `${primary.primaryCategory}::${primary.primaryCategory}`;
                const candidateKeys = secondaryKeys.length > 0 ? secondaryKeys : [fallbackKey];
                const isVisible = candidateKeys.some(isLegendSelected);

                if (!isVisible) {
                    continue;
                }

                const hasPrimaryBudget = primary.primaryBudgetAmount > 0 || primary.primarySpentAmount > 0;
                const budgetAmount = hasPrimaryBudget ? primary.primaryBudgetAmount : primary.aggregatedSecondaryBudget;
                const spentAmount = hasPrimaryBudget
                    ? (primary.primarySpentAmount > 0 ? primary.primarySpentAmount : primary.aggregatedSecondarySpent)
                    : primary.aggregatedSecondarySpent;

                if (budgetAmount <= 0 && spentAmount <= 0) {
                    continue;
                }

                rows.push({
                    key: `primary::${primary.primaryCategory}`,
                    primaryCategory: primary.primaryCategory,
                    secondaryCategory: '',
                    displayCategory: primary.primaryCategory,
                    isPrimaryRow: true,
                    budgetAmount,
                    spentAmount,
                    color: primaryColor,
                    primaryColor,
                    primaryIcon,
                    primaryIconColor,
                    secondaryIcon: undefined,
                    secondaryIconColor: undefined,
                    childRows: []
                });
                continue;
            }

            const sortedSecondaries = Array.from(primary.secondaries.values()).sort((a, b) => {
                const order = sortByOrder(a, b);
                if (order !== 0) {
                    return order;
                }
                return a.secondaryCategory.localeCompare(b.secondaryCategory, 'zh-CN');
            });

            const visibleSecondaries = sortedSecondaries.filter(secondary => {
                if (secondary.budgetAmount <= 0 && secondary.spentAmount <= 0) {
                    return false;
                }
                return isLegendSelected(`${primary.primaryCategory}::${secondary.secondaryCategory}`);
            });

            if (visibleSecondaries.length === 0) {
                const fallbackKey = `${primary.primaryCategory}::${primary.primaryCategory}`;
                const fallbackVisible = isLegendSelected(fallbackKey);
                if (!fallbackVisible || (primary.primaryBudgetAmount <= 0 && primary.primarySpentAmount <= 0)) {
                    continue;
                }

                rows.push({
                    key: `primary::${primary.primaryCategory}`,
                    primaryCategory: primary.primaryCategory,
                    secondaryCategory: '',
                    displayCategory: primary.primaryCategory,
                    isPrimaryRow: true,
                    budgetAmount: primary.primaryBudgetAmount,
                    spentAmount: primary.primarySpentAmount,
                    color: primaryColor,
                    primaryColor,
                    primaryIcon,
                    primaryIconColor,
                    secondaryIcon: undefined,
                    secondaryIconColor: undefined,
                    childRows: []
                });
                continue;
            }

            const childRows: HistoricalBudgetGroupRowAccumulator[] = [];
            let childBudgetTotal = 0;
            let childSpentTotal = 0;
            for (const secondary of visibleSecondaries) {
                const subMeta = primaryMeta?.subCategories?.[secondary.secondaryCategory];
                const subColor = getColor(subMeta?.color) ?? primaryColor;
                childBudgetTotal += secondary.budgetAmount;
                childSpentTotal += secondary.spentAmount;
                childRows.push({
                    key: `secondary::${primary.primaryCategory}::${secondary.secondaryCategory}`,
                    primaryCategory: primary.primaryCategory,
                    secondaryCategory: secondary.secondaryCategory,
                    displayCategory: secondary.secondaryCategory,
                    isPrimaryRow: false,
                    budgetAmount: secondary.budgetAmount,
                    spentAmount: secondary.spentAmount,
                    color: subColor,
                    primaryColor,
                    primaryIcon,
                    primaryIconColor,
                    secondaryIcon: subMeta?.icon,
                    secondaryIconColor: subMeta?.iconColor,
                    childRows: []
                });
            }

            const primaryRowBudget = primary.primaryBudgetAmount > 0
                ? primary.primaryBudgetAmount
                : childBudgetTotal;
            const primaryRowSpent = primary.primarySpentAmount > 0
                ? primary.primarySpentAmount
                : childSpentTotal;

            rows.push({
                key: `primary::${primary.primaryCategory}`,
                primaryCategory: primary.primaryCategory,
                secondaryCategory: '',
                displayCategory: primary.primaryCategory,
                isPrimaryRow: true,
                budgetAmount: primaryRowBudget,
                spentAmount: primaryRowSpent,
                color: primaryColor,
                primaryColor,
                primaryIcon,
                primaryIconColor,
                secondaryIcon: undefined,
                secondaryIconColor: undefined,
                childRows
            });
        }

        if (rows.length === 0) {
            continue;
        }

        const totalBudget = rows.reduce((sum, row) => sum + row.budgetAmount, 0);
        const totalSpent = rows.reduce((sum, row) => sum + row.spentAmount, 0);
        const itemCount = rows.reduce((sum, row) => sum + 1 + row.childRows.length, 0);
        const finalRows: HistoricalBudgetCategoryRow[] = rows.map(row => ({
            key: row.key,
            primaryCategory: row.primaryCategory,
            secondaryCategory: row.secondaryCategory,
            displayCategory: row.displayCategory,
            isPrimaryRow: row.isPrimaryRow,
            budgetAmount: row.budgetAmount,
            spentAmount: row.spentAmount,
            color: row.color,
            primaryColor: row.primaryColor,
            primaryIcon: row.primaryIcon,
            primaryIconColor: row.primaryIconColor,
            secondaryIcon: row.secondaryIcon,
            secondaryIconColor: row.secondaryIconColor,
            remainingAmount: Math.max(0, row.budgetAmount - row.spentAmount),
            executionRate: row.budgetAmount > 0 ? Number(((row.spentAmount / row.budgetAmount) * 100).toFixed(1)) : 0,
            childRows: row.childRows.map((child): HistoricalBudgetCategoryRow => ({
                key: child.key,
                primaryCategory: child.primaryCategory,
                secondaryCategory: child.secondaryCategory,
                displayCategory: child.displayCategory,
                isPrimaryRow: child.isPrimaryRow,
                budgetAmount: child.budgetAmount,
                spentAmount: child.spentAmount,
                color: child.color,
                primaryColor: child.primaryColor,
                primaryIcon: child.primaryIcon,
                primaryIconColor: child.primaryIconColor,
                secondaryIcon: child.secondaryIcon,
                secondaryIconColor: child.secondaryIconColor,
                remainingAmount: Math.max(0, child.budgetAmount - child.spentAmount),
                executionRate: child.budgetAmount > 0 ? Number(((child.spentAmount / child.budgetAmount) * 100).toFixed(1)) : 0,
                childRows: []
            }))
        }));

        groups.push({
            key: periodEntry.key,
            label: periodEntry.label,
            startDate: periodEntry.startDate,
            endDate: periodEntry.endDate,
            totalBudget,
            totalSpent,
            totalExecutionRate: totalBudget > 0
                ? Number(((totalSpent / totalBudget) * 100).toFixed(1))
                : 0,
            itemCount,
            rows: finalRows
        });
    }

    return groups.sort((left, right) => right.startDate.localeCompare(left.startDate, 'zh-CN'));
}

interface HistoricalBudgetGroupRowAccumulator {
    key: string;
    primaryCategory: string;
    secondaryCategory: string;
    displayCategory: string;
    isPrimaryRow: boolean;
    budgetAmount: number;
    spentAmount: number;
    color: string;
    primaryColor: string;
    primaryIcon: string | undefined;
    primaryIconColor: string | undefined;
    secondaryIcon: string | undefined;
    secondaryIconColor: string | undefined;
    childRows: HistoricalBudgetGroupRowAccumulator[];
}
