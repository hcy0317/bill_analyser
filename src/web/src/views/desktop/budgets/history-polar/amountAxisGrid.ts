import type {
    HistoricalGridLineAnimationFrame,
    HistoricalGridLineKeyframe,
    HistoricalSectorRenderApi,
    HistoricalSectorRenderParams
} from './types.ts';
import {
    HISTORY_AMOUNT_AXIS_FADE_IN_START_PERCENT,
    HISTORY_ANIMATION_DURATION,
    HISTORY_ANIMATION_EASING
} from './constants.ts';
import { interpolateNumber } from './math.ts';
import {
    getHistoricalAmountAxisTransitionEdgeRatio,
    resolveHistoricalAmountAxisStartRadiusRatio
} from './amountAxis.ts';
import {
    getFrameByGridLineRenderParams,
    getHistoricalGridLineShape
} from './amountAxisRenderUtils.ts';

function buildHistoricalGridLineKeyframes(
    frame: HistoricalGridLineAnimationFrame,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): HistoricalGridLineKeyframe[] | undefined {
    const startRadiusRatio = resolveHistoricalAmountAxisStartRadiusRatio(frame);
    const startOpacity = frame.previous ? (frame.previous.visible === false ? 0 : 1) : 0;

    if (frame.leaving) {
        const endRadiusRatio = getHistoricalAmountAxisTransitionEdgeRatio(startRadiusRatio);
        const startShape = getHistoricalGridLineShape(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: startRadiusRatio
            },
            params,
            api
        );
        const endShape = getHistoricalGridLineShape(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: endRadiusRatio
            },
            params,
            api
        );
        if (!startShape || !endShape) {
            return undefined;
        }

        return [
            {
                percent: 0,
                shape: startShape,
                style: { opacity: startOpacity }
            },
            {
                percent: 0.62,
                shape: getHistoricalGridLineShape(
                    {
                        radiusAxisMaxValue: frame.radiusAxisMaxValue,
                        radiusRatio: interpolateNumber(startRadiusRatio, endRadiusRatio, 0.62)
                    },
                    params,
                    api
                ) ?? startShape,
                style: { opacity: Math.max(0, startOpacity * 0.35) }
            },
            {
                percent: 1,
                shape: endShape,
                style: { opacity: 0 }
            }
        ];
    }

    if (!frame.previous) {
        const appearStartRatio = getHistoricalAmountAxisTransitionEdgeRatio(frame.radiusRatio);
        const startShape = getHistoricalGridLineShape(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: appearStartRatio
            },
            params,
            api
        );
        const endShape = getHistoricalGridLineShape(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: frame.radiusRatio
            },
            params,
            api
        );
        if (!startShape || !endShape) {
            return undefined;
        }

        return [
            {
                percent: 0,
                shape: startShape,
                style: { opacity: 0 }
            },
            {
                percent: HISTORY_AMOUNT_AXIS_FADE_IN_START_PERCENT,
                shape: startShape,
                style: { opacity: 0 }
            },
            {
                percent: 1,
                shape: endShape,
                style: { opacity: frame.opacity }
            }
        ];
    }

    const keyframes = [0, 0.25, 0.5, 0.75, 1].map(percent => {
        const shape = getHistoricalGridLineShape(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: interpolateNumber(startRadiusRatio, frame.radiusRatio, percent)
            },
            params,
            api
        );
        if (!shape) {
            return null;
        }

        return {
            percent,
            shape,
            style: {
                opacity: interpolateNumber(startOpacity, frame.opacity, percent)
            }
        };
    }).filter((item): item is HistoricalGridLineKeyframe => item !== null);

    return keyframes.length ? keyframes : undefined;
}

function createEmptyHistoricalGridLineElement(id: string): Record<string, unknown> {
    return {
        type: 'circle',
        id,
        name: id,
        silent: true,
        shape: {
            cx: 0,
            cy: 0,
            r: 0
        },
        style: {
            fill: 'transparent',
            stroke: 'transparent',
            opacity: 0
        }
    };
}

function createHistoricalGridLineRenderItem(
    frames: HistoricalGridLineAnimationFrame[]
): (params: HistoricalSectorRenderParams, api: HistoricalSectorRenderApi) => Record<string, unknown> {
    return (params, api) => {
        const frame = getFrameByGridLineRenderParams(frames, params);
        if (!frame) {
            return createEmptyHistoricalGridLineElement(`historical-grid-empty-${params.dataIndex ?? 0}`);
        }

        const shape = getHistoricalGridLineShape(frame, params, api);
        if (!shape) {
            return createEmptyHistoricalGridLineElement(frame.dataId);
        }

        const keyframes = buildHistoricalGridLineKeyframes(frame, params, api);
        const isNewRenderElement = !frame.previous || frame.previous.dataId !== frame.dataId;
        const displayKeyframe = isNewRenderElement
            ? keyframes?.[0]
            : keyframes?.[keyframes.length - 1];
        const displayShape = displayKeyframe?.shape ?? shape;
        const displayOpacity = displayKeyframe?.style?.opacity ?? frame.opacity;

        return {
            type: 'circle',
            id: frame.dataId,
            name: frame.dataId,
            silent: true,
            shape: displayShape,
            style: {
                fill: 'transparent',
                stroke: frame.color,
                lineWidth: 1,
                lineDash: [4, 4],
                opacity: displayOpacity
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

/**
 * 构建金额轴网格线自定义系列，复用动画状态实现平滑更新。
 */
export function buildHistoricalGridLineCustomSeries(
    frames: HistoricalGridLineAnimationFrame[]
): Record<string, unknown> {
    return {
        id: 'amount-grid-lines',
        name: 'amount-grid-lines',
        type: 'custom',
        coordinateSystem: 'polar',
        polarIndex: 0,
        z: 0,
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
        renderItem: createHistoricalGridLineRenderItem(frames),
        data: frames.map(frame => ({
            id: frame.dataId,
            name: frame.name,
            value: [frame.amountValue, 0],
            itemStyle: {
                color: frame.color,
                opacity: frame.opacity
            }
        }))
    };
}
