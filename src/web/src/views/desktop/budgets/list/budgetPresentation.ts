export const CATEGORY_CHART_PALETTE = [
    '#5470c6', '#91cc75', '#fac858', '#ee6666', '#73c0de',
    '#3ba272', '#fc8452', '#9a60b4', '#ea7ccc', '#48b8d0'
];

/**
 * 统一预算分类颜色值，去掉可选的 # 前缀，便于图表与 CSS 复用。
 */
export function normalizeCategoryColor(color: string | null | undefined): string {
    return String(color || '').trim().replace(/^#/, '');
}

/**
 * 将分类颜色恢复为 CSS 十六进制格式，空值保持为空字符串。
 */
export function toCssCategoryColor(color: string | null | undefined): string {
    const normalizedColor = normalizeCategoryColor(color);
    return normalizedColor ? `#${normalizedColor}` : '';
}

/**
 * 按执行率阈值选择 Naive UI 进度条语义色。
 */
export function getExecutionRateColor(rate: number): string {
    if (rate >= 100) return 'error';
    if (rate >= 80) return 'warning';
    if (rate >= 50) return 'info';
    return 'success';
}

/**
 * 为预算进度条复用执行率语义色，保留页面原有展示规则。
 */
export function getProgressColorByRate(rate: number): string {
    return getExecutionRateColor(rate);
}

/**
 * 按执行率返回预算金额文字颜色类，超预算时突出错误色。
 */
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

/**
 * 将预测置信度枚举转为列表展示文案。
 */
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

/**
 * 将预测置信度映射为页面文字颜色类。
 */
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

/**
 * 按预算列表原有人民币展示格式格式化元金额。
 */
export function formatAmount(amount: number): string {
    return '¥' + amount.toFixed(2);
}
