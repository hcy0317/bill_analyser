import type {
    HistoricalGridLineAnimationFrame,
    HistoricalGridLineShape,
    HistoricalSectorRenderApi,
    HistoricalSectorRenderParams
} from './types.ts';
import { AMOUNT_AXIS_MAX_RENDER_RADIUS_RATIO } from './constants.ts';
import {
    clamp,
    toFiniteNumber,
    toNonNegativeFiniteNumber
} from './math.ts';
import {
    getPolarRadiusPx,
    getSectorCenter
} from './renderUtils.ts';

/**
 * 从 ECharts 自定义系列 render 参数中取回金额轴动画帧。
 */
export function getFrameByGridLineRenderParams(
    frames: HistoricalGridLineAnimationFrame[],
    params: HistoricalSectorRenderParams
): HistoricalGridLineAnimationFrame | null {
    return frames[params.dataIndex]
        ?? frames[params.dataIndexInside ?? -1]
        ?? null;
}

/**
 * 根据金额轴动画帧和当前进度计算网格线圆环形状。
 */
export function getHistoricalGridLineShape(
    frame: Pick<HistoricalGridLineAnimationFrame, 'radiusAxisMaxValue' | 'radiusRatio'>,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): HistoricalGridLineShape | null {
    const center = getSectorCenter(params);
    if (!center) {
        return null;
    }

    const radiusAxisMaxValue = Math.max(1, toNonNegativeFiniteNumber(frame.radiusAxisMaxValue));
    const minRadius = getPolarRadiusPx(api, center, 0, 0);
    const maxRadius = getPolarRadiusPx(api, center, radiusAxisMaxValue, 0);
    if (minRadius === null || maxRadius === null) {
        return null;
    }

    const radiusSpan = Math.max(0, maxRadius - minRadius);

    return {
        cx: center[0],
        cy: center[1],
        r: minRadius + (
            radiusSpan * clamp(toFiniteNumber(frame.radiusRatio, 0), 0, AMOUNT_AXIS_MAX_RENDER_RADIUS_RATIO)
        )
    };
}
