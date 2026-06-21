import type {
    HistoricalLabelRenderApi,
    HistoricalSectorRenderApi,
    HistoricalSectorRenderParams
} from './types.ts';

/**
 * 安全读取 ECharts 自定义系列坐标，坐标非法时返回 null 中止渲染。
 */
export function getSafeRenderCoord(api: HistoricalLabelRenderApi, value: number[]): [number, number] | null {
    try {
        const [x = 0, y = 0] = api.coord(value);
        if (!Number.isFinite(x) || !Number.isFinite(y)) {
            return null;
        }

        return [x, y];
    } catch {
        return null;
    }
}

/**
 * 读取扇区渲染中心点，兼容自定义系列坐标返回异常的情况。
 */
export function getSectorCenter(params: HistoricalSectorRenderParams): [number, number] | null {
    const cx = Number(params.coordSys?.cx);
    const cy = Number(params.coordSys?.cy);

    if (!Number.isFinite(cx) || !Number.isFinite(cy)) {
        return null;
    }

    return [cx, cy];
}

/**
 * 按半径比例计算极坐标像素半径。
 */
export function getPolarRadiusPx(
    api: HistoricalSectorRenderApi,
    center: [number, number],
    radiusValue: number,
    angleValue: number
): number | null {
    const point = getSafeRenderCoord(api, [radiusValue, angleValue]);
    if (!point) {
        return null;
    }

    return Math.hypot(point[0] - center[0], point[1] - center[1]);
}

/**
 * 将极坐标角度轴值转换为画布渲染使用的弧度角。
 */
export function getPolarCanvasAngle(
    api: HistoricalSectorRenderApi,
    center: [number, number],
    radiusValue: number,
    angleValue: number
): number | null {
    const point = getSafeRenderCoord(api, [radiusValue, angleValue]);
    if (!point) {
        return null;
    }

    return Math.atan2(point[1] - center[1], point[0] - center[0]);
}
