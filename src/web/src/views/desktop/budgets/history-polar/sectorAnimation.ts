import type {
    HistoricalLabelAnimationState,
    HistoricalSectorAnimationFrame,
    HistoricalSectorAnimationFrameInput,
    HistoricalSectorAnimationSnapshot,
    HistoricalSectorKeyframe,
    HistoricalSectorRenderApi,
    HistoricalSectorRenderParams,
    HistoricalSectorShape
} from './types.ts';
import {
    HISTORY_ANIMATION_DURATION,
    HISTORY_ANIMATION_EASING
} from './constants.ts';
import {
    clamp,
    degreesToRadians,
    interpolateNumber,
    normalizeCircleAngle,
    resolveNearestCircularRadian,
    toFiniteNumber,
    toNonNegativeFiniteNumber
} from './math.ts';
import {
    interpolateHistoricalPolarAngle,
    resolveNearestCircularAngle
} from './geometry.ts';
import {
    getPolarCanvasAngle,
    getPolarRadiusPx,
    getSectorCenter
} from './renderUtils.ts';
import { isHistoricalSectorSnapshot } from './state.ts';

function getClockwiseAngleSpan(startAngleValue: number, endAngleValue: number): number {
    const rawSpan = toFiniteNumber(endAngleValue - startAngleValue, 0);
    if (rawSpan > 0) {
        return rawSpan;
    }

    return normalizeCircleAngle(endAngleValue - startAngleValue) || 0.1;
}

function getSectorCollapsedSnapshot(
    frame: HistoricalSectorAnimationFrameInput
): HistoricalSectorAnimationSnapshot {
    const span = getClockwiseAngleSpan(frame.startAngleValue, frame.endAngleValue);
    const midpoint = frame.startAngleValue + (span / 2);
    const isRadiusCollapse = frame.collapseMode === 'radius';

    return {
        kind: 'sector',
        startAngleValue: isRadiusCollapse ? frame.startAngleValue : midpoint,
        endAngleValue: isRadiusCollapse ? frame.endAngleValue : midpoint,
        innerRadiusValue: frame.innerRadiusValue,
        outerRadiusValue: isRadiusCollapse ? frame.innerRadiusValue : frame.outerRadiusValue,
        radiusAxisMaxValue: frame.radiusAxisMaxValue,
        innerRadiusRatio: frame.innerRadiusRatio ?? 0,
        outerRadiusRatio: isRadiusCollapse
            ? (frame.innerRadiusRatio ?? 0)
            : (frame.outerRadiusRatio ?? 1),
        dataId: frame.dataId,
        name: frame.name,
        color: frame.color,
        opacity: 0,
        borderColor: frame.borderColor,
        borderWidth: frame.borderWidth,
        collapseMode: frame.collapseMode,
        visible: false,
        ghostEmitted: false
    };
}

export function resolveHistoricalSectorAnimationFrames(
    inputs: HistoricalSectorAnimationFrameInput[],
    state?: HistoricalLabelAnimationState,
    stateKeyPrefix?: string
): HistoricalSectorAnimationFrame[] {
    const visibleKeys = new Set(inputs.map(input => input.stateKey));
    const currentRadiusAxisMaxValue = inputs.reduce(
        (maxValue, input) => Math.max(maxValue, toNonNegativeFiniteNumber(input.radiusAxisMaxValue)),
        0
    );
    const frames = inputs.map(input => {
        const previousSnapshot = state?.get(input.stateKey);
        const previous = isHistoricalSectorSnapshot(previousSnapshot) && previousSnapshot.visible !== false
            ? previousSnapshot
            : null;
        const normalizedStartAngle = normalizeCircleAngle(input.startAngleValue);
        const spanAngle = getClockwiseAngleSpan(input.startAngleValue, input.endAngleValue);
        const startAngleValue = previous
            ? resolveNearestCircularAngle(normalizedStartAngle, previous.startAngleValue)
            : normalizedStartAngle;
        const radiusAxisMaxValue = Math.max(1, toNonNegativeFiniteNumber(input.radiusAxisMaxValue));
        const innerRadiusValue = toNonNegativeFiniteNumber(input.innerRadiusValue);
        const outerRadiusValue = Math.max(innerRadiusValue, toNonNegativeFiniteNumber(input.outerRadiusValue));
        const innerRadiusRatio = clamp(
            toFiniteNumber(input.innerRadiusRatio ?? (innerRadiusValue / radiusAxisMaxValue)),
            0,
            1
        );
        const outerRadiusRatio = Math.max(
            innerRadiusRatio,
            clamp(toFiniteNumber(input.outerRadiusRatio ?? (outerRadiusValue / radiusAxisMaxValue)), 0, 1)
        );
        const frame: HistoricalSectorAnimationFrame = {
            ...input,
            startAngleValue,
            endAngleValue: startAngleValue + spanAngle,
            innerRadiusValue,
            outerRadiusValue,
            radiusAxisMaxValue,
            innerRadiusRatio,
            outerRadiusRatio,
            opacity: clamp(toFiniteNumber(input.opacity, 1), 0, 1),
            previous,
            leaving: false
        };

        state?.set(input.stateKey, {
            kind: 'sector',
            startAngleValue: frame.startAngleValue,
            endAngleValue: frame.endAngleValue,
            innerRadiusValue: frame.innerRadiusValue,
            outerRadiusValue: frame.outerRadiusValue,
            radiusAxisMaxValue: frame.radiusAxisMaxValue,
            innerRadiusRatio,
            outerRadiusRatio,
            dataId: frame.dataId,
            name: frame.name,
            color: frame.color,
            opacity: frame.opacity,
            borderColor: frame.borderColor,
            borderWidth: frame.borderWidth,
            collapseMode: frame.collapseMode,
            visible: true,
            ghostEmitted: false
        });

        return frame;
    });

    if (!state) {
        return frames;
    }

    for (const [stateKey, snapshot] of Array.from(state.entries())) {
        if (stateKeyPrefix && !stateKey.startsWith(stateKeyPrefix)) {
            continue;
        }

        if (
            visibleKeys.has(stateKey)
            || !isHistoricalSectorSnapshot(snapshot)
            || (snapshot.visible === false && snapshot.ghostEmitted)
            || !snapshot.dataId
            || !snapshot.name
            || !snapshot.color
        ) {
            continue;
        }

        const collapseMode = snapshot.collapseMode ?? 'angle';
        const radiusAxisMaxValue = currentRadiusAxisMaxValue > 0
            ? currentRadiusAxisMaxValue
            : snapshot.radiusAxisMaxValue;
        const collapsed = getSectorCollapsedSnapshot({
            stateKey,
            dataId: snapshot.dataId,
            name: snapshot.name,
            startAngleValue: snapshot.startAngleValue,
            endAngleValue: snapshot.endAngleValue,
            innerRadiusValue: snapshot.innerRadiusValue,
            outerRadiusValue: snapshot.outerRadiusValue,
            radiusAxisMaxValue,
            innerRadiusRatio: snapshot.innerRadiusRatio,
            outerRadiusRatio: snapshot.outerRadiusRatio,
            color: snapshot.color,
            opacity: 0,
            borderColor: snapshot.borderColor,
            borderWidth: snapshot.borderWidth,
            collapseMode
        });

        frames.push({
            stateKey,
            dataId: snapshot.dataId,
            name: snapshot.name,
            startAngleValue: collapsed.startAngleValue,
            endAngleValue: collapsed.endAngleValue,
            innerRadiusValue: collapsed.innerRadiusValue,
            outerRadiusValue: collapsed.outerRadiusValue,
            radiusAxisMaxValue: collapsed.radiusAxisMaxValue,
            innerRadiusRatio: collapsed.innerRadiusRatio,
            outerRadiusRatio: collapsed.outerRadiusRatio,
            color: snapshot.color,
            opacity: 0,
            borderColor: snapshot.borderColor,
            borderWidth: snapshot.borderWidth,
            collapseMode,
            previous: snapshot,
            leaving: true
        });

        state.set(stateKey, {
            ...snapshot,
            visible: false,
            ghostEmitted: true
        });
    }

    return frames;
}

function getFrameBySectorRenderParams(
    frames: HistoricalSectorAnimationFrame[],
    params: HistoricalSectorRenderParams
): HistoricalSectorAnimationFrame | null {
    return frames[params.dataIndex]
        ?? frames[params.dataIndexInside ?? -1]
        ?? null;
}

function createEmptyHistoricalSectorElement(id: string): Record<string, unknown> {
    return {
        type: 'sector',
        id,
        name: id,
        silent: true,
        shape: {
            cx: 0,
            cy: 0,
            r0: 0,
            r: 0,
            startAngle: 0,
            endAngle: 0,
            clockwise: true
        },
        style: {
            fill: 'transparent',
            opacity: 0
        }
    };
}

function getHistoricalSectorShape(
    frame: HistoricalSectorAnimationFrameInput | HistoricalSectorAnimationSnapshot,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): HistoricalSectorShape | null {
    const center = getSectorCenter(params);
    if (!center) {
        return null;
    }

    const radiusAxisMaxValue = Math.max(1, toNonNegativeFiniteNumber(frame.radiusAxisMaxValue));
    const minRadius = getPolarRadiusPx(api, center, 0, frame.startAngleValue);
    const maxRadius = getPolarRadiusPx(api, center, radiusAxisMaxValue, frame.startAngleValue);
    const startAngle = getPolarCanvasAngle(api, center, radiusAxisMaxValue, frame.startAngleValue);
    if (minRadius === null || maxRadius === null || startAngle === null) {
        return null;
    }
    const innerRadiusRatio = clamp(toFiniteNumber(frame.innerRadiusRatio ?? 0), 0, 1);
    const outerRadiusRatio = Math.max(
        innerRadiusRatio,
        clamp(toFiniteNumber(frame.outerRadiusRatio ?? 1), 0, 1)
    );
    const radiusSpan = Math.max(0, maxRadius - minRadius);
    const innerRadius = minRadius + (radiusSpan * innerRadiusRatio);
    const outerRadius = minRadius + (radiusSpan * outerRadiusRatio);

    return {
        cx: center[0],
        cy: center[1],
        r0: Math.min(innerRadius, outerRadius),
        r: Math.max(innerRadius, outerRadius),
        startAngle,
        endAngle: startAngle + degreesToRadians(getClockwiseAngleSpan(frame.startAngleValue, frame.endAngleValue)),
        clockwise: true
    };
}

function interpolateHistoricalSectorSnapshot(
    start: HistoricalSectorAnimationSnapshot,
    end: HistoricalSectorAnimationFrame,
    progress: number
): HistoricalSectorAnimationSnapshot {
    return {
        kind: 'sector',
        startAngleValue: interpolateHistoricalPolarAngle(start.startAngleValue, end.startAngleValue, progress),
        endAngleValue: interpolateHistoricalPolarAngle(start.endAngleValue, end.endAngleValue, progress),
        innerRadiusValue: interpolateNumber(start.innerRadiusValue, end.innerRadiusValue, progress),
        outerRadiusValue: interpolateNumber(start.outerRadiusValue, end.outerRadiusValue, progress),
        radiusAxisMaxValue: end.radiusAxisMaxValue,
        innerRadiusRatio: interpolateNumber(start.innerRadiusRatio, end.innerRadiusRatio ?? 0, progress),
        outerRadiusRatio: interpolateNumber(start.outerRadiusRatio, end.outerRadiusRatio ?? 1, progress),
        dataId: end.dataId,
        name: end.name,
        color: end.color,
        opacity: interpolateNumber(start.opacity ?? 1, end.opacity, progress),
        borderColor: end.borderColor,
        borderWidth: end.borderWidth,
        collapseMode: end.collapseMode,
        visible: end.opacity > 0
    };
}

function unwrapHistoricalSectorShape(
    shape: HistoricalSectorShape,
    previousShape: HistoricalSectorShape | null
): HistoricalSectorShape {
    if (!previousShape) {
        return shape;
    }

    const span = Math.max(0.001, shape.endAngle - shape.startAngle);
    const startAngle = resolveNearestCircularRadian(shape.startAngle, previousShape.startAngle);

    return {
        ...shape,
        startAngle,
        endAngle: startAngle + span
    };
}

function buildHistoricalSectorKeyframes(
    frame: HistoricalSectorAnimationFrame,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): HistoricalSectorKeyframe[] | undefined {
    const startFrame = frame.previous ?? getSectorCollapsedSnapshot(frame);
    let previousShape: HistoricalSectorShape | null = null;

    const keyframes = [0, 0.25, 0.5, 0.75, 1].map(percent => {
        const snapshot = interpolateHistoricalSectorSnapshot(startFrame, frame, percent);
        const rawShape = getHistoricalSectorShape(snapshot, params, api);
        if (!rawShape) {
            return null;
        }
        const shape = unwrapHistoricalSectorShape(rawShape, previousShape);
        previousShape = shape;

        return {
            percent,
            shape,
            style: {
                opacity: snapshot.opacity ?? frame.opacity
            }
        };
    }).filter((item): item is HistoricalSectorKeyframe => item !== null);

    return keyframes.length ? keyframes : undefined;
}

function createHistoricalSectorRenderItem(
    frames: HistoricalSectorAnimationFrame[]
): (params: HistoricalSectorRenderParams, api: HistoricalSectorRenderApi) => Record<string, unknown> {
    return (params, api) => {
        const frame = getFrameBySectorRenderParams(frames, params);
        if (!frame) {
            return createEmptyHistoricalSectorElement(`historical-sector-empty-${params.dataIndex ?? 0}`);
        }

        const shape = getHistoricalSectorShape(frame, params, api);
        if (!shape) {
            return createEmptyHistoricalSectorElement(frame.dataId);
        }

        const keyframes = buildHistoricalSectorKeyframes(frame, params, api);
        const displayShape = keyframes?.[keyframes.length - 1]?.shape ?? shape;

        return {
            type: 'sector',
            id: frame.dataId,
            name: frame.dataId,
            silent: true,
            shape: displayShape,
            style: {
                fill: frame.color,
                opacity: frame.opacity,
                stroke: frame.borderColor,
                lineWidth: frame.borderWidth
            },
            enterFrom: {
                style: {
                    opacity: 0
                }
            },
            enterAnimation: {
                duration: 360,
                easing: 'cubicOut'
            },
            updateAnimation: {
                duration: HISTORY_ANIMATION_DURATION,
                easing: HISTORY_ANIMATION_EASING
            },
            keyframeAnimation: keyframes
                ? {
                    duration: HISTORY_ANIMATION_DURATION,
                    easing: HISTORY_ANIMATION_EASING,
                    keyframes
                }
                : undefined
        };
    };
}

export function buildHistoricalSectorCustomSeries(
    id: string,
    name: string,
    polarIndex: number,
    z: number,
    frames: HistoricalSectorAnimationFrame[]
): Record<string, unknown> {
    return {
        id,
        name,
        type: 'custom',
        coordinateSystem: 'polar',
        polarIndex,
        z,
        silent: true,
        clip: false,
        tooltip: { show: false },
        animation: true,
        animationTypeUpdate: 'transition',
        animationDuration: HISTORY_ANIMATION_DURATION,
        animationDurationUpdate: HISTORY_ANIMATION_DURATION,
        animationEasing: 'cubicOut',
        animationEasingUpdate: HISTORY_ANIMATION_EASING,
        animationDelay: 0,
        animationDelayUpdate: 0,
        dimensions: ['innerRadius', 'outerRadius', 'startAngle', 'endAngle'],
        encode: { radius: 1, angle: 2 },
        renderItem: createHistoricalSectorRenderItem(frames),
        data: frames.map(frame => ({
            id: frame.dataId,
            name: frame.name,
            value: [
                frame.innerRadiusValue,
                frame.outerRadiusValue,
                frame.startAngleValue,
                frame.endAngleValue
            ],
            itemStyle: {
                color: frame.color,
                opacity: frame.opacity,
                borderColor: frame.borderColor,
                borderWidth: frame.borderWidth
            }
        }))
    };
}
