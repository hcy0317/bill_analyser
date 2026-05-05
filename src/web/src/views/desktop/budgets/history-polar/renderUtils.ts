import type {
    HistoricalLabelRenderApi,
    HistoricalSectorRenderApi,
    HistoricalSectorRenderParams
} from './types.ts';

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

export function getSectorCenter(params: HistoricalSectorRenderParams): [number, number] | null {
    const cx = Number(params.coordSys?.cx);
    const cy = Number(params.coordSys?.cy);

    if (!Number.isFinite(cx) || !Number.isFinite(cy)) {
        return null;
    }

    return [cx, cy];
}

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
