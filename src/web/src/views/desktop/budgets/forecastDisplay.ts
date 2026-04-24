import type { BudgetForecastItem } from '@/models/budget.ts';

export type ForecastSortBy = 'backtest' | 'confidence' | 'projected_total' | 'category';
export type ForecastConfidence = BudgetForecastItem['confidence'];

export interface ForecastDisplayOptions {
    readonly sortBy: ForecastSortBy;
    readonly onlyLowConfidence?: boolean;
    readonly onlyOverBudget?: boolean;
}

export interface ForecastRiskSummary {
    readonly totalCount: number;
    readonly lowConfidenceCount: number;
    readonly overBudgetCount: number;
    readonly filteredCount: number;
}

const FORECAST_CATEGORY_NAME_COLLATOR = new Intl.Collator('zh-Hans-CN-u-co-pinyin');

/**
 * 预测置信等级排序权重（越小越优先）
 */
export function getForecastConfidenceRank(confidence?: ForecastConfidence | null): number {
    switch (confidence) {
        case 'high':
            return 0;
        case 'medium':
            return 1;
        default:
            return 2;
    }
}

/**
 * 可空数字比较，空值排在最后
 */
export function compareNullableNumbers(left?: number | null, right?: number | null): number {
    const leftMissing = left === null || left === undefined;
    const rightMissing = right === null || right === undefined;

    if (leftMissing && rightMissing) {
        return 0;
    }
    if (leftMissing) {
        return 1;
    }
    if (rightMissing) {
        return -1;
    }

    return (left as number) - (right as number);
}

function compareForecastCategoryNames(left: string, right: string): number {
    return FORECAST_CATEGORY_NAME_COLLATOR.compare(left, right);
}

/**
 * 对预算预测结果执行快速筛选和排序
 */
export function filterAndSortForecasts(
    forecasts: readonly BudgetForecastItem[],
    options: ForecastDisplayOptions
): BudgetForecastItem[] {
    const filtered = [...forecasts].filter(forecast => {
        if (options.onlyLowConfidence && forecast.confidence !== 'low') {
            return false;
        }
        if (options.onlyOverBudget && !forecast.projectedOverBudget) {
            return false;
        }
        return true;
    });

    filtered.sort((a, b) => {
        switch (options.sortBy) {
            case 'confidence':
                return getForecastConfidenceRank(a.confidence) - getForecastConfidenceRank(b.confidence)
                    || compareNullableNumbers(a.backtestMape, b.backtestMape)
                    || compareForecastCategoryNames(a.categoryName, b.categoryName);
            case 'projected_total':
                return b.projectedTotal - a.projectedTotal
                    || compareNullableNumbers(a.backtestMape, b.backtestMape)
                    || compareForecastCategoryNames(a.categoryName, b.categoryName);
            case 'category':
                return compareForecastCategoryNames(a.categoryName, b.categoryName);
            case 'backtest':
            default:
                return compareNullableNumbers(a.backtestMape, b.backtestMape)
                    || getForecastConfidenceRank(a.confidence) - getForecastConfidenceRank(b.confidence)
                    || compareForecastCategoryNames(a.categoryName, b.categoryName);
        }
    });

    return filtered;
}

/**
 * 生成预测风险摘要统计
 */
export function summarizeForecastRisks(
    forecasts: readonly BudgetForecastItem[],
    filteredCount: number
): ForecastRiskSummary {
    return {
        totalCount: forecasts.length,
        lowConfidenceCount: forecasts.filter(item => item.confidence === 'low').length,
        overBudgetCount: forecasts.filter(item => item.projectedOverBudget).length,
        filteredCount
    };
}
