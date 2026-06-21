import type {
    BudgetForecastRequest,
    BudgetForecastStrategy,
    BudgetHistoryRequest,
    BudgetType
} from '@/models/budget.ts';

export interface BudgetForecastLoadRequestInput {
    readonly budgetType: BudgetType;
    readonly periodRequest: BudgetHistoryRequest;
    readonly monthsHistory: number;
    readonly forecastStrategy: BudgetForecastStrategy;
}

/**
 * 根据预算预测页的筛选状态构造后端 forecast 请求参数。
 */
export function buildBudgetForecastLoadRequest({
    budgetType,
    periodRequest,
    monthsHistory,
    forecastStrategy
}: BudgetForecastLoadRequestInput): BudgetForecastRequest & { type: BudgetType } {
    return {
        type: budgetType,
        periodType: periodRequest.periodType,
        year: periodRequest.year,
        month: periodRequest.month,
        quarter: periodRequest.quarter,
        monthsHistory,
        forecastStrategy
    };
}
