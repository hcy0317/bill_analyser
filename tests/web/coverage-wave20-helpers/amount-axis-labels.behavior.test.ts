import { describe, expect, test } from '@jest/globals';

import { buildHistoricalAmountAxisLabelCustomSeries } from '@/views/desktop/budgets/history-polar/amountAxisLabels.ts';
import type {
    HistoricalGridLineAnimationFrame,
    HistoricalGridLineAnimationSnapshot,
    HistoricalSectorRenderApi,
    HistoricalSectorRenderParams
} from '@/views/desktop/budgets/history-polar/types.ts';

function previous(overrides: Partial<HistoricalGridLineAnimationSnapshot> = {}): HistoricalGridLineAnimationSnapshot {
    return {
        kind: 'grid-line',
        axisIndex: 1,
        amountValue: 50,
        radiusAxisMaxValue: 100,
        radiusRatio: 0.5,
        dataId: 'amount-1',
        name: '50',
        color: '#999',
        visible: true,
        ...overrides
    };
}

function frame(overrides: Partial<HistoricalGridLineAnimationFrame> = {}): HistoricalGridLineAnimationFrame {
    return {
        stateKey: 'amount:1',
        dataId: 'amount-1',
        name: '50',
        axisIndex: 1,
        amountValue: 50,
        radiusAxisMaxValue: 100,
        radiusRatio: 0.5,
        color: '#999',
        opacity: 1,
        previous: null,
        leaving: false,
        labelGhost: false,
        rangeChanged: false,
        ...overrides
    };
}

function renderItem(frames: HistoricalGridLineAnimationFrame[]) {
    return buildHistoricalAmountAxisLabelCustomSeries(frames, amount => `¥${amount}`, '#333')['renderItem'] as (
        params: HistoricalSectorRenderParams,
        api: HistoricalSectorRenderApi
    ) => Record<string, unknown>;
}

function radialCoord([radius]: number[]): number[] {
    return [100 + Number(radius), 100];
}

const PARAMS: HistoricalSectorRenderParams = {
    dataIndex: 0,
    coordSys: { cx: 100, cy: 100 }
};

describe('historical amount-axis label rendering boundaries', () => {
    test('renders a new label from the transition edge and formats the amount', () => {
        const result = renderItem([frame()])(PARAMS, { coord: radialCoord });
        const animation = result['keyframeAnimation'] as {
            keyframes: Array<{ percent: number; style: { opacity: number } }>;
        };

        expect(result).toMatchObject({
            type: 'text',
            id: 'amount-1',
            style: expect.objectContaining({ text: '¥50', fill: '#333', opacity: 0 })
        });
        expect(animation.keyframes.map(keyframe => keyframe.percent)).toEqual([0, 0.68, 1]);
        expect(animation.keyframes.at(-1)?.style.opacity).toBe(1);
    });

    test('renders label ghosts from hidden and visible previous snapshots', () => {
        const hiddenGhost = frame({
            labelGhost: true,
            previous: previous({ visible: false })
        });
        const visibleGhost = frame({
            dataId: 'amount-2',
            labelGhost: true,
            previous: previous({ dataId: 'amount-2', visible: true })
        });
        const hiddenResult = renderItem([hiddenGhost])(PARAMS, { coord: radialCoord });
        const visibleResult = renderItem([visibleGhost])(PARAMS, { coord: radialCoord });
        const hiddenFrames = (hiddenResult['keyframeAnimation'] as { keyframes: Array<{ style: { opacity: number } }> }).keyframes;
        const visibleFrames = (visibleResult['keyframeAnimation'] as { keyframes: Array<{ style: { opacity: number } }> }).keyframes;

        expect(hiddenFrames.map(item => item.style.opacity)).toEqual([0, 0, 0]);
        expect(visibleFrames.map(item => item.style.opacity)).toEqual([1, 0, 0]);
    });

    test('renders leaving and range-change paths with explicit end-state assertions', () => {
        const leavingResult = renderItem([frame({
            leaving: true,
            opacity: 0,
            previous: previous()
        })])(PARAMS, { coord: radialCoord });
        const rangeResult = renderItem([frame({
            radiusRatio: 0.8,
            rangeChanged: true,
            previous: previous({ radiusRatio: 0.25 })
        })])(PARAMS, { coord: radialCoord });
        const leavingFrames = (leavingResult['keyframeAnimation'] as {
            keyframes: Array<{ x: number; y: number; style: { opacity: number } }>;
        }).keyframes;
        const rangeFrames = (rangeResult['keyframeAnimation'] as {
            keyframes: Array<{ x: number; y: number; style: { opacity: number } }>;
        }).keyframes;

        expect(leavingFrames).toHaveLength(3);
        expect(leavingFrames.at(-1)?.style.opacity).toBe(0);
        expect(leavingFrames.at(-1)?.y).toBeLessThan(leavingFrames[0]!.y);
        expect(rangeFrames[0]?.style.opacity).toBe(0);
        expect(rangeFrames.at(-1)?.style.opacity).toBe(1);
        expect(rangeFrames.at(-1)?.y).toBeLessThan(rangeFrames[0]!.y);
    });

    test('uses the general interpolation path for an existing stable data id', () => {
        const result = renderItem([frame({
            radiusRatio: 0.75,
            opacity: 0.4,
            previous: previous({ radiusRatio: 0.25, dataId: 'amount-1' })
        })])(PARAMS, { coord: radialCoord });
        const animation = result['keyframeAnimation'] as {
            keyframes: Array<{ percent: number; x: number; y: number; style: { opacity: number } }>;
        };

        expect(animation.keyframes.map(item => item.percent)).toEqual([0, 0.25, 0.5, 0.75, 1]);
        expect(animation.keyframes[0]?.y).toBeGreaterThan(animation.keyframes.at(-1)!.y);
        expect(result).toMatchObject({
            x: animation.keyframes.at(-1)?.x,
            style: expect.objectContaining({ opacity: 0.4 })
        });
    });

    test('returns empty labels for missing frames, centers and invalid coordinates', () => {
        const render = renderItem([frame()]);

        expect(render(
            { dataIndex: undefined as unknown as number, coordSys: { cx: 100, cy: 100 } },
            { coord: radialCoord }
        )).toMatchObject({
            id: 'historical-amount-label-empty-0',
            opacity: 0,
            style: expect.objectContaining({ text: '' })
        });
        expect(render({ dataIndex: 0 }, { coord: radialCoord })).toMatchObject({
            id: 'amount-1',
            opacity: 0
        });
        expect(render(PARAMS, { coord: () => [Number.NaN, 100] })).toMatchObject({
            id: 'amount-1',
            opacity: 0
        });
    });

    test.each([
        ['ghost', frame({ labelGhost: true, previous: previous() }), 2],
        ['leaving', frame({ leaving: true, previous: previous() }), 2],
        ['restart', frame({ previous: previous({ dataId: 'old-id' }) }), 2]
    ])('drops %s keyframes when their start position cannot be projected', (_name, targetFrame, validCalls) => {
        let calls = 0;
        const result = renderItem([targetFrame])(PARAMS, {
            coord: value => {
                calls += 1;
                return calls <= validCalls ? radialCoord(value) : [Number.NaN, Number.NaN];
            }
        });

        expect(result).toMatchObject({
            id: targetFrame.dataId,
            keyframeAnimation: undefined,
            style: expect.objectContaining({ text: `¥${targetFrame.amountValue}` })
        });
    });

    test('falls back to the leaving start position when intermediate projections fail', () => {
        let calls = 0;
        const result = renderItem([frame({
            leaving: true,
            previous: previous()
        })])(PARAMS, {
            coord: value => {
                calls += 1;
                return calls <= 4 ? radialCoord(value) : [Number.NaN, Number.NaN];
            }
        });
        const keyframes = (result['keyframeAnimation'] as {
            keyframes: Array<{ x: number; y: number }>;
        }).keyframes;

        expect(keyframes).toHaveLength(3);
        expect(keyframes[1]).toMatchObject({ x: keyframes[0]?.x, y: keyframes[0]?.y });
        expect(keyframes[2]).toMatchObject({ x: keyframes[0]?.x, y: keyframes[0]?.y });
    });

    test('rejects a missing range-change end position and filters all general keyframes', () => {
        let rangeCalls = 0;
        const rangeResult = renderItem([frame({
            rangeChanged: true,
            previous: previous()
        })])(PARAMS, {
            coord: value => {
                rangeCalls += 1;
                return rangeCalls <= 4 ? radialCoord(value) : [Number.NaN, Number.NaN];
            }
        });

        let generalCalls = 0;
        const generalResult = renderItem([frame({
            previous: previous({ dataId: 'amount-1' })
        })])(PARAMS, {
            coord: value => {
                generalCalls += 1;
                return generalCalls <= 2 ? radialCoord(value) : [Number.NaN, Number.NaN];
            }
        });

        expect(rangeResult['keyframeAnimation']).toBeUndefined();
        expect(generalResult['keyframeAnimation']).toBeUndefined();
    });
});
