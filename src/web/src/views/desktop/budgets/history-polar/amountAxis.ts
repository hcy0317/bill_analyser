import type {
    HistoricalGridLineAnimationFrame,
    HistoricalGridLineAnimationFrameInput,
    HistoricalGridLineAnimationSnapshot,
    HistoricalLabelAnimationState
} from './types.ts';
import {
    AMOUNT_AXIS_GRID_LINE_COUNT,
    AMOUNT_AXIS_OUTER_TRANSITION_RADIUS_RATIO,
    AMOUNT_AXIS_SPLIT_LINE_DARK_COLOR
} from './constants.ts';
import {
    clamp,
    formatHistoricalGridAmountKey,
    toFiniteNumber,
    toNonNegativeFiniteNumber
} from './math.ts';
import { isHistoricalGridLineSnapshot } from './state.ts';

export function getHistoricalAmountGridLineValues(amountAxisMax: number): number[] {
    const maxValue = toNonNegativeFiniteNumber(amountAxisMax);
    if (maxValue <= 0) {
        return [];
    }

    const step = maxValue / (AMOUNT_AXIS_GRID_LINE_COUNT - 1);

    return Array.from({ length: AMOUNT_AXIS_GRID_LINE_COUNT }, (_, index) => (
        Number((step * index).toFixed(4))
    ));
}

type HistoricalAmountAxisFrameMode = 'line' | 'label';

function getHistoricalAmountAxisFrameIndex(stateKey: string): number {
    const matched = /:(\d+)$/.exec(stateKey);

    return matched ? Number(matched[1]) : 0;
}

function collectHistoricalAmountAxisSnapshots(
    state: HistoricalLabelAnimationState | undefined,
    stateKeyPrefix: string | undefined
): Map<number, HistoricalGridLineAnimationSnapshot> {
    const snapshots = new Map<number, HistoricalGridLineAnimationSnapshot>();
    if (!state || !stateKeyPrefix) {
        return snapshots;
    }

    for (const [stateKey, snapshot] of state.entries()) {
        if (
            !stateKey.startsWith(stateKeyPrefix)
            || !isHistoricalGridLineSnapshot(snapshot)
            || snapshot.visible === false
        ) {
            continue;
        }

        const axisIndex = Number.isFinite(snapshot.axisIndex)
            ? Number(snapshot.axisIndex)
            : getHistoricalAmountAxisFrameIndex(stateKey);
        snapshots.set(axisIndex, snapshot);
    }

    return snapshots;
}

function getHistoricalAmountAxisPreviousMaxValue(
    snapshots: Map<number, HistoricalGridLineAnimationSnapshot>
): number {
    return Math.max(
        0,
        ...Array.from(snapshots.values()).map(snapshot => toNonNegativeFiniteNumber(snapshot.radiusAxisMaxValue))
    );
}

function resolveHistoricalAmountAxisSourceIndex(
    axisIndex: number,
    previousMaxValue: number,
    currentMaxValue: number
): number | null {
    if (previousMaxValue <= 0 || Math.abs(previousMaxValue - currentMaxValue) < 0.0001) {
        return axisIndex;
    }

    if (currentMaxValue < previousMaxValue) {
        return Math.max(0, axisIndex - 1);
    }

    if (axisIndex === 0) {
        return 0;
    }

    if (axisIndex >= AMOUNT_AXIS_GRID_LINE_COUNT - 1) {
        return null;
    }

    return axisIndex + 1;
}

function buildHistoricalAmountAxisLeavingFrames(
    previousSnapshots: Map<number, HistoricalGridLineAnimationSnapshot>,
    previousMaxValue: number,
    currentMaxValue: number,
    color: string,
    mode: HistoricalAmountAxisFrameMode
): HistoricalGridLineAnimationFrame[] {
    if (previousMaxValue <= 0 || Math.abs(previousMaxValue - currentMaxValue) < 0.0001) {
        return [];
    }

    const leavingIndices = mode === 'label'
        ? Array.from(previousSnapshots.keys())
        : [currentMaxValue < previousMaxValue ? AMOUNT_AXIS_GRID_LINE_COUNT - 1 : 1];

    return leavingIndices.flatMap(axisIndex => {
        const previous = previousSnapshots.get(axisIndex);
        if (!previous?.dataId) {
            return [];
        }

        return [{
            stateKey: `amount-axis-ghost:${mode}:${axisIndex}:${previous.dataId}`,
            dataId: previous.dataId,
            name: previous.name ?? formatHistoricalGridAmountKey(previous.amountValue),
            axisIndex,
            amountValue: previous.amountValue,
            radiusAxisMaxValue: currentMaxValue > 0 ? currentMaxValue : previous.radiusAxisMaxValue,
            radiusRatio: clamp(toFiniteNumber(previous.radiusRatio, 0), 0, 1),
            color: previous.color ?? color,
            opacity: 0,
            previous,
            leaving: true,
            labelGhost: mode === 'label',
            rangeChanged: true
        }];
    });
}

export function resolveHistoricalGridLineAnimationFrames(
    inputs: HistoricalGridLineAnimationFrameInput[],
    state?: HistoricalLabelAnimationState,
    stateKeyPrefix?: string,
    mode: HistoricalAmountAxisFrameMode = 'line'
): HistoricalGridLineAnimationFrame[] {
    const visibleKeys = new Set(inputs.map(input => input.stateKey));
    const currentRadiusAxisMaxValue = inputs.reduce(
        (maxValue, input) => Math.max(maxValue, toNonNegativeFiniteNumber(input.radiusAxisMaxValue)),
        0
    );
    const previousSnapshots = collectHistoricalAmountAxisSnapshots(state, stateKeyPrefix);
    const previousMaxValue = getHistoricalAmountAxisPreviousMaxValue(previousSnapshots);
    const rangeChanged = previousMaxValue > 0 && Math.abs(previousMaxValue - currentRadiusAxisMaxValue) >= 0.0001;
    const frames = inputs.map(input => {
        const axisIndex = Number.isFinite(input.axisIndex)
            ? Number(input.axisIndex)
            : getHistoricalAmountAxisFrameIndex(input.stateKey);
        const sourceIndex = resolveHistoricalAmountAxisSourceIndex(
            axisIndex,
            previousMaxValue,
            currentRadiusAxisMaxValue
        );
        const previousSnapshot = sourceIndex === null
            ? undefined
            : previousSnapshots.get(sourceIndex) ?? state?.get(input.stateKey);
        const previous = isHistoricalGridLineSnapshot(previousSnapshot) && previousSnapshot.visible !== false
            ? previousSnapshot
            : null;
        const radiusAxisMaxValue = Math.max(1, toNonNegativeFiniteNumber(input.radiusAxisMaxValue));
        const amountValue = toNonNegativeFiniteNumber(input.amountValue);
        const radiusRatio = clamp(amountValue / radiusAxisMaxValue, 0, 1);
        const frame: HistoricalGridLineAnimationFrame = {
            ...input,
            axisIndex,
            amountValue,
            radiusAxisMaxValue,
            radiusRatio,
            opacity: 1,
            previous,
            leaving: false,
            labelGhost: false,
            rangeChanged
        };

        state?.set(input.stateKey, {
            kind: 'grid-line',
            axisIndex,
            amountValue: frame.amountValue,
            radiusAxisMaxValue: frame.radiusAxisMaxValue,
            radiusRatio: frame.radiusRatio,
            dataId: frame.dataId,
            name: frame.name,
            color: frame.color,
            visible: true,
            ghostEmitted: false
        });

        return frame;
    });

    frames.push(...buildHistoricalAmountAxisLeavingFrames(
        previousSnapshots,
        previousMaxValue,
        currentRadiusAxisMaxValue,
        inputs[0]?.color ?? AMOUNT_AXIS_SPLIT_LINE_DARK_COLOR,
        mode
    ));

    if (!state) {
        return frames;
    }

    for (const [stateKey, snapshot] of Array.from(state.entries())) {
        if (stateKeyPrefix && !stateKey.startsWith(stateKeyPrefix)) {
            continue;
        }

        if (
            visibleKeys.has(stateKey)
            || !isHistoricalGridLineSnapshot(snapshot)
            || (snapshot.visible === false && snapshot.ghostEmitted)
            || !snapshot.dataId
            || !snapshot.name
            || !snapshot.color
        ) {
            continue;
        }

        const radiusAxisMaxValue = currentRadiusAxisMaxValue > 0
            ? currentRadiusAxisMaxValue
            : snapshot.radiusAxisMaxValue;
        const radiusRatio = clamp(toFiniteNumber(snapshot.radiusRatio, 0), 0, 1);

        frames.push({
            stateKey,
            dataId: snapshot.dataId,
            name: snapshot.name,
            axisIndex: Number.isFinite(snapshot.axisIndex)
                ? Number(snapshot.axisIndex)
                : getHistoricalAmountAxisFrameIndex(stateKey),
            amountValue: radiusAxisMaxValue * radiusRatio,
            radiusAxisMaxValue,
            radiusRatio,
            color: snapshot.color,
            opacity: 0,
            previous: snapshot,
            leaving: true,
            labelGhost: false,
            rangeChanged: false
        });

        state.set(stateKey, {
            ...snapshot,
            visible: false,
            ghostEmitted: true
        });
    }

    return frames;
}

export function resolveHistoricalAmountAxisStartRadiusRatio(frame: HistoricalGridLineAnimationFrame): number {
    if (!frame.previous) {
        return clamp(toFiniteNumber(frame.radiusRatio, 0), 0, 1);
    }

    return clamp(toFiniteNumber(frame.previous.radiusRatio, frame.radiusRatio), 0, 1);
}

export function getHistoricalAmountAxisTransitionEdgeRatio(radiusRatio: number): number {
    return clamp(toFiniteNumber(radiusRatio, 0), 0, 1) >= 0.5
        ? AMOUNT_AXIS_OUTER_TRANSITION_RADIUS_RATIO
        : 0;
}

export function isHistoricalAmountAxisRenderRestart(frame: HistoricalGridLineAnimationFrame): boolean {
    return !!frame.previous && frame.previous.dataId !== frame.dataId;
}

export function getHistoricalAmountAxisInterval(maxAmount: number): number {
    if (maxAmount <= 0) {
        return 25;
    }

    const roughInterval = maxAmount / 4;
    const magnitude = Math.pow(10, Math.floor(Math.log10(roughInterval)));
    const normalized = roughInterval / magnitude;

    if (normalized <= 1) {
        return magnitude;
    }
    if (normalized <= 2) {
        return 2 * magnitude;
    }
    if (normalized <= 2.5) {
        return 2.5 * magnitude;
    }
    if (normalized <= 5) {
        return 5 * magnitude;
    }

    return 10 * magnitude;
}
