import type {
    HistoricalLabelAnimationFrame,
    HistoricalLabelAnimationFrameInput,
    HistoricalLabelAnimationState,
    HistoricalLabelKeyframe,
    HistoricalLabelRenderApi,
    HistoricalLabelRenderParams
} from './types.ts';
import {
    HISTORICAL_LABEL_SERIES_Z,
    HISTORICAL_LABEL_Z2,
    HISTORY_ANIMATION_DURATION,
    HISTORY_ANIMATION_EASING,
    HISTORY_LABEL_FADE_IN_START_PERCENT,
    HISTORY_LABEL_FADE_OUT_PERCENT,
    LABEL_ROTATION_CROSS_FADE_THRESHOLD
} from './constants.ts';
import {
    clamp,
    degreesToRadians,
    interpolateNumber,
    normalizeCircleAngle,
    normalizeRotation,
    toFiniteNumber,
    toNonNegativeFiniteNumber
} from './math.ts';
import {
    getHistoricalLabelFacingBucket,
    interpolateHistoricalPolarAngle,
    resolveNearestCircularAngle
} from './geometry.ts';
import { getSafeRenderCoord } from './renderUtils.ts';
import { isHistoricalLabelSnapshot } from './state.ts';

export function resolveHistoricalLabelAnimationFrames(
    inputs: HistoricalLabelAnimationFrameInput[],
    state?: HistoricalLabelAnimationState,
    stateKeyPrefix?: string
): HistoricalLabelAnimationFrame[] {
    const visibleKeys = new Set(inputs.map(input => input.stateKey));
    const currentRadiusAxisMaxValue = inputs.reduce(
        (maxValue, input) => Math.max(
            maxValue,
            toNonNegativeFiniteNumber(input.radiusAxisMaxValue ?? input.radiusValue)
        ),
        0
    );
    const frames = inputs.map(input => {
        const previousSnapshot = state?.get(input.stateKey);
        const previous = isHistoricalLabelSnapshot(previousSnapshot) ? previousSnapshot : null;
        const radiusValue = toNonNegativeFiniteNumber(input.radiusValue);
        const radiusAxisMaxValue = Math.max(
            1,
            toNonNegativeFiniteNumber(input.radiusAxisMaxValue ?? radiusValue)
        );
        const radiusRatio = clamp(
            toFiniteNumber(input.radiusRatio ?? (radiusValue / radiusAxisMaxValue)),
            0,
            1
        );
        const inputPolarAngle = normalizeCircleAngle(input.polarAngleValue);
        const inputRotate = toFiniteNumber(input.rotate);
        const facingBucket = getHistoricalLabelFacingBucket(inputPolarAngle);
        const previousFacingBucket = previous?.facingBucket
            ?? (previous ? getHistoricalLabelFacingBucket(previous.polarAngleValue) : facingBucket);
        const polarAngleValue = previous
            ? resolveNearestCircularAngle(inputPolarAngle, previous.polarAngleValue)
            : inputPolarAngle;
        const rotate = previous
            ? resolveNearestCircularAngle(inputRotate, previous.rotate)
            : inputRotate;
        const crossFadeOnly = previous
            ? previousFacingBucket !== facingBucket
                || Math.abs(normalizeRotation(inputRotate - normalizeRotation(previous.rotate))) > LABEL_ROTATION_CROSS_FADE_THRESHOLD
            : false;
        const frame: HistoricalLabelAnimationFrame = {
            ...input,
            radiusValue,
            radiusAxisMaxValue,
            radiusRatio,
            polarAngleValue,
            rotate,
            previous,
            opacity: 1,
            scale: 1,
            leaving: false,
            crossFadeOnly,
            facingBucket
        };

        state?.set(input.stateKey, {
            radiusValue: frame.radiusValue,
            radiusAxisMaxValue: frame.radiusAxisMaxValue,
            radiusRatio: frame.radiusRatio,
            polarAngleValue: frame.polarAngleValue,
            rotate: frame.rotate,
            facingBucket: frame.facingBucket,
            dataId: frame.dataId,
            name: frame.name,
            text: frame.text,
            color: frame.color,
            fontSize: frame.fontSize,
            fontWeight: frame.fontWeight,
            width: frame.width,
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

        if (visibleKeys.has(stateKey) || (snapshot.visible === false && snapshot.ghostEmitted)) {
            continue;
        }

        if (
            !isHistoricalLabelSnapshot(snapshot)
            || !snapshot.dataId
            || !snapshot.name
            || !snapshot.text
            || !snapshot.color
            || typeof snapshot.fontSize !== 'number'
            || typeof snapshot.fontWeight !== 'number'
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
            text: snapshot.text,
            radiusValue: radiusAxisMaxValue * radiusRatio,
            radiusAxisMaxValue,
            radiusRatio,
            polarAngleValue: snapshot.polarAngleValue,
            rotate: snapshot.rotate,
            color: snapshot.color,
            fontSize: snapshot.fontSize,
            fontWeight: snapshot.fontWeight,
            width: snapshot.width,
            previous: snapshot,
            opacity: 0,
            scale: 1,
            leaving: true,
            crossFadeOnly: false,
            facingBucket: snapshot.facingBucket ?? getHistoricalLabelFacingBucket(snapshot.polarAngleValue)
        });

        state.set(stateKey, {
            ...snapshot,
            visible: false,
            ghostEmitted: true
        });
    }

    return frames;
}

function getFrameByRenderParams(
    frames: HistoricalLabelAnimationFrame[],
    params: HistoricalLabelRenderParams
): HistoricalLabelAnimationFrame | null {
    return frames[params.dataIndex]
        ?? frames[params.dataIndexInside ?? -1]
        ?? null;
}

function buildHistoricalLabelArcKeyframes(
    frame: HistoricalLabelAnimationFrame,
    api: HistoricalLabelRenderApi
): HistoricalLabelKeyframe[] | undefined {
    const startRadiusRatio = clamp(
        toFiniteNumber(
            frame.previous?.radiusRatio
                ?? ((frame.previous?.radiusValue ?? frame.radiusValue)
                    / Math.max(1, toNonNegativeFiniteNumber(frame.previous?.radiusAxisMaxValue ?? frame.radiusAxisMaxValue))),
            frame.radiusRatio
        ),
        0,
        1
    );
    const startRadiusValue = frame.previous
        ? frame.radiusAxisMaxValue * startRadiusRatio
        : frame.radiusValue;
    const startPolarAngleValue = frame.previous?.polarAngleValue ?? frame.polarAngleValue;
    const startRotate = frame.previous?.rotate ?? frame.rotate;
    const startOpacity = frame.previous ? (frame.previous.visible === false ? 0 : 1) : 0;

    if (frame.leaving && frame.previous) {
        const startCoord = getSafeRenderCoord(api, [startRadiusValue, startPolarAngleValue]);
        if (!startCoord) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startCoord[0],
                y: startCoord[1],
                rotation: degreesToRadians(startRotate),
                style: { opacity: startOpacity }
            },
            {
                percent: HISTORY_LABEL_FADE_OUT_PERCENT,
                x: startCoord[0],
                y: startCoord[1],
                rotation: degreesToRadians(startRotate),
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: startCoord[0],
                y: startCoord[1],
                rotation: degreesToRadians(startRotate),
                style: { opacity: 0 }
            }
        ];
    }

    if (frame.crossFadeOnly && frame.previous) {
        const startCoord = getSafeRenderCoord(api, [startRadiusValue, startPolarAngleValue]);
        const endCoord = getSafeRenderCoord(api, [frame.radiusValue, frame.polarAngleValue]);
        if (!startCoord || !endCoord) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startCoord[0],
                y: startCoord[1],
                rotation: degreesToRadians(startRotate),
                style: { opacity: startOpacity }
            },
            {
                percent: HISTORY_LABEL_FADE_OUT_PERCENT,
                x: startCoord[0],
                y: startCoord[1],
                rotation: degreesToRadians(startRotate),
                style: { opacity: 0 }
            },
            {
                percent: HISTORY_LABEL_FADE_IN_START_PERCENT,
                x: endCoord[0],
                y: endCoord[1],
                rotation: degreesToRadians(frame.rotate),
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: endCoord[0],
                y: endCoord[1],
                rotation: degreesToRadians(frame.rotate),
                style: { opacity: frame.opacity }
            }
        ];
    }

    const startCoord = getSafeRenderCoord(api, [startRadiusValue, startPolarAngleValue]);
    const endCoord = getSafeRenderCoord(api, [frame.radiusValue, frame.polarAngleValue]);
    if (!startCoord || !endCoord) {
        return undefined;
    }

    if (!frame.previous) {
        return [
            {
                percent: 0,
                x: endCoord[0],
                y: endCoord[1],
                rotation: degreesToRadians(frame.rotate),
                style: { opacity: 0 }
            },
            {
                percent: HISTORY_LABEL_FADE_IN_START_PERCENT,
                x: endCoord[0],
                y: endCoord[1],
                rotation: degreesToRadians(frame.rotate),
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: endCoord[0],
                y: endCoord[1],
                rotation: degreesToRadians(frame.rotate),
                style: { opacity: frame.opacity }
            }
        ];
    }

    const motionKeyframes = [0.35, 0.7]
        .map(percent => {
            const radiusValue = interpolateNumber(startRadiusValue, frame.radiusValue, percent);
            const polarAngleValue = interpolateHistoricalPolarAngle(
                startPolarAngleValue,
                frame.polarAngleValue,
                percent
            );
            const rotate = interpolateHistoricalPolarAngle(startRotate, frame.rotate, percent);
            const coord = getSafeRenderCoord(api, [radiusValue, polarAngleValue]);

            if (!coord) {
                return null;
            }

            return {
                percent,
                x: coord[0],
                y: coord[1],
                rotation: degreesToRadians(rotate),
                style: {
                    opacity: interpolateNumber(startOpacity, frame.opacity, percent)
                }
            };
        })
        .filter((keyframe): keyframe is HistoricalLabelKeyframe => keyframe !== null);

    return [
        {
            percent: 0,
            x: startCoord[0],
            y: startCoord[1],
            rotation: degreesToRadians(startRotate),
            style: { opacity: startOpacity }
        },
        ...motionKeyframes,
        {
            percent: 1,
            x: endCoord[0],
            y: endCoord[1],
            rotation: degreesToRadians(frame.rotate),
            style: { opacity: frame.opacity }
        }
    ];
}

function createEmptyHistoricalLabelElement(id: string): Record<string, unknown> {
    return {
        type: 'text',
        id,
        name: id,
        z2: HISTORICAL_LABEL_Z2,
        x: 0,
        y: 0,
        opacity: 0,
        silent: true,
        style: {
            text: '',
            fill: 'transparent',
            fontSize: 1
        }
    };
}

function createHistoricalLabelRenderItem(
    frames: HistoricalLabelAnimationFrame[]
): (params: HistoricalLabelRenderParams, api: HistoricalLabelRenderApi) => Record<string, unknown> {
    return (params, api) => {
        const frame = getFrameByRenderParams(frames, params);
        if (!frame) {
            return createEmptyHistoricalLabelElement(`historical-label-empty-${params.dataIndex ?? 0}`);
        }

        const coord = getSafeRenderCoord(api, [frame.radiusValue, frame.polarAngleValue]);
        if (!coord) {
            return createEmptyHistoricalLabelElement(frame.dataId);
        }

        const [x, y] = coord;
        const keyframes = buildHistoricalLabelArcKeyframes(frame, api);
        const displayKeyframe = keyframes?.[keyframes.length - 1];
        const textAnimation = {
            duration: HISTORY_ANIMATION_DURATION,
            easing: HISTORY_ANIMATION_EASING
        };

        return {
            type: 'text',
            id: frame.dataId,
            name: frame.dataId,
            z2: HISTORICAL_LABEL_Z2,
            x: displayKeyframe?.x ?? x,
            y: displayKeyframe?.y ?? y,
            rotation: displayKeyframe?.rotation ?? degreesToRadians(frame.rotate),
            silent: true,
            transition: ['x', 'y'],
            enterFrom: {
                style: {
                    opacity: 0
                }
            },
            enterAnimation: {
                duration: 360,
                easing: 'cubicOut'
            },
            updateAnimation: textAnimation,
            style: {
                text: frame.text,
                fill: frame.color,
                fontSize: frame.fontSize,
                fontWeight: frame.fontWeight,
                width: frame.width,
                overflow: frame.width ? 'truncate' : undefined,
                align: 'center',
                verticalAlign: 'middle',
                opacity: frame.opacity
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

export function buildHistoricalLabelCustomSeries(
    name: string,
    polarIndex: number,
    z: number,
    frames: HistoricalLabelAnimationFrame[],
    id = name
): Record<string, unknown> {
    return {
        id,
        name,
        type: 'custom',
        coordinateSystem: 'polar',
        polarIndex,
        z: Math.max(z, HISTORICAL_LABEL_SERIES_Z),
        silent: true,
        clip: false,
        tooltip: { show: false },
        animation: true,
        animationDuration: 520,
        animationDurationUpdate: HISTORY_ANIMATION_DURATION,
        animationEasing: 'cubicOut',
        animationEasingUpdate: HISTORY_ANIMATION_EASING,
        dimensions: ['radius', 'angle', 'rotate'],
        encode: { radius: 0, angle: 1 },
        renderItem: createHistoricalLabelRenderItem(frames),
        data: frames.map(frame => ({
            id: frame.dataId,
            name: frame.name,
            value: [frame.radiusValue, frame.polarAngleValue, frame.rotate],
            label: {
                formatter: frame.text,
                rotate: frame.rotate,
                width: frame.width,
                overflow: frame.width ? 'truncate' : undefined
            }
        }))
    };
}
