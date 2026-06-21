export const CATEGORY_CHART_PALETTE = [
    '#5470c6', '#91cc75', '#fac858', '#ee6666', '#73c0de',
    '#3ba272', '#fc8452', '#9a60b4', '#ea7ccc', '#48b8d0'
];

export function normalizeCategoryColor(color: string | null | undefined): string {
    return String(color || '').trim().replace(/^#/, '');
}

export function toCssCategoryColor(color: string | null | undefined): string {
    const normalizedColor = normalizeCategoryColor(color);
    return normalizedColor ? `#${normalizedColor}` : '';
}

export function getExecutionRateColor(rate: number): string {
    if (rate >= 100) return 'error';
    if (rate >= 80) return 'warning';
    if (rate >= 50) return 'info';
    return 'success';
}

export function getProgressColorByRate(rate: number): string {
    return getExecutionRateColor(rate);
}

export function getExecutionRateTextClass(rate: number): string {
    if (rate >= 100) return 'text-error';
    return 'text-default';
}

/**
 * 按预算执行率阈值返回汇总行颜色类。
 */
export function getExecutionRateColorClass(rate: number): string {
    if (rate >= 100) return 'text-error';
    if (rate >= 80) return 'text-warning';
    if (rate >= 50) return 'text-orange';
    return 'text-success';
}

export function getForecastConfidenceLabel(confidence?: 'high' | 'medium' | 'low' | null): string {
    switch (confidence) {
        case 'high':
            return 'High Confidence';
        case 'medium':
            return 'Medium Confidence';
        default:
            return 'Low Confidence';
    }
}

export function getForecastConfidenceClass(confidence?: 'high' | 'medium' | 'low' | null): string {
    switch (confidence) {
        case 'high':
            return 'text-success';
        case 'medium':
            return 'text-warning';
        default:
            return 'text-error';
    }
}

export function formatAmount(amount: number): string {
    return '¥' + amount.toFixed(2);
}
