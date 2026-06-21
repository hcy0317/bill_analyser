/**
 * 将数值限制在指定闭区间内，供图表动画进度和半径计算复用。
 */
export function clamp(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
}

/**
 * 将可选数值规范化为有限数，非法时使用兜底值。
 */
export function toFiniteNumber(value: number | undefined, fallback = 0): number {
    const numericValue = Number(value);
    return Number.isFinite(numericValue) ? numericValue : fallback;
}

/**
 * 将可选数值规整为非负有限数，适用于金额和半径输入。
 */
export function toNonNegativeFiniteNumber(value: number | undefined): number {
    return Math.max(0, toFiniteNumber(value));
}

/**
 * 将排序权重规整为有限数，非法值排到最后。
 */
export function toSortOrder(value: number): number {
    return toFiniteNumber(value, Number.MAX_SAFE_INTEGER);
}

/**
 * 统一历史图表标签文本，避免空白差异影响 key 匹配。
 */
export function normalizeText(value: string): string {
    return String(value ?? '').trim();
}

/**
 * 为十六进制颜色附加透明度，非十六进制颜色保持原值。
 */
export function withAlpha(color: string, alpha: number): string {
    const normalizedColor = normalizeText(color).replace('#', '');
    if (!/^[0-9a-fA-F]{6}$/.test(normalizedColor)) {
        return `rgba(0,0,0,${alpha})`;
    }

    const matched = normalizedColor.match(/.{2}/g)!;
    const [red, green, blue] = matched.map(item => parseInt(item, 16));
    return `rgba(${red},${green},${blue},${alpha})`;
}

/**
 * 将角度归一化到 0 到 360 度区间。
 */
export function normalizeCircleAngle(angle: number): number {
    if (!Number.isFinite(angle)) {
        return 0;
    }

    return ((angle % 360) + 360) % 360;
}

/**
 * 将文字旋转角度归一化到 -180 到 180 度，避免标签倒置。
 */
export function normalizeRotation(rotation: number): number {
    return ((rotation + 180) % 360 + 360) % 360 - 180;
}

/**
 * 将角度值转换为弧度值。
 */
export function degreesToRadians(degrees: number): number {
    return (degrees * Math.PI) / 180;
}

/**
 * 将弧度归一化到 0 到 2π 区间。
 */
export function normalizeRadians(radians: number): number {
    const fullCircle = Math.PI * 2;
    return ((radians + Math.PI) % fullCircle + fullCircle) % fullCircle - Math.PI;
}

/**
 * 在圆周上选择距离上一帧最近的目标弧度，避免跨 0 度动画跳变。
 */
export function resolveNearestCircularRadian(targetAngle: number, previousAngle: number): number {
    if (!Number.isFinite(targetAngle) || !Number.isFinite(previousAngle)) {
        return Number.isFinite(targetAngle) ? targetAngle : 0;
    }

    return previousAngle + normalizeRadians(targetAngle - previousAngle);
}

/**
 * 按动画进度线性插值两个数值。
 */
export function interpolateNumber(startValue: number, endValue: number, progress: number): number {
    return startValue + ((endValue - startValue) * progress);
}

/**
 * 将金额刻度值格式化为稳定 key，避免浮点误差导致动画状态错配。
 */
export function formatHistoricalGridAmountKey(amountValue: number): string {
    return Number(amountValue.toFixed(4)).toString();
}
