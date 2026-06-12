import {
    DEFAULT_BUDGET_ALERT_THRESHOLD,
    DEFAULT_BUDGET_ENABLED,
    DEFAULT_BUDGET_FORECAST_HISTORY_PERIODS
} from '@/config/budget.ts';
import {
    BudgetForecastStrategy,
    BudgetPeriodType,
    BudgetType
} from '@/models/budget.ts';

interface BudgetExecutionQueryRequest {
    type?: number,
    periodType?: string,
    year?: number,
    month?: number,
    quarter?: number,
    startDate?: string,
    endDate?: string
}

interface BudgetListQueryRequest {
    type?: number,
    periodType?: string,
    enabled?: boolean,
    category?: string
}

interface BudgetHistoryQueryRequest extends BudgetExecutionQueryRequest {
    budgetId?: string,
    categoryId?: string,
    accountIds?: string[],
    tagIds?: string[]
}

interface BudgetForecastQueryRequest extends BudgetExecutionQueryRequest {
    monthsHistory?: number,
    forecastStrategy?: string
}

function toBudgetAmountCents(value: unknown): number {
    if (value === null || value === undefined || typeof value === 'boolean') {
        return 0;
    }

    if (typeof value === 'number') {
        return Number.isSafeInteger(value) ? value : 0;
    }

    if (typeof value === 'string') {
        const normalized = value.trim();
        if (!/^[+-]?\d+$/.test(normalized)) {
            return 0;
        }
        const parsed = Number(normalized);
        return Number.isSafeInteger(parsed) ? parsed : 0;
    }

    return 0;
}

function normalizeRestBudgetType(value: unknown): BudgetType | null {
    if (value === BudgetType.Expense || value === BudgetType.Investment) {
        return value;
    }

    const normalizedText = String(value ?? '').trim().toLowerCase();
    if (normalizedText === 'expense') {
        return BudgetType.Expense;
    }
    if (normalizedText === 'investment') {
        return BudgetType.Investment;
    }

    const numericValue = Number(value);
    if (numericValue === BudgetType.Expense || numericValue === 1) {
        return BudgetType.Expense;
    }
    if (numericValue === BudgetType.Investment) {
        return BudgetType.Investment;
    }

    return null;
}

export function mapRestBudgetToFrontend(item: any, fallbackType = BudgetType.Expense): any {
    const categoryInfo = item?.category_info || {};

    return {
        id: String(item?.id ?? ''),
        name: item?.name || '',
        category: item?.category || '',
        subCategory: item?.subCategory || item?.sub_category || '',
        categoryId: String(item?.categoryId || item?.category_id || categoryInfo?.id || ''),
        periodType: item?.periodType || item?.period_type || BudgetPeriodType.Monthly,
        amountCents: toBudgetAmountCents(item?.amountCents ?? item?.amount_cents ?? 0),
        startDate: item?.startDate || item?.start_date || '',
        endDate: item?.endDate || item?.end_date || '',
        alertThreshold: item?.alertThreshold ?? item?.alert_threshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD,
        enabled: item?.enabled ?? DEFAULT_BUDGET_ENABLED,
        type: item?.type ?? fallbackType,
        createdAt: item?.createdAt || item?.created_at || '',
        updatedAt: item?.updatedAt || item?.updated_at || '',
        spentAmountCents: toBudgetAmountCents(item?.spentAmountCents ?? item?.spent_amount_cents ?? 0),
        remainingAmountCents: toBudgetAmountCents(item?.remainingAmountCents ?? item?.remaining_amount_cents ?? 0),
        executionRate: item?.executionRate ?? item?.execution_rate ?? 0,
        categoryName: item?.categoryName || item?.category || '',
        categoryIcon: item?.categoryIcon || categoryInfo?.icon || '',
        categoryColor: item?.categoryColor || categoryInfo?.color || ''
    };
}

export function buildBudgetExecutionQuery(req?: BudgetExecutionQueryRequest): string {
    const queryParams: string[] = [];

    if (req?.type !== undefined) {
        queryParams.push(`budget_type=${req.type}`);
    }
    if (req?.periodType) {
        queryParams.push(`period_type=${req.periodType}`);
    }
    if (req?.year !== undefined) {
        queryParams.push(`year=${req.year}`);
    }
    if (req?.month !== undefined) {
        queryParams.push(`month=${req.month}`);
    }
    if (req?.quarter !== undefined) {
        queryParams.push(`quarter=${req.quarter}`);
    }
    if (req?.startDate) {
        queryParams.push(`start_date=${encodeURIComponent(req.startDate)}`);
    }
    if (req?.endDate) {
        queryParams.push(`end_date=${encodeURIComponent(req.endDate)}`);
    }

    return queryParams.length > 0 ? `?${queryParams.join('&')}` : '';
}

export function buildBudgetListQuery(req?: BudgetListQueryRequest): string {
    const queryParams: string[] = [];

    if (req?.type !== undefined) {
        queryParams.push(`budget_type=${req.type}`);
    }
    if (req?.periodType) {
        queryParams.push(`period_type=${req.periodType}`);
    }
    if (req?.enabled !== undefined) {
        queryParams.push(`enabled=${req.enabled ? 'true' : 'false'}`);
    }
    if (req?.category) {
        queryParams.push(`category=${encodeURIComponent(req.category)}`);
    }

    return queryParams.length > 0 ? `?${queryParams.join('&')}` : '';
}

export function buildBudgetHistoryQuery(req?: BudgetHistoryQueryRequest): string {
    const queryParams: string[] = [];

    if (req?.type !== undefined) {
        queryParams.push(`budget_type=${req.type}`);
    }
    if (req?.periodType) {
        queryParams.push(`period_type=${req.periodType}`);
    }
    if (req?.year !== undefined) {
        queryParams.push(`year=${req.year}`);
    }
    if (req?.month !== undefined) {
        queryParams.push(`month=${req.month}`);
    }
    if (req?.quarter !== undefined) {
        queryParams.push(`quarter=${req.quarter}`);
    }
    if (req?.startDate) {
        queryParams.push(`start_date=${encodeURIComponent(req.startDate)}`);
    }
    if (req?.endDate) {
        queryParams.push(`end_date=${encodeURIComponent(req.endDate)}`);
    }
    if (req?.budgetId) {
        queryParams.push(`budget_id=${encodeURIComponent(req.budgetId)}`);
    }
    if (req?.categoryId) {
        queryParams.push(`category_id=${encodeURIComponent(req.categoryId)}`);
    }
    if (req?.accountIds?.length) {
        queryParams.push(`account_ids=${encodeURIComponent(req.accountIds.join(','))}`);
    }
    if (req?.tagIds?.length) {
        queryParams.push(`tag_ids=${encodeURIComponent(req.tagIds.join(','))}`);
    }

    return queryParams.length > 0 ? `?${queryParams.join('&')}` : '';
}

export function buildBudgetForecastQuery(req?: BudgetForecastQueryRequest): string {
    const queryParams: string[] = [];

    if (req?.type !== undefined) {
        queryParams.push(`budget_type=${req.type}`);
    }
    if (req?.periodType) {
        queryParams.push(`period_type=${req.periodType}`);
    }
    if (req?.year !== undefined) {
        queryParams.push(`year=${req.year}`);
    }
    if (req?.month !== undefined) {
        queryParams.push(`month=${req.month}`);
    }
    if (req?.quarter !== undefined) {
        queryParams.push(`quarter=${req.quarter}`);
    }
    if (req?.monthsHistory !== undefined) {
        queryParams.push(`months_history=${req.monthsHistory}`);
    }
    if (req?.forecastStrategy) {
        queryParams.push(`forecast_strategy=${encodeURIComponent(req.forecastStrategy)}`);
    }
    if (req?.startDate) {
        queryParams.push(`start_date=${encodeURIComponent(req.startDate)}`);
    }
    if (req?.endDate) {
        queryParams.push(`end_date=${encodeURIComponent(req.endDate)}`);
    }

    return queryParams.length > 0 ? `?${queryParams.join('&')}` : '';
}

export function mapRestExecutionToFrontend(restResult: any): any {
    const items = Array.isArray(restResult?.items) ? restResult.items : [];
    const summary = restResult?.summary || {};

    return {
        totalBudgetCents: toBudgetAmountCents(summary?.totalBudgetCents ?? summary?.total_budget_cents ?? 0),
        totalSpentCents: toBudgetAmountCents(summary?.totalSpentCents ?? summary?.total_spent_cents ?? 0),
        totalExecutionRate: summary?.overall_execution_rate ?? 0,
        categories: items.map((item: any) => ({
            budgetId: String(item?.id || ''),
            categoryId: String(item?.category_id || item?.category_info?.id || ''),
            categoryName: item?.sub_category ? `${item.category}-${item.sub_category}` : (item?.category || ''),
            categoryIcon: item?.category_info?.icon || '',
            categoryColor: item?.category_info?.color || '',
            budgetAmountCents: toBudgetAmountCents(item?.budgetAmountCents ?? item?.budget_amount_cents ?? 0),
            spentAmountCents: toBudgetAmountCents(item?.spentAmountCents ?? item?.spent_amount_cents ?? 0),
            remainingAmountCents: toBudgetAmountCents(item?.remainingAmountCents ?? item?.remaining_amount_cents ?? 0),
            executionRate: item?.execution_rate ?? 0,
            alertThreshold: item?.alert_threshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD,
            isOverBudget: toBudgetAmountCents(item?.spentAmountCents ?? item?.spent_amount_cents ?? 0) > toBudgetAmountCents(item?.budgetAmountCents ?? item?.budget_amount_cents ?? 0),
            alertTriggered: Number(item?.execution_rate ?? 0) >= Number(item?.alert_threshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD)
        })),
        periodStart: restResult?.periodStart || restResult?.period_start || '',
        periodEnd: restResult?.periodEnd || restResult?.period_end || ''
    };
}

export function mapRestForecastToFrontend(restResult: any): any {
    const items = Array.isArray(restResult?.items) ? restResult.items : [];
    const summary = restResult?.summary || {};

    return {
        forecasts: items.map((item: any) => {
            const projectedTotalCents = toBudgetAmountCents(item?.forecastAmountCents ?? item?.forecast_amount_cents ?? 0);
            const historicalAverageCents = toBudgetAmountCents(item?.averageAmountCents ?? item?.average_amount_cents ?? 0);
            const currentSpentCents = toBudgetAmountCents(
                item?.currentSpentCents
                    ?? item?.current_spent_cents
                    ?? item?.periods?.[item?.periods?.length - 1]?.amountCents
                    ?? item?.periods?.[item?.periods?.length - 1]?.amount_cents
                    ?? item?.totalAmountCents
                    ?? item?.total_amount_cents
                    ?? 0
            );
            const budgetAmountCents = toBudgetAmountCents(item?.budgetAmountCents ?? item?.budget_amount_cents ?? 0);

            return {
                categoryId: String(item?.category_info?.id || item?.categoryId || ''),
                categoryName: item?.category || item?.categoryName || '',
                historicalAverageCents,
                currentSpentCents,
                projectedTotalCents,
                budgetAmountCents,
                projectedOverBudget: !!(item?.projected_over_budget ?? item?.projectedOverBudget ?? (budgetAmountCents > 0 && projectedTotalCents > budgetAmountCents)),
                trend: item?.trend || 'stable',
                samplePeriods: item?.sample_periods ?? item?.samplePeriods ?? 0,
                strategyExplanation: item?.strategy_explanation || item?.strategyExplanation || '',
                backtestMape: item?.backtest_mape ?? item?.backtestMape ?? null,
                confidence: item?.confidence || 'low',
                periods: Array.isArray(item?.periods) ? item.periods.map((period: any) => ({
                    period: period?.period || '',
                    amountCents: toBudgetAmountCents(period?.amountCents ?? period?.amount_cents ?? 0)
                })) : []
            };
        }),
        periodStart: restResult?.periodStart || restResult?.period_start || '',
        periodEnd: restResult?.periodEnd || restResult?.period_end || '',
        daysRemaining: restResult?.daysRemaining ?? 0,
        daysElapsed: restResult?.daysElapsed ?? 0,
        forecastStrategy: summary?.forecast_strategy || summary?.forecastStrategy || BudgetForecastStrategy.HistoricalAverage,
        historyPeriods: summary?.history_periods ?? summary?.historyPeriods ?? DEFAULT_BUDGET_FORECAST_HISTORY_PERIODS,
        avgBacktestMape: summary?.avg_backtest_mape ?? summary?.avgBacktestMape ?? null
    };
}

export function mapRestHistoryToFrontend(restResult: any): any {
    const items = Array.isArray(restResult?.items) ? restResult.items : [];
    const summary = restResult?.summary || {};

    return {
        items: items.map((item: any) => {
            const budgetType = normalizeRestBudgetType(item?.budget_type ?? item?.budgetType ?? item?.type);

            return {
                id: String(item?.id ?? ''),
                budgetId: String(item?.budget_id ?? item?.budgetId ?? ''),
                name: item?.name || '',
                ...(budgetType === null ? {} : { type: budgetType }),
                category: item?.category || '',
                subCategory: item?.sub_category || item?.subCategory || '',
                periodType: item?.period_type || item?.periodType || BudgetPeriodType.Monthly,
                periodStart: item?.period_start || item?.periodStart || '',
                periodEnd: item?.period_end || item?.periodEnd || '',
                budgetAmountCents: toBudgetAmountCents(item?.budgetAmountCents ?? item?.budget_amount_cents ?? 0),
                spentAmountCents: toBudgetAmountCents(item?.spentAmountCents ?? item?.spent_amount_cents ?? 0),
                remainingAmountCents: toBudgetAmountCents(item?.remainingAmountCents ?? item?.remaining_amount_cents ?? 0),
                executionRate: item?.execution_rate ?? item?.executionRate ?? 0,
                status: item?.status || '',
                filterSummary: item?.filter_summary || item?.filterSummary || '',
                calculatedAt: item?.calculated_at || item?.calculatedAt || '',
                alertThreshold: item?.alert_threshold ?? item?.alertThreshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD,
                enabled: item?.enabled ?? DEFAULT_BUDGET_ENABLED
            };
        }),
        count: summary?.count ?? items.length,
        periodStart: summary?.period_start || summary?.periodStart || '',
        periodEnd: summary?.period_end || summary?.periodEnd || ''
    };
}

export function mapBudgetRequestToRest(req: any): any {
    return {
        name: req?.name || '',
        category: req?.category || '',
        sub_category: req?.subCategory || '',
        period_type: req?.periodType || BudgetPeriodType.Monthly,
        amount_cents: toBudgetAmountCents(req?.amountCents ?? 0),
        start_date: req?.startDate,
        end_date: req?.endDate,
        alert_threshold: req?.alertThreshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD,
        enabled: req?.enabled ?? DEFAULT_BUDGET_ENABLED
    };
}

export function mapImportedBudgetToRest(budget: any): any {
    const amountCents = toBudgetAmountCents(budget?.amountCents ?? budget?.amount_cents ?? 0);

    return {
        name: budget?.name || '',
        category: budget?.category || '',
        sub_category: budget?.subCategory || budget?.sub_category || '',
        period_type: budget?.periodType || budget?.period_type || BudgetPeriodType.Monthly,
        amount_cents: amountCents,
        start_date: budget?.startDate || budget?.start_date,
        end_date: budget?.endDate || budget?.end_date,
        alert_threshold: budget?.alertThreshold ?? budget?.alert_threshold ?? DEFAULT_BUDGET_ALERT_THRESHOLD,
        enabled: budget?.enabled ?? DEFAULT_BUDGET_ENABLED
    };
}
