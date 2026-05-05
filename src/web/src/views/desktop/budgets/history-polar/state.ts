import type {
    HistoricalAnimationSnapshot,
    HistoricalGridLineAnimationSnapshot,
    HistoricalLabelAnimationSnapshot,
    HistoricalLabelAnimationState,
    HistoricalSectorAnimationSnapshot
} from './types.ts';
import { AMOUNT_AXIS_STATE_PREFIXES } from './constants.ts';
import { formatHistoricalGridAmountKey, normalizeText } from './math.ts';

export function normalizeHistoricalAnimationScope(scope?: string): string {
    return normalizeText(scope ?? '').replace(/[^a-zA-Z0-9_-]+/g, '_');
}

export function buildHistoricalScopedKey(prefix: string, key: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${prefix}${normalizedScope}:${key}` : `${prefix}${key}`;
}

export function buildHistoricalScopedPrefix(prefix: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${prefix}${normalizedScope}:` : prefix;
}

export function buildHistoricalAmountAxisRenderId(baseId: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${baseId}:${normalizedScope}` : baseId;
}

export function buildHistoricalAmountAxisRenderScope(scope: string, amountAxisMax: number): string {
    const axisScope = `axis-${formatHistoricalGridAmountKey(amountAxisMax)}`;

    return normalizeHistoricalAnimationScope(scope ? `${scope}-${axisScope}` : axisScope);
}

export function buildHistoricalScopedSeriesId(baseId: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${baseId}:${normalizedScope}` : baseId;
}

export function createHistoricalLabelAnimationState(): HistoricalLabelAnimationState {
    return new Map();
}

export function resetHistoricalLabelAnimationState(state: HistoricalLabelAnimationState): void {
    state.clear();
}

export function resetHistoricalCategoryAnimationState(state: HistoricalLabelAnimationState): void {
    for (const key of Array.from(state.keys())) {
        if (!AMOUNT_AXIS_STATE_PREFIXES.some(prefix => key.startsWith(prefix))) {
            state.delete(key);
        }
    }
}

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

export function isHistoricalSectorSnapshot(
    snapshot: HistoricalAnimationSnapshot | undefined
): snapshot is HistoricalSectorAnimationSnapshot {
    return !!snapshot
        && (snapshot as HistoricalSectorAnimationSnapshot).kind === 'sector'
        && typeof (snapshot as HistoricalSectorAnimationSnapshot).startAngleValue === 'number'
        && typeof (snapshot as HistoricalSectorAnimationSnapshot).endAngleValue === 'number';
}

export function isHistoricalGridLineSnapshot(
    snapshot: HistoricalAnimationSnapshot | undefined
): snapshot is HistoricalGridLineAnimationSnapshot {
    return !!snapshot
        && (snapshot as HistoricalGridLineAnimationSnapshot).kind === 'grid-line'
        && typeof (snapshot as HistoricalGridLineAnimationSnapshot).radiusAxisMaxValue === 'number'
        && typeof (snapshot as HistoricalGridLineAnimationSnapshot).radiusRatio === 'number';
}
