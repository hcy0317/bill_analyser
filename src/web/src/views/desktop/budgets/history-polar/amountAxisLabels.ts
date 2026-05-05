import type {
    HistoricalAmountAxisLabelKeyframe,
    HistoricalGridLineAnimationFrame,
    HistoricalSectorRenderApi,
    HistoricalSectorRenderParams
} from './types.ts';
import {
    AMOUNT_AXIS_LABEL_OFFSET_Y,
    HISTORICAL_LABEL_Z2,
    HISTORY_ANIMATION_DURATION,
    HISTORY_ANIMATION_EASING,
    HISTORY_LABEL_FADE_IN_START_PERCENT,
    HISTORY_LABEL_FADE_OUT_PERCENT
} from './constants.ts';
import { interpolateNumber } from './math.ts';
import {
    getHistoricalAmountAxisTransitionEdgeRatio,
    isHistoricalAmountAxisRenderRestart,
    resolveHistoricalAmountAxisStartRadiusRatio
} from './amountAxis.ts';
import {
    getFrameByGridLineRenderParams,
    getHistoricalGridLineShape
} from './amountAxisRenderUtils.ts';

function getHistoricalAmountAxisLabelPosition(
    frame: Pick<HistoricalGridLineAnimationFrame, 'radiusAxisMaxValue' | 'radiusRatio'>,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): { x: number; y: number } | null {
    const shape = getHistoricalGridLineShape(frame, params, api);
    if (!shape) {
        return null;
    }

    return {
        x: shape.cx,
        y: shape.cy - shape.r - AMOUNT_AXIS_LABEL_OFFSET_Y
    };
}

function buildHistoricalAmountAxisLabelKeyframes(
    frame: HistoricalGridLineAnimationFrame,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): HistoricalAmountAxisLabelKeyframe[] | undefined {
    const startRadiusRatio = resolveHistoricalAmountAxisStartRadiusRatio(frame);
    const startOpacity = frame.previous ? (frame.previous.visible === false ? 0 : 1) : 0;

    if (frame.labelGhost) {
        const startPosition = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: startRadiusRatio
            },
            params,
            api
        );
        if (!startPosition) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: startOpacity }
            },
            {
                percent: HISTORY_LABEL_FADE_OUT_PERCENT,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: 0 }
            }
        ];
    }

    if (frame.leaving) {
        const endRadiusRatio = getHistoricalAmountAxisTransitionEdgeRatio(startRadiusRatio);
        const startPosition = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: startRadiusRatio
            },
            params,
            api
        );
        if (!startPosition) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: startOpacity }
            },
            {
                percent: 0.62,
                ...(
                    getHistoricalAmountAxisLabelPosition(
                        {
                            radiusAxisMaxValue: frame.radiusAxisMaxValue,
                            radiusRatio: interpolateNumber(startRadiusRatio, endRadiusRatio, 0.62)
                        },
                        params,
                        api
                    ) ?? startPosition
                ),
                style: { opacity: Math.max(0, startOpacity * 0.35) }
            },
            {
                percent: 1,
                ...(
                    getHistoricalAmountAxisLabelPosition(
                        {
                            radiusAxisMaxValue: frame.radiusAxisMaxValue,
                            radiusRatio: endRadiusRatio
                        },
                        params,
                        api
                    ) ?? startPosition
                ),
                style: { opacity: 0 }
            }
        ];
    }

    if (frame.rangeChanged) {
        const startPosition = frame.previous
            ? getHistoricalAmountAxisLabelPosition(
                {
                    radiusAxisMaxValue: frame.radiusAxisMaxValue,
                    radiusRatio: startRadiusRatio
                },
                params,
                api
            )
            : null;
        const endPosition = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: frame.radiusRatio
            },
            params,
            api
        );
        if (!endPosition) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startPosition?.x ?? endPosition.x,
                y: startPosition?.y ?? endPosition.y,
                style: { opacity: 0 }
            },
            {
                percent: HISTORY_LABEL_FADE_IN_START_PERCENT,
                x: endPosition.x,
                y: endPosition.y,
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: endPosition.x,
                y: endPosition.y,
                style: { opacity: frame.opacity }
            }
        ];
    }

    if (!frame.previous || isHistoricalAmountAxisRenderRestart(frame)) {
        const appearStartRatio = getHistoricalAmountAxisTransitionEdgeRatio(frame.radiusRatio);
        const startPosition = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: appearStartRatio
            },
            params,
            api
        );
        const endPosition = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: frame.radiusRatio
            },
            params,
            api
        );
        if (!startPosition || !endPosition) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: 0 }
            },
            {
                percent: HISTORY_LABEL_FADE_IN_START_PERCENT,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: endPosition.x,
                y: endPosition.y,
                style: { opacity: frame.opacity }
            }
        ];
    }

    const keyframes = [0, 0.25, 0.5, 0.75, 1].map(percent => {
        const position = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: interpolateNumber(startRadiusRatio, frame.radiusRatio, percent)
            },
            params,
            api
        );
        if (!position) {
            return null;
        }

        return {
            percent,
            x: position.x,
            y: position.y,
            style: {
                opacity: interpolateNumber(startOpacity, frame.opacity, percent)
            }
        };
    }).filter((item): item is HistoricalAmountAxisLabelKeyframe => item !== null);

    return keyframes.length ? keyframes : undefined;
}

function createEmptyHistoricalAmountAxisLabelElement(id: string): Record<string, unknown> {
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

function createHistoricalAmountAxisLabelRenderItem(
    frames: HistoricalGridLineAnimationFrame[],
    formatAmount: (amount: number) => string,
    labelColor: string
): (params: HistoricalSectorRenderParams, api: HistoricalSectorRenderApi) => Record<string, unknown> {
    return (params, api) => {
        const frame = getFrameByGridLineRenderParams(frames, params);
        if (!frame) {
            return createEmptyHistoricalAmountAxisLabelElement(`historical-amount-label-empty-${params.dataIndex ?? 0}`);
        }

        const position = getHistoricalAmountAxisLabelPosition(frame, params, api);
        if (!position) {
            return createEmptyHistoricalAmountAxisLabelElement(frame.dataId);
        }

        const keyframes = buildHistoricalAmountAxisLabelKeyframes(frame, params, api);
        const isNewRenderElement = !frame.previous || frame.previous.dataId !== frame.dataId;
        const displayKeyframe = isNewRenderElement
            ? keyframes?.[0]
            : keyframes?.[keyframes.length - 1];
        const displayOpacity = displayKeyframe?.style?.opacity ?? frame.opacity;

        return {
            type: 'text',
            id: frame.dataId,
            name: frame.dataId,
            z2: HISTORICAL_LABEL_Z2 - 1,
            x: displayKeyframe?.x ?? position.x,
            y: displayKeyframe?.y ?? position.y,
            rotation: 0,
            silent: true,
            transition: ['x', 'y'],
            updateAnimation: {
                duration: HISTORY_ANIMATION_DURATION,
                easing: HISTORY_ANIMATION_EASING
            },
            style: {
                text: formatAmount(frame.amountValue),
                fill: labelColor,
                fontSize: 10,
                fontWeight: 700,
                align: 'center',
                verticalAlign: 'bottom',
                opacity: displayOpacity
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

export function buildHistoricalAmountAxisLabelCustomSeries(
    frames: HistoricalGridLineAnimationFrame[],
    formatAmount: (amount: number) => string,
    labelColor: string
): Record<string, unknown> {
    return {
        id: 'amount-axis-labels',
        name: 'amount-axis-labels',
        type: 'custom',
        coordinateSystem: 'polar',
        polarIndex: 0,
        z: 1,
        silent: true,
        clip: false,
        tooltip: { show: false },
        animation: true,
        animationDuration: HISTORY_ANIMATION_DURATION,
        animationDurationUpdate: HISTORY_ANIMATION_DURATION,
        animationEasing: 'cubicOut',
        animationEasingUpdate: HISTORY_ANIMATION_EASING,
        dimensions: ['radius', 'angle'],
        encode: { radius: 0, angle: 1 },
        renderItem: createHistoricalAmountAxisLabelRenderItem(frames, formatAmount, labelColor),
        data: frames.map(frame => ({
            id: frame.dataId,
            name: frame.name,
            value: [frame.amountValue, 0],
            label: {
                formatter: formatAmount(frame.amountValue),
                rotate: 0
            },
            itemStyle: {
                color: labelColor,
                opacity: frame.opacity
            }
        }))
    };
}
