import { describe, expect, test } from '@jest/globals';

import {
    buildHistoricalSectorCustomSeries,
    resolveHistoricalSectorAnimationFrames
} from '@/views/desktop/budgets/history-polar/sectorAnimation.ts';
import type {
    HistoricalLabelAnimationState,
    HistoricalSectorAnimationFrame,
    HistoricalSectorAnimationFrameInput,
    HistoricalSectorAnimationSnapshot,
    HistoricalSectorRenderApi,
    HistoricalSectorRenderParams
} from '@/views/desktop/budgets/history-polar/types.ts';

function input(overrides: Partial<HistoricalSectorAnimationFrameInput> = {}): HistoricalSectorAnimationFrameInput {
    return {
        stateKey: 'sector:one',
        dataId: 'sector-one',
        name: 'One',
        startAngleValue: 350,
        endAngleValue: 10,
        innerRadiusValue: 20,
        outerRadiusValue: 80,
        radiusAxisMaxValue: 100,
        color: '#f00',
        opacity: 1,
        collapseMode: 'angle',
        ...overrides
    };
}

function snapshot(overrides: Partial<HistoricalSectorAnimationSnapshot> = {}): HistoricalSectorAnimationSnapshot {
    return {
        kind: 'sector',
        startAngleValue: 10,
        endAngleValue: 40,
        innerRadiusValue: 10,
        outerRadiusValue: 60,
        radiusAxisMaxValue: 100,
        innerRadiusRatio: 0.1,
        outerRadiusRatio: 0.6,
        dataId: 'old-sector',
        name: 'Old sector',
        color: '#00f',
        opacity: 1,
        visible: true,
        ghostEmitted: false,
        ...overrides
    };
}

function renderItem(frames: HistoricalSectorAnimationFrame[]) {
    return buildHistoricalSectorCustomSeries('series', 'Series', 0, 1, frames)['renderItem'] as (
        params: HistoricalSectorRenderParams,
        api: HistoricalSectorRenderApi
    ) => Record<string, unknown>;
}

function polarCoord([radius, angle = 0]: number[]): number[] {
    const radians = Number(angle) * Math.PI / 180;
    return [
        100 + Number(radius) * Math.cos(radians),
        100 + Number(radius) * Math.sin(radians)
    ];
}

describe('historical sector animation boundaries', () => {
    test('derives missing radius ratios and normalizes wrapped or degenerate spans', () => {
        const [wrapped, degenerate] = resolveHistoricalSectorAnimationFrames([
            input(),
            input({
                stateKey: 'sector:two',
                dataId: 'sector-two',
                startAngleValue: 20,
                endAngleValue: 20,
                innerRadiusValue: -1,
                outerRadiusValue: Number.NaN,
                radiusAxisMaxValue: 0,
                opacity: Number.NaN
            })
        ]);

        expect(wrapped).toMatchObject({
            startAngleValue: 350,
            endAngleValue: 370,
            innerRadiusRatio: 0.2,
            outerRadiusRatio: 0.8
        });
        expect(degenerate).toMatchObject({
            endAngleValue: 20.1,
            innerRadiusValue: 0,
            outerRadiusValue: 0,
            radiusAxisMaxValue: 1,
            innerRadiusRatio: 0,
            outerRadiusRatio: 0,
            opacity: 1
        });
    });

    test('emits exactly one radius-collapse ghost while skipping every ineligible snapshot', () => {
        const state: HistoricalLabelAnimationState = new Map();
        state.set('other-prefix', snapshot({ dataId: 'foreign' }));
        state.set('sector:visible', snapshot({ dataId: 'visible' }));
        state.set('sector:wrong-kind', {
                kind: 'grid-line',
                amountValue: 1,
                radiusAxisMaxValue: 1,
                radiusRatio: 1,
                dataId: 'wrong',
                name: 'Wrong',
                color: '#000'
        });
        state.set('sector:already-ghosted', snapshot({ visible: false, ghostEmitted: true }));
        state.set('sector:no-id', snapshot({ dataId: undefined }));
        state.set('sector:no-name', snapshot({ name: undefined }));
        state.set('sector:no-color', snapshot({ color: undefined }));
        state.set('sector:leaving', snapshot({
                dataId: 'leaving',
                name: 'Leaving',
                color: '#abc',
                innerRadiusRatio: undefined as unknown as number,
                outerRadiusRatio: undefined as unknown as number,
                collapseMode: 'radius'
        }));

        const frames = resolveHistoricalSectorAnimationFrames([
            input({ stateKey: 'sector:visible', dataId: 'visible' })
        ], state, 'sector:');
        const leaving = frames.find(frame => frame.dataId === 'leaving');

        expect(frames.filter(frame => frame.leaving)).toHaveLength(1);
        expect(leaving).toMatchObject({
            leaving: true,
            startAngleValue: 10,
            endAngleValue: 40,
            innerRadiusRatio: 0,
            outerRadiusRatio: 0,
            opacity: 0
        });
        expect(state.get('sector:leaving')).toMatchObject({ visible: false, ghostEmitted: true });
    });

    test('defaults an absent snapshot collapse mode and retains its previous axis maximum', () => {
        const state: HistoricalLabelAnimationState = new Map([
            ['sector:old', snapshot({
                collapseMode: undefined,
                radiusAxisMaxValue: 240,
                outerRadiusRatio: undefined as unknown as number
            })]
        ]);

        const frames = resolveHistoricalSectorAnimationFrames([], state, 'sector:');

        expect(frames).toEqual([
            expect.objectContaining({
                collapseMode: 'angle',
                radiusAxisMaxValue: 240,
                outerRadiusRatio: 1,
                leaving: true
            })
        ]);
    });

    test('renders normal, dataIndexInside and missing-frame results with explicit assertions', () => {
        const frame = resolveHistoricalSectorAnimationFrames([input()])[0]!;
        const render = renderItem([frame]);
        const normal = render(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            { coord: polarCoord }
        );
        const byInsideIndex = render(
            { dataIndex: 99, dataIndexInside: 0, coordSys: { cx: 100, cy: 100 } },
            { coord: polarCoord }
        );
        const missing = render(
            { dataIndex: undefined as unknown as number, coordSys: { cx: 100, cy: 100 } },
            { coord: polarCoord }
        );

        expect(normal).toMatchObject({
            type: 'sector',
            id: 'sector-one',
            shape: expect.objectContaining({ cx: 100, cy: 100, clockwise: true }),
            keyframeAnimation: expect.objectContaining({
                keyframes: expect.arrayContaining([
                    expect.objectContaining({ percent: 0 }),
                    expect.objectContaining({ percent: 1 })
                ])
            })
        });
        expect(byInsideIndex).toMatchObject({ id: 'sector-one' });
        expect(missing).toMatchObject({
            id: 'historical-sector-empty-0',
            style: { fill: 'transparent', opacity: 0 }
        });
    });

    test('returns empty elements for invalid centers and coordinate failures', () => {
        const frame = resolveHistoricalSectorAnimationFrames([input()])[0]!;
        const render = renderItem([frame]);

        expect(render({ dataIndex: 0 }, { coord: polarCoord })).toMatchObject({
            id: 'sector-one',
            style: { fill: 'transparent', opacity: 0 }
        });
        expect(render(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            { coord: () => [Number.NaN, 100] }
        )).toMatchObject({
            id: 'sector-one',
            style: { fill: 'transparent', opacity: 0 }
        });
    });

    test('falls back to the final static shape when all generated keyframes are invalid', () => {
        const frame = resolveHistoricalSectorAnimationFrames([input()])[0]!;
        const render = renderItem([frame]);
        let calls = 0;
        const result = render(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            {
                coord: value => {
                    calls += 1;
                    return calls <= 3 ? polarCoord(value) : [Number.NaN, Number.NaN];
                }
            }
        );

        expect(result).toMatchObject({
            id: 'sector-one',
            shape: expect.objectContaining({ r0: expect.any(Number), r: expect.any(Number) }),
            keyframeAnimation: undefined
        });
    });

    test('interpolates optional end ratios and an absent previous opacity through defaults', () => {
        const previous = snapshot({ opacity: undefined });
        const manualFrame = {
            ...input({ opacity: 0.5 }),
            innerRadiusRatio: undefined,
            outerRadiusRatio: undefined,
            previous,
            leaving: false
        } as HistoricalSectorAnimationFrame;
        const result = renderItem([manualFrame])(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            { coord: polarCoord }
        );
        const animation = result['keyframeAnimation'] as { keyframes: Array<{ style: { opacity: number } }> };

        expect(animation.keyframes[0]?.style.opacity).toBe(1);
        expect(animation.keyframes.at(-1)?.style.opacity).toBe(0.5);
    });
});
