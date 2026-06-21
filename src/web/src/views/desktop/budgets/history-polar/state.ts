import type {
    HistoricalAnimationSnapshot,
    HistoricalGridLineAnimationSnapshot,
    HistoricalLabelAnimationSnapshot,
    HistoricalLabelAnimationState,
    HistoricalSectorAnimationSnapshot
} from './types.ts';
import { AMOUNT_AXIS_STATE_PREFIXES } from './constants.ts';
import { formatHistoricalGridAmountKey, normalizeText } from './math.ts';

/**
 * 规范化历史图表动画作用域，空值归入 default。
 */
export function normalizeHistoricalAnimationScope(scope?: string): string {
    return normalizeText(scope ?? '').replace(/[^a-zA-Z0-9_-]+/g, '_');
}

/**
 * 构造带作用域的动画状态 key，避免多个图表实例互相覆盖。
 */
export function buildHistoricalScopedKey(prefix: string, key: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${prefix}${normalizedScope}:${key}` : `${prefix}${key}`;
}

/**
 * 构造作用域前缀，用于批量清理同一图表实例的动画状态。
 */
export function buildHistoricalScopedPrefix(prefix: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${prefix}${normalizedScope}:` : prefix;
}

/**
 * 构造金额轴自定义系列的渲染 ID。
 */
export function buildHistoricalAmountAxisRenderId(baseId: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${baseId}:${normalizedScope}` : baseId;
}

/**
 * 构造金额轴渲染作用域，将最大金额纳入 key 以触发刻度动画重启。
 */
export function buildHistoricalAmountAxisRenderScope(scope: string, amountAxisMax: number): string {
    const axisScope = `axis-${formatHistoricalGridAmountKey(amountAxisMax)}`;

    return normalizeHistoricalAnimationScope(scope ? `${scope}-${axisScope}` : axisScope);
}

/**
 * 构造带作用域的系列 ID，避免多个历史图表的 ECharts 系列冲突。
 */
export function buildHistoricalScopedSeriesId(baseId: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${baseId}:${normalizedScope}` : baseId;
}

/**
 * 创建历史图表标签/扇区/金额轴共享的动画状态容器。
 */
export function createHistoricalLabelAnimationState(): HistoricalLabelAnimationState {
    return new Map();
}

/**
 * 清空所有历史标签动画快照。
 */
export function resetHistoricalLabelAnimationState(state: HistoricalLabelAnimationState): void {
    state.clear();
}

/**
 * 清理分类扇区和二级标签动画快照，保留其他作用域状态。
 */
export function resetHistoricalCategoryAnimationState(state: HistoricalLabelAnimationState): void {
    for (const key of Array.from(state.keys())) {
        if (!AMOUNT_AXIS_STATE_PREFIXES.some(prefix => key.startsWith(prefix))) {
            state.delete(key);
        }
    }
}

/**
 * 判断动画快照是否为标签快照。
 */
export function isHistoricalLabelSnapshot(
    snapshot: HistoricalAnimationSnapshot | undefined
): snapshot is HistoricalLabelAnimationSnapshot {
    const labelSnapshot = snapshot as HistoricalLabelAnimationSnapshot | undefined;
    return !!snapshot
        && (snapshot as HistoricalSectorAnimationSnapshot).kind !== 'sector'
        && (snapshot as HistoricalGridLineAnimationSnapshot).kind !== 'grid-line'
        && typeof labelSnapshot?.radiusValue === 'number'
        && typeof labelSnapshot?.polarAngleValue === 'number'
        && typeof labelSnapshot?.rotate === 'number';
}

/**
 * 判断动画快照是否为扇区快照。
 */
export function isHistoricalSectorSnapshot(
    snapshot: HistoricalAnimationSnapshot | undefined
): snapshot is HistoricalSectorAnimationSnapshot {
    return !!snapshot
        && (snapshot as HistoricalSectorAnimationSnapshot).kind === 'sector'
        && typeof (snapshot as HistoricalSectorAnimationSnapshot).startAngleValue === 'number'
        && typeof (snapshot as HistoricalSectorAnimationSnapshot).endAngleValue === 'number';
}

/**
 * 判断动画快照是否为金额轴网格线快照。
 */
export function isHistoricalGridLineSnapshot(
    snapshot: HistoricalAnimationSnapshot | undefined
): snapshot is HistoricalGridLineAnimationSnapshot {
    return !!snapshot
        && (snapshot as HistoricalGridLineAnimationSnapshot).kind === 'grid-line'
        && typeof (snapshot as HistoricalGridLineAnimationSnapshot).radiusAxisMaxValue === 'number'
        && typeof (snapshot as HistoricalGridLineAnimationSnapshot).radiusRatio === 'number';
}
