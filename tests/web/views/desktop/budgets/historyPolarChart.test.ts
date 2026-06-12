import { describe, expect, test } from '@jest/globals';

import { BUDGET_HISTORY_CHART_CONFIG } from '@/config/budget.ts';

import type {
    HistoricalCategoryChartPoint,
    HistoricalLabelAnimationFrameInput,
    HistoricalLegendSelection
} from '@/views/desktop/budgets/historyPolarChart.ts';
import {
    buildHistoricalPolarChartModel,
    buildHistoricalPolarChartOption,
    createHistoricalLabelAnimationState,
    getHistoricalAmountAxisInterval,
    getTangentialTextRotation,
    interpolateHistoricalPolarAngle,
    resetHistoricalCategoryAnimationState,
    resolveHistoricalLabelAnimationFrames,
    resolveNearestCircularAngle,
    syncHistoricalLegendSelection,
    toggleHistoricalPrimarySelection,
    toggleHistoricalSecondarySelection
} from '@/views/desktop/budgets/historyPolarChart.ts';

const SAMPLE_POINTS: HistoricalCategoryChartPoint[] = [
    {
        category: '早餐',
        primaryCategory: '餐饮',
        secondaryCategory: '早餐',
        budgetAmountCents: 800,
        spentAmountCents: 620,
        executionRate: 77.5,
        color: '#5470c6',
        groupOrder: 1,
        itemOrder: 1
    },
    {
        category: '午餐',
        primaryCategory: '餐饮',
        secondaryCategory: '午餐',
        budgetAmountCents: 1500,
        spentAmountCents: 1300,
        executionRate: 86.7,
        color: '#5470c6',
        groupOrder: 1,
        itemOrder: 2
    },
    {
        category: '地铁',
        primaryCategory: '交通',
        secondaryCategory: '地铁',
        budgetAmountCents: 500,
        spentAmountCents: 260,
        executionRate: 52,
        color: '#91cc75',
        groupOrder: 2,
        itemOrder: 1
    },
    {
        category: '打车',
        primaryCategory: '交通',
        secondaryCategory: '打车',
        budgetAmountCents: 700,
        spentAmountCents: 820,
        executionRate: 117.1,
        color: '#91cc75',
        groupOrder: 2,
        itemOrder: 2
    }
];

function buildSelection(overrides: HistoricalLegendSelection = {}): HistoricalLegendSelection {
    return syncHistoricalLegendSelection(SAMPLE_POINTS, overrides);
}

function makeLabelInput(key: string, polarAngleValue: number, rotate = 0): HistoricalLabelAnimationFrameInput {
    return {
        stateKey: `secondary:${key}`,
        dataId: `${key}:label`,
        name: key,
        text: key,
        radiusValue: 100,
        polarAngleValue,
        rotate,
        color: '#5470c6',
        fontSize: 10,
        fontWeight: 600
    };
}

function normalizeRotationForTest(rotation: number): number {
    return ((rotation + 180) % 360 + 360) % 360 - 180;
}

function expectNoLeaveTransition(value: unknown): void {
    if (!value || typeof value !== 'object') {
        return;
    }

    const record = value as Record<string, unknown>;
    expect(record['leaveTo']).toBeUndefined();
    expect(record['leaveAnimation']).toBeUndefined();

    Object.values(record).forEach(child => {
        if (Array.isArray(child)) {
            child.forEach(item => expectNoLeaveTransition(item));
            return;
        }

        expectNoLeaveTransition(child);
    });
}

const SUPPORTED_ELEMENT_TRANSITION_PROPS = new Set([
    'x',
    'y',
    'originX',
    'originY',
    'anchorX',
    'anchorY',
    'rotation',
    'scaleX',
    'scaleY',
    'skewX',
    'skewY'
]);

function expectNoUnsupportedElementTransition(value: unknown): void {
    if (!value || typeof value !== 'object') {
        return;
    }

    const record = value as Record<string, unknown>;
    if (Array.isArray(record['transition'])) {
        expect(record['transition'].every(prop => SUPPORTED_ELEMENT_TRANSITION_PROPS.has(String(prop)))).toBe(true);
    }

    Object.values(record).forEach(child => {
        if (Array.isArray(child)) {
            child.forEach(item => expectNoUnsupportedElementTransition(item));
            return;
        }

        expectNoUnsupportedElementTransition(child);
    });
}

describe('historyPolarChart helpers', () => {
    test('syncHistoricalLegendSelection preserves known states and defaults new items to visible', () => {
        const selection = syncHistoricalLegendSelection(SAMPLE_POINTS, {
            '餐饮::早餐': false
        });

        expect(selection).toStrictEqual({
            '餐饮::早餐': false,
            '餐饮::午餐': true,
            '交通::地铁': true,
            '交通::打车': true
        });
    });

    test('buildHistoricalPolarChartModel uses only visible slots and computes clockwise primary bands', () => {
        const model = buildHistoricalPolarChartModel(SAMPLE_POINTS, buildSelection());

        expect(model.slots).toHaveLength(4);
        expect(model.slots.map(slot => Number(slot.angle.toFixed(2)))).toStrictEqual([45, -45, -135, -225]);
        expect(model.primaryBands).toStrictEqual([
            expect.objectContaining({ key: '餐饮', state: 'all' }),
            expect.objectContaining({ key: '交通', state: 'all' })
        ]);
        expect(model.primaryPadAngle).toBeGreaterThan(0);
        expect(model.primaryLabels).toStrictEqual([
            expect.objectContaining({ label: '餐饮', rotate: expect.any(Number) }),
            expect.objectContaining({ label: '交通', rotate: expect.any(Number) })
        ]);
        expect(Math.abs(model.primaryLabels[0]?.rotate ?? 0)).toBeLessThanOrEqual(90);
        expect(Math.abs(model.primaryLabels[1]?.rotate ?? 0)).toBeLessThanOrEqual(90);
        expect(model.primaryBands[0]?.startAngle).toBeGreaterThan(model.primaryBands[0]?.endAngle || -Infinity);
        expect(model.amountAxisMax).toBeGreaterThan(1500);
    });

    test('getTangentialTextRotation keeps labels upright while staying perpendicular to the radial direction', () => {
        expect(getTangentialTextRotation(45)).toBeCloseTo(-45, 1);
        expect(getTangentialTextRotation(135)).toBeCloseTo(45, 1);
        expect(getTangentialTextRotation(200)).toBeCloseTo(-70, 1);
        expect(getTangentialTextRotation(220)).toBeLessThan(0);
        expect(getTangentialTextRotation(270)).toBeCloseTo(0, 1);
        expect(getTangentialTextRotation(320)).toBeGreaterThan(0);
        expect(getTangentialTextRotation(340)).toBeGreaterThan(0);
        expect(getTangentialTextRotation(350)).toBeCloseTo(80, 1);
        [45, 135, 200, 220, 270, 320, 340, 350].forEach(angle => {
            const rotation = getTangentialTextRotation(angle);
            expect(Math.abs(rotation)).toBeLessThanOrEqual(90);
            expect(Math.abs(normalizeRotationForTest(rotation - (angle - 90))) % 180).toBeCloseTo(0, 1);
        });
    });

    test('resolveNearestCircularAngle and interpolateHistoricalPolarAngle use the shortest path across zero degrees', () => {
        expect(resolveNearestCircularAngle(10, 350)).toBeCloseTo(370);
        expect(resolveNearestCircularAngle(350, 10)).toBeCloseTo(-10);
        expect(interpolateHistoricalPolarAngle(350, 10, 0.5)).toBeCloseTo(360);
        expect(interpolateHistoricalPolarAngle(10, 350, 0.5)).toBeCloseTo(0);
    });

    test('resolveHistoricalLabelAnimationFrames keeps hidden label state for restore animation', () => {
        const state = createHistoricalLabelAnimationState();

        resolveHistoricalLabelAnimationFrames([
            makeLabelInput('food', 350, 80),
            makeLabelInput('coffee', 120, -20)
        ], state);

        const visibleAfterHide = resolveHistoricalLabelAnimationFrames([
            makeLabelInput('coffee', 140, -10)
        ], state);
        const coffeeAfterHide = visibleAfterHide.find(frame => frame.stateKey === 'secondary:coffee');
        const leavingFood = visibleAfterHide.find(frame => frame.stateKey === 'secondary:food');
        expect(visibleAfterHide).toHaveLength(2);
        expect(coffeeAfterHide?.previous?.polarAngleValue).toBeCloseTo(120);
        expect(leavingFood).toMatchObject({
            opacity: 0,
            leaving: true,
            scale: expect.any(Number)
        });

        const visibleStillHidden = resolveHistoricalLabelAnimationFrames([
            makeLabelInput('coffee', 150, -5)
        ], state);
        expect(visibleStillHidden.map(frame => frame.stateKey)).toStrictEqual(['secondary:coffee']);

        const visibleAfterRestore = resolveHistoricalLabelAnimationFrames([
            makeLabelInput('food', 10, 85),
            makeLabelInput('coffee', 160, 0)
        ], state);
        const restoredFood = visibleAfterRestore.find(frame => frame.stateKey === 'secondary:food');

        expect(restoredFood?.previous?.polarAngleValue).toBeCloseTo(350);
        expect(restoredFood?.polarAngleValue).toBeCloseTo(370);
        expect(Math.abs((restoredFood?.polarAngleValue ?? 0) - (restoredFood?.previous?.polarAngleValue ?? 0)))
            .toBeLessThanOrEqual(180);
    });

    test('resolveHistoricalLabelAnimationFrames keeps primary and secondary ghost labels isolated', () => {
        const state = createHistoricalLabelAnimationState();
        const primaryInput = {
            ...makeLabelInput('food-primary', 40),
            stateKey: 'primary:food',
            dataId: 'primary:food:label'
        };

        resolveHistoricalLabelAnimationFrames([primaryInput], state, 'primary:');

        const secondaryFrames = resolveHistoricalLabelAnimationFrames([
            makeLabelInput('coffee', 140)
        ], state, 'secondary:');

        expect(secondaryFrames.map(frame => frame.stateKey)).toStrictEqual(['secondary:coffee']);

        const primaryFrames = resolveHistoricalLabelAnimationFrames([], state, 'primary:');
        expect(primaryFrames).toHaveLength(1);
        expect(primaryFrames[0]).toMatchObject({
            stateKey: 'primary:food',
            leaving: true,
            opacity: 0
        });
    });

    test('resetHistoricalCategoryAnimationState preserves amount axis snapshots for type switching', () => {
        const state = createHistoricalLabelAnimationState();

        resolveHistoricalLabelAnimationFrames([
            makeLabelInput('food', 120)
        ], state, 'secondary:');
        buildHistoricalPolarChartOption(buildHistoricalPolarChartModel(SAMPLE_POINTS, buildSelection()), {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true,
            labelAnimationState: state
        });

        expect(Array.from(state.keys()).some(key => key.startsWith('grid:amount:'))).toBe(true);
        expect(Array.from(state.keys()).some(key => key.startsWith('grid-label:amount:'))).toBe(true);
        expect(Array.from(state.keys()).some(key => key.startsWith('secondary:'))).toBe(true);
        expect(Array.from(state.keys()).some(key => key.startsWith('ring:'))).toBe(true);
        expect(Array.from(state.keys()).some(key => key.startsWith('bar:budget:'))).toBe(true);

        resetHistoricalCategoryAnimationState(state);

        expect(Array.from(state.keys()).some(key => key.startsWith('grid:amount:'))).toBe(true);
        expect(Array.from(state.keys()).some(key => key.startsWith('grid-label:amount:'))).toBe(true);
        expect(Array.from(state.keys()).some(key => key.startsWith('secondary:'))).toBe(false);
        expect(Array.from(state.keys()).some(key => key.startsWith('ring:'))).toBe(false);
        expect(Array.from(state.keys()).some(key => key.startsWith('bar:budget:'))).toBe(false);
    });

    test('buildHistoricalPolarChartOption restarts amount-axis rendering across structural switches while reusing indexed amount-axis state', () => {
        const labelAnimationState = createHistoricalLabelAnimationState();
        const points: HistoricalCategoryChartPoint[] = [
            {
                category: '早餐',
                primaryCategory: '餐饮',
                secondaryCategory: '早餐',
                budgetAmountCents: 1000,
                spentAmountCents: 800,
                executionRate: 80,
                color: '#5470c6',
                groupOrder: 1,
                itemOrder: 1
            },
            {
                category: '大额',
                primaryCategory: '居住',
                secondaryCategory: '大额',
                budgetAmountCents: 3000,
                spentAmountCents: 3000,
                executionRate: 100,
                color: '#91cc75',
                groupOrder: 2,
                itemOrder: 1
            }
        ];
        const baseOptions = {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true,
            labelAnimationState
        };
        const fullModel = buildHistoricalPolarChartModel(points, syncHistoricalLegendSelection(points));
        buildHistoricalPolarChartOption(fullModel, {
            ...baseOptions,
            categoryAnimationScope: 'expense-secondary',
            amountAxisRenderScope: 'expense-secondary'
        });
        const filteredModel = buildHistoricalPolarChartModel(
            points,
            syncHistoricalLegendSelection(points, { '居住::大额': false })
        );
        const filteredOption = buildHistoricalPolarChartOption(filteredModel, {
            ...baseOptions,
            categoryAnimationScope: 'expense-primary',
            amountAxisRenderScope: 'expense-primary'
        }) as {
            series: Array<{
                name?: string;
                data?: Array<{ id?: string; value?: [number, number] }>;
                renderItem?: (
                    params: { dataIndex: number; coordSys: { cx: number; cy: number } },
                    api: { coord: (value: number[]) => number[] }
                ) => {
                    keyframeAnimation?: {
                        keyframes?: Array<{
                            x?: number;
                            y?: number;
                            shape?: { r?: number };
                        }>;
                    };
                };
            }>;
        };
        const amountGridLineSeries = filteredOption.series.find(item => item.name === 'amount-grid-lines');
        const amountAxisLabelSeries = filteredOption.series.find(item => item.name === 'amount-axis-labels');
        const retainedGridId = amountGridLineSeries?.data
            ?.map(item => item.id)
            .find((id): id is string => Boolean(
                id
                && id.startsWith('grid:amount:')
                && id.includes(':expense-primary-axis-')
                && !id.startsWith('grid:amount:0:')
            ));
        const retainedGridStateKey = retainedGridId?.match(/^(grid:amount:\d+)/)?.[1] ?? '';
        const retainedLabelStateKey = retainedGridStateKey.replace('grid:amount:', 'grid-label:amount:');
        const retainedLabelId = amountAxisLabelSeries?.data
            ?.map(item => item.id)
            .find((id): id is string => Boolean(
                id
                && id.startsWith(`${retainedLabelStateKey}:expense-primary-axis-`)
            ));
        const gridIndex = amountGridLineSeries?.data?.findIndex(item => item.id === retainedGridId) ?? -1;
        const labelIndex = amountAxisLabelSeries?.data?.findIndex(item => item.id === retainedLabelId) ?? -1;
        const polarCoord = ([radius]: number[]) => {
            const pixelRadius = (Number(radius) / filteredModel.amountAxisMax) * 100;
            return [100 + pixelRadius, 100];
        };
        const renderedGridLine = gridIndex >= 0
            ? amountGridLineSeries?.renderItem?.(
                { dataIndex: gridIndex, coordSys: { cx: 100, cy: 100 } },
                { coord: polarCoord }
            )
            : undefined;
        const renderedAmountLabel = labelIndex >= 0
            ? amountAxisLabelSeries?.renderItem?.(
                { dataIndex: labelIndex, coordSys: { cx: 100, cy: 100 } },
                { coord: polarCoord }
            )
            : undefined;
        const gridKeyframes = renderedGridLine?.keyframeAnimation?.keyframes ?? [];
        const labelKeyframes = renderedAmountLabel?.keyframeAnimation?.keyframes ?? [];

        const amountStateKeys = Array.from(labelAnimationState.keys())
            .filter(key => key.startsWith('grid:amount:') || key.startsWith('grid-label:amount:'));
        expect(amountStateKeys).toContain(retainedGridStateKey);
        expect(amountStateKeys).toContain(retainedLabelStateKey);
        expect(amountStateKeys.some(key => key.includes('expense-primary'))).toBe(false);
        expect(gridIndex).toBeGreaterThanOrEqual(0);
        expect(labelIndex).toBeGreaterThanOrEqual(0);
        expect(gridKeyframes[0]?.shape?.r).not.toBe(gridKeyframes[gridKeyframes.length - 1]?.shape?.r);
        expect(labelKeyframes[0]?.y).not.toBe(labelKeyframes[labelKeyframes.length - 1]?.y);
    });

    test('buildHistoricalPolarChartOption keeps four indexed amount-axis guides evenly spaced with relay transitions', () => {
        const labelAnimationState = createHistoricalLabelAnimationState();
        const points: HistoricalCategoryChartPoint[] = [
            {
                category: '早餐',
                primaryCategory: '餐饮',
                secondaryCategory: '早餐',
                budgetAmountCents: 1000,
                spentAmountCents: 800,
                executionRate: 80,
                color: '#5470c6',
                groupOrder: 1,
                itemOrder: 1
            },
            {
                category: '大额',
                primaryCategory: '居住',
                secondaryCategory: '大额',
                budgetAmountCents: 3000,
                spentAmountCents: 3000,
                executionRate: 100,
                color: '#91cc75',
                groupOrder: 2,
                itemOrder: 1
            }
        ];
        const baseOptions = {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true,
            categoryAnimationScope: 'expense-secondary',
            labelAnimationState
        };
        type AmountAxisOptionForTest = {
            series: Array<{
                name?: string;
                data?: Array<{ id?: string; value?: [number, number] }>;
                renderItem?: (
                    params: { dataIndex: number; coordSys: { cx: number; cy: number } },
                    api: { coord: (value: number[]) => number[] }
                ) => {
                    shape?: { r?: number };
                    x?: number;
                    y?: number;
                    style?: { opacity?: number };
                    enterAnimation?: unknown;
                    enterFrom?: unknown;
                    keyframeAnimation?: {
                        keyframes?: Array<{
                            x?: number;
                            y?: number;
                            shape?: { r?: number };
                            style?: { opacity?: number };
                        }>;
                    };
                };
            }>;
        };
        const renderGrid = (
            series: NonNullable<AmountAxisOptionForTest['series'][number]>,
            dataIndex: number,
            axisMax: number
        ) => series.renderItem?.(
            { dataIndex, coordSys: { cx: 100, cy: 100 } },
            {
                coord: ([radius]) => [100 + ((Number(radius) / axisMax) * 100), 100]
            }
        );
        const renderLabel = renderGrid;
        const scopedRenderSuffix = 'expense-secondary';
        const fullModel = buildHistoricalPolarChartModel(points, syncHistoricalLegendSelection(points));
        const firstOption = buildHistoricalPolarChartOption(fullModel, {
            ...baseOptions,
            amountAxisRenderScope: scopedRenderSuffix
        }) as AmountAxisOptionForTest;
        const firstGridSeries = firstOption.series.find(item => item.name === 'amount-grid-lines');
        const firstLabelSeries = firstOption.series.find(item => item.name === 'amount-axis-labels');
        const firstOuterGridIndex = firstGridSeries?.data?.findIndex(
            item => item.id?.startsWith('grid:amount:3:expense-secondary-axis-')
        ) ?? -1;
        const firstOuterLabelIndex = firstLabelSeries?.data?.findIndex(
            item => item.id?.startsWith('grid-label:amount:3:expense-secondary-axis-')
        ) ?? -1;
        const firstOuterGrid = firstOuterGridIndex >= 0 && firstGridSeries
            ? renderGrid(firstGridSeries, firstOuterGridIndex, fullModel.amountAxisMax)
            : undefined;
        const firstOuterLabel = firstOuterLabelIndex >= 0 && firstLabelSeries
            ? renderLabel(firstLabelSeries, firstOuterLabelIndex, fullModel.amountAxisMax)
            : undefined;
        const firstGridKeyframes = firstOuterGrid?.keyframeAnimation?.keyframes ?? [];
        const firstLabelKeyframes = firstOuterLabel?.keyframeAnimation?.keyframes ?? [];

        expect(firstGridSeries?.data?.length).toBe(4);
        expect(firstLabelSeries?.data?.length).toBe(4);
        firstGridSeries?.data?.forEach((item, index) => {
            expect(item.id).toMatch(new RegExp(`^grid:amount:${index}:${scopedRenderSuffix}-axis-`));
            expect(item.value?.[0]).toBeCloseTo((fullModel.amountAxisMax / 3) * index);
        });
        firstLabelSeries?.data?.forEach((item, index) => {
            expect(item.id).toMatch(new RegExp(`^grid-label:amount:${index}:${scopedRenderSuffix}-axis-`));
            expect(item.value?.[0]).toBeCloseTo((fullModel.amountAxisMax / 3) * index);
        });
        expect(firstOuterGrid?.enterAnimation).toBeUndefined();
        expect(firstOuterGrid?.enterFrom).toBeUndefined();
        expect(firstOuterGrid?.style?.opacity).toBe(0);
        expect(firstOuterGrid?.shape?.r).toBeCloseTo(firstGridKeyframes[0]?.shape?.r ?? 0);
        expect(firstGridKeyframes[0]?.shape?.r)
            .toBeGreaterThan(firstGridKeyframes[firstGridKeyframes.length - 1]?.shape?.r ?? 0);
        expect(firstGridKeyframes[0]?.style?.opacity).toBe(0);
        expect(firstGridKeyframes[firstGridKeyframes.length - 1]?.style?.opacity).toBe(1);
        expect(firstOuterLabel?.enterAnimation).toBeUndefined();
        expect(firstOuterLabel?.enterFrom).toBeUndefined();
        expect(firstOuterLabel?.style?.opacity).toBe(0);
        expect(firstOuterLabel?.y).toBeCloseTo(firstLabelKeyframes[0]?.y ?? 0);
        expect(firstLabelKeyframes[0]?.style?.opacity).toBe(0);
        expect(firstLabelKeyframes[firstLabelKeyframes.length - 1]?.style?.opacity).toBe(1);

        const amountStateKeys = Array.from(labelAnimationState.keys())
            .filter(key => key.startsWith('grid:amount:') || key.startsWith('grid-label:amount:'));
        expect(amountStateKeys).toEqual(expect.arrayContaining([
            'grid:amount:0',
            'grid:amount:1',
            'grid:amount:2',
            'grid:amount:3',
            'grid-label:amount:0',
            'grid-label:amount:1',
            'grid-label:amount:2',
            'grid-label:amount:3'
        ]));
        expect(amountStateKeys.some(key => key.includes(scopedRenderSuffix))).toBe(false);

        const filteredModel = buildHistoricalPolarChartModel(
            points,
            syncHistoricalLegendSelection(points, { '居住::大额': false })
        );
        const filteredOption = buildHistoricalPolarChartOption(filteredModel, {
            ...baseOptions,
            amountAxisRenderScope: scopedRenderSuffix
        }) as AmountAxisOptionForTest;
        const filteredGridSeries = filteredOption.series.find(item => item.name === 'amount-grid-lines');
        const filteredLabelSeries = filteredOption.series.find(item => item.name === 'amount-axis-labels');
        const filteredGridData = filteredGridSeries?.data ?? [];
        const filteredLabelData = filteredLabelSeries?.data ?? [];
        const filteredCurrentGridData = filteredGridData.slice(0, 4);
        const filteredCurrentLabelData = filteredLabelData.slice(0, 4);
        const filteredGhostGridData = filteredGridData.slice(4);
        const filteredGhostLabelData = filteredLabelData.slice(4);

        expect(filteredModel.amountAxisMax).toBeLessThan(fullModel.amountAxisMax);
        expect(filteredGridData).toHaveLength(5);
        expect(filteredLabelData).toHaveLength(8);
        expect(new Set(filteredGridData.map(item => item.id)).size).toBe(filteredGridData.length);
        expect(new Set(filteredLabelData.map(item => item.id)).size).toBe(filteredLabelData.length);
        expect(filteredGhostGridData).toHaveLength(1);
        expect(filteredGhostLabelData).toHaveLength(4);
        filteredCurrentGridData.forEach((item, index) => {
            expect(item.id).toMatch(new RegExp(`^grid:amount:${index}:${scopedRenderSuffix}-axis-`));
            expect(item.value?.[0]).toBeCloseTo((filteredModel.amountAxisMax / 3) * index);
        });
        filteredCurrentLabelData.forEach((item, index) => {
            expect(item.id).toMatch(new RegExp(`^grid-label:amount:${index}:${scopedRenderSuffix}-axis-`));
            expect(item.value?.[0]).toBeCloseTo((filteredModel.amountAxisMax / 3) * index);
        });
        [1, 2, 3].forEach(index => {
            const rendered = filteredGridSeries ? renderGrid(filteredGridSeries, index, filteredModel.amountAxisMax) : undefined;
            const keyframes = rendered?.keyframeAnimation?.keyframes ?? [];
            expect(keyframes[0]?.shape?.r).toBeLessThan(keyframes[keyframes.length - 1]?.shape?.r ?? 0);
            expect(keyframes[0]?.style?.opacity).toBe(1);
            expect(keyframes[keyframes.length - 1]?.style?.opacity).toBe(1);
        });
        [1, 2, 3].forEach(index => {
            const rendered = filteredLabelSeries ? renderLabel(filteredLabelSeries, index, filteredModel.amountAxisMax) : undefined;
            const keyframes = rendered?.keyframeAnimation?.keyframes ?? [];
            expect(keyframes[0]?.style?.opacity).toBe(0);
            expect(keyframes[1]?.style?.opacity).toBe(0);
            expect(keyframes[keyframes.length - 1]?.style?.opacity).toBe(1);
        });
        const filteredLeavingGridIndex = filteredGridData.length - 1;
        const filteredLeavingGrid = filteredGridSeries
            ? renderGrid(filteredGridSeries, filteredLeavingGridIndex, filteredModel.amountAxisMax)
            : undefined;
        const filteredLeavingGridKeyframes = filteredLeavingGrid?.keyframeAnimation?.keyframes ?? [];
        expect(filteredGhostGridData[0]?.id).toMatch(/^grid:amount:3:/);
        expect(filteredLeavingGridKeyframes[filteredLeavingGridKeyframes.length - 1]?.shape?.r)
            .toBeGreaterThan(filteredLeavingGridKeyframes[0]?.shape?.r ?? 0);
        expect(filteredLeavingGridKeyframes[0]?.style?.opacity).toBe(1);
        expect(filteredLeavingGridKeyframes[filteredLeavingGridKeyframes.length - 1]?.style?.opacity).toBe(0);
        filteredGhostLabelData.forEach((_, offset) => {
            const dataIndex = 4 + offset;
            const rendered = filteredLabelSeries ? renderLabel(filteredLabelSeries, dataIndex, filteredModel.amountAxisMax) : undefined;
            const keyframes = rendered?.keyframeAnimation?.keyframes ?? [];
            expect(keyframes[0]?.style?.opacity).toBe(1);
            expect(keyframes[keyframes.length - 1]?.style?.opacity).toBe(0);
            expect(keyframes[0]?.y).toBeCloseTo(keyframes[keyframes.length - 1]?.y ?? 0);
        });

        const restoredOption = buildHistoricalPolarChartOption(fullModel, {
            ...baseOptions,
            amountAxisRenderScope: scopedRenderSuffix
        }) as AmountAxisOptionForTest;
        const restoredGridSeries = restoredOption.series.find(item => item.name === 'amount-grid-lines');
        const restoredLabelSeries = restoredOption.series.find(item => item.name === 'amount-axis-labels');
        const restoredGridData = restoredGridSeries?.data ?? [];
        const restoredLabelData = restoredLabelSeries?.data ?? [];

        expect(restoredGridData).toHaveLength(5);
        expect(restoredLabelData).toHaveLength(8);
        expect(new Set(restoredGridData.map(item => item.id)).size).toBe(restoredGridData.length);
        expect(new Set(restoredLabelData.map(item => item.id)).size).toBe(restoredLabelData.length);
        [1, 2].forEach(index => {
            const rendered = restoredGridSeries ? renderGrid(restoredGridSeries, index, fullModel.amountAxisMax) : undefined;
            const keyframes = rendered?.keyframeAnimation?.keyframes ?? [];
            expect(keyframes[0]?.shape?.r).toBeGreaterThan(keyframes[keyframes.length - 1]?.shape?.r ?? 0);
            expect(keyframes[0]?.style?.opacity).toBe(1);
            expect(keyframes[keyframes.length - 1]?.style?.opacity).toBe(1);
        });
        const restoredOuterGrid = restoredGridSeries ? renderGrid(restoredGridSeries, 3, fullModel.amountAxisMax) : undefined;
        const restoredOuterGridKeyframes = restoredOuterGrid?.keyframeAnimation?.keyframes ?? [];
        expect(restoredOuterGridKeyframes[0]?.shape?.r)
            .toBeGreaterThan(restoredOuterGridKeyframes[restoredOuterGridKeyframes.length - 1]?.shape?.r ?? 0);
        expect(restoredOuterGridKeyframes[0]?.style?.opacity).toBe(0);
        expect(restoredOuterGridKeyframes[restoredOuterGridKeyframes.length - 1]?.style?.opacity).toBe(1);
        const restoredLeavingGrid = restoredGridSeries
            ? renderGrid(restoredGridSeries, restoredGridData.length - 1, fullModel.amountAxisMax)
            : undefined;
        const restoredLeavingGridKeyframes = restoredLeavingGrid?.keyframeAnimation?.keyframes ?? [];
        expect(restoredGridData[4]?.id).toMatch(/^grid:amount:1:/);
        expect(restoredLeavingGridKeyframes[0]?.shape?.r)
            .toBeGreaterThan(restoredLeavingGridKeyframes[restoredLeavingGridKeyframes.length - 1]?.shape?.r ?? 0);
        expect(restoredLeavingGridKeyframes[0]?.style?.opacity).toBe(1);
        expect(restoredLeavingGridKeyframes[restoredLeavingGridKeyframes.length - 1]?.style?.opacity).toBe(0);
        [1, 2, 3].forEach(index => {
            const rendered = restoredLabelSeries ? renderLabel(restoredLabelSeries, index, fullModel.amountAxisMax) : undefined;
            const keyframes = rendered?.keyframeAnimation?.keyframes ?? [];
            expect(keyframes[0]?.style?.opacity).toBe(0);
            expect(keyframes[keyframes.length - 1]?.style?.opacity).toBe(1);
        });
    });

    test('buildHistoricalPolarChartOption scopes category rings across budget type switches', () => {
        const labelAnimationState = createHistoricalLabelAnimationState();
        const baseOptions = {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true,
            labelAnimationState
        };
        buildHistoricalPolarChartOption(buildHistoricalPolarChartModel(SAMPLE_POINTS, buildSelection()), {
            ...baseOptions,
            categoryAnimationScope: 'expense-secondary',
            amountAxisRenderScope: 'expense-secondary'
        });
        resetHistoricalCategoryAnimationState(labelAnimationState);

        const investmentPoints: HistoricalCategoryChartPoint[] = [
            {
                category: '基金申购',
                primaryCategory: '基金',
                secondaryCategory: '基金申购',
                budgetAmountCents: 2000,
                spentAmountCents: 1200,
                executionRate: 60,
                color: '#ffb300',
                groupOrder: 1,
                itemOrder: 1
            },
            {
                category: '股票',
                primaryCategory: '股票',
                secondaryCategory: '股票',
                budgetAmountCents: 1600,
                spentAmountCents: 900,
                executionRate: 56.3,
                color: '#26a69a',
                groupOrder: 2,
                itemOrder: 1
            }
        ];
        const investmentOption = buildHistoricalPolarChartOption(
            buildHistoricalPolarChartModel(investmentPoints, syncHistoricalLegendSelection(investmentPoints)),
            {
                ...baseOptions,
                accentColor: '#ffb300',
                categoryAnimationScope: 'investment-secondary',
                amountAxisRenderScope: 'investment-secondary'
            }
        ) as {
            series: Array<{
                name?: string;
                data?: Array<{ id?: string; value?: [number, number] }>;
                renderItem?: (
                    params: { dataIndex: number; coordSys: { cx: number; cy: number } },
                    api: { coord: (value: number[]) => number[] }
                ) => {
                    keyframeAnimation?: {
                        keyframes?: Array<{
                            shape?: { startAngle?: number; endAngle?: number };
                        }>;
                    };
                };
            }>;
        };
        const primaryRing = investmentOption.series.find(item => item.name === 'primary-ring');
        const ringIds = primaryRing?.data?.map(item => item.id) ?? [];
        const renderedFirstRing = primaryRing?.renderItem?.(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            {
                coord: ([radius, angle]) => [
                    100 + ((50 + (Number(radius) * 50)) * Math.cos((Number(angle) * Math.PI) / 180)),
                    100 + ((50 + (Number(radius) * 50)) * Math.sin((Number(angle) * Math.PI) / 180))
                ]
            }
        );
        const firstKeyframe = renderedFirstRing?.keyframeAnimation?.keyframes?.[0];
        const firstSpan = Math.abs((firstKeyframe?.shape?.endAngle ?? 0) - (firstKeyframe?.shape?.startAngle ?? 0));

        expect(ringIds.every(id => id?.includes('investment-secondary'))).toBe(true);
        expect(ringIds.some(id => id?.includes('expense-secondary'))).toBe(false);
        expect(firstSpan).toBeLessThan(0.01);

        resetHistoricalCategoryAnimationState(labelAnimationState);
        const expenseAgainOption = buildHistoricalPolarChartOption(buildHistoricalPolarChartModel(SAMPLE_POINTS, buildSelection()), {
            ...baseOptions,
            categoryAnimationScope: 'expense-secondary',
            amountAxisRenderScope: 'expense-secondary'
        }) as { series: Array<{ name?: string; data?: Array<{ id?: string }> }> };
        const expenseRingIds = expenseAgainOption.series
            .find(item => item.name === 'primary-ring')?.data?.map(item => item.id) ?? [];
        expect(expenseRingIds.every(id => id?.includes('expense-secondary'))).toBe(true);
        expect(expenseRingIds.some(id => id?.includes('investment-secondary'))).toBe(false);
    });

    test('buildHistoricalPolarChartOption renders a scoped single-primary investment ring on first entry', () => {
        const investmentPoints: HistoricalCategoryChartPoint[] = [
            {
                category: '房地产',
                primaryCategory: '投资',
                secondaryCategory: '房地产',
                budgetAmountCents: 300,
                spentAmountCents: 300,
                executionRate: 0,
                color: '#ffb300',
                groupOrder: 1,
                itemOrder: 1
            }
        ];
        const option = buildHistoricalPolarChartOption(
            buildHistoricalPolarChartModel(investmentPoints, syncHistoricalLegendSelection(investmentPoints)),
            {
                isDarkMode: false,
                accentColor: '#ffb300',
                budgetAmountLabel: '预算金额',
                spentAmountLabel: '已花费',
                executionRateLabel: '执行度',
                formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
                showPrimaryRing: true,
                categoryAnimationScope: 'investment-secondary',
                amountAxisRenderScope: 'investment-secondary',
                labelAnimationState: createHistoricalLabelAnimationState()
            }
        ) as {
            series: Array<{
                id?: string;
                name?: string;
                data?: Array<{ id?: string }>;
                renderItem?: (
                    params: { dataIndex: number; coordSys: { cx: number; cy: number } },
                    api: { coord: (value: number[]) => number[] }
                ) => {
                    keyframeAnimation?: {
                        keyframes?: Array<{
                            shape?: { startAngle?: number; endAngle?: number };
                        }>;
                    };
                };
            }>;
        };
        const primaryRing = option.series.find(item => item.name === 'primary-ring');
        const renderedRing = primaryRing?.renderItem?.(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            {
                coord: ([radius, angle]) => [
                    100 + ((50 + (Number(radius) * 50)) * Math.cos((Number(angle) * Math.PI) / 180)),
                    100 + ((50 + (Number(radius) * 50)) * Math.sin((Number(angle) * Math.PI) / 180))
                ]
            }
        );
        const lastKeyframe = renderedRing?.keyframeAnimation?.keyframes?.at(-1);
        const finalSpan = Math.abs((lastKeyframe?.shape?.endAngle ?? 0) - (lastKeyframe?.shape?.startAngle ?? 0));

        expect(primaryRing?.id).toBe('primary-ring:investment-secondary');
        expect(primaryRing?.data?.map(item => item.id)).toStrictEqual(['ring:investment-secondary:投资']);
        expect(finalSpan).toBeGreaterThan(6);
        expect(finalSpan).toBeLessThan(Math.PI * 2);
    });

    test('resolveHistoricalLabelAnimationFrames cross-fades labels instead of spinning through face flips', () => {
        const state = createHistoricalLabelAnimationState();

        resolveHistoricalLabelAnimationFrames([
            makeLabelInput('flip', 30, -80)
        ], state);
        const frames = resolveHistoricalLabelAnimationFrames([
            makeLabelInput('flip', 210, 80)
        ], state);

        expect(frames[0]?.crossFadeOnly).toBe(true);

        resolveHistoricalLabelAnimationFrames([
            makeLabelInput('stale-rotate', 30, 260)
        ], state);
        const staleRotateFrames = resolveHistoricalLabelAnimationFrames([
            makeLabelInput('stale-rotate', 210, 80)
        ], state);

        expect(staleRotateFrames[0]?.crossFadeOnly).toBe(true);

        resolveHistoricalLabelAnimationFrames([
            makeLabelInput('smooth-move', 30, -60)
        ], state);
        const smoothMoveFrames = resolveHistoricalLabelAnimationFrames([
            makeLabelInput('smooth-move', 60, -30)
        ], state);

        expect(smoothMoveFrames[0]?.crossFadeOnly).toBe(false);
    });

    test('toggleHistoricalPrimarySelection cascades to all secondary categories in the group', () => {
        const initialSelection = buildSelection();
        const initialModel = buildHistoricalPolarChartModel(SAMPLE_POINTS, initialSelection);
        const hiddenSelection = toggleHistoricalPrimarySelection(initialSelection, initialModel, '餐饮');
        const hiddenModel = buildHistoricalPolarChartModel(SAMPLE_POINTS, hiddenSelection);

        expect(hiddenSelection['餐饮::早餐']).toBe(false);
        expect(hiddenSelection['餐饮::午餐']).toBe(false);
        expect(hiddenModel.legendGroups[0]?.state).toBe('none');

        const restoredSelection = toggleHistoricalPrimarySelection(hiddenSelection, hiddenModel, '餐饮');
        expect(restoredSelection['餐饮::早餐']).toBe(true);
        expect(restoredSelection['餐饮::午餐']).toBe(true);

        expect(toggleHistoricalPrimarySelection(initialSelection, initialModel, '不存在')).toStrictEqual(initialSelection);
    });

    test('toggleHistoricalSecondarySelection only changes the requested secondary category', () => {
        const selection = toggleHistoricalSecondarySelection(buildSelection(), '交通::地铁');
        const model = buildHistoricalPolarChartModel(SAMPLE_POINTS, selection);

        expect(selection['交通::地铁']).toBe(false);
        expect(selection['交通::打车']).toBe(true);
        expect(selection['餐饮::早餐']).toBe(true);
        expect(model.legendGroups[1]?.state).toBe('partial');
    });

    test('buildHistoricalPolarChartModel reflows remaining visible items to fill the circle', () => {
        const selection = buildSelection({
            '餐饮::早餐': false,
            '餐饮::午餐': false
        });
        const model = buildHistoricalPolarChartModel(SAMPLE_POINTS, selection);

        expect(model.slots).toHaveLength(2);
        expect(model.primaryBands).toHaveLength(1);
        expect(model.primaryPadAngle).toBe(0);
        expect(model.legendGroups[0]?.state).toBe('none');
        expect(model.legendGroups[1]?.state).toBe('all');
        expect(model.slots.map(slot => Number(slot.angle.toFixed(2)))).toStrictEqual([0, -180]);
    });

    test('buildHistoricalPolarChartModel handles empty points and blank labels safely', () => {
        const blankLabelModel = buildHistoricalPolarChartModel([
            {
                category: '其他',
                primaryCategory: '  ',
                secondaryCategory: '',
                budgetAmountCents: 10,
                spentAmountCents: 5,
                executionRate: 50,
                color: 'bad-color',
                groupOrder: 1,
                itemOrder: 1
            }
        ]);
        const emptyModel = buildHistoricalPolarChartModel([]);

        expect(blankLabelModel.primaryLabels).toHaveLength(0);
        expect(emptyModel.slots).toHaveLength(0);
        expect(emptyModel.primaryBands).toHaveLength(0);
        expect(emptyModel.legendGroups).toHaveLength(0);
        expect(emptyModel.averageExecutionRate).toBe(0);
        expect(emptyModel.amountAxisInterval).toBe(25);
    });

    test('buildHistoricalPolarChartModel sanitizes anomalous point values before building axes', () => {
        const model = buildHistoricalPolarChartModel([
            {
                category: '异常项',
                primaryCategory: '异常项',
                secondaryCategory: '',
                budgetAmountCents: Number.POSITIVE_INFINITY,
                spentAmountCents: Number.NaN,
                executionRate: Number.NaN,
                color: undefined,
                groupOrder: Number.NaN,
                itemOrder: Number.NaN
            } as unknown as HistoricalCategoryChartPoint
        ]);
        const option = buildHistoricalPolarChartOption(model, {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: false
        }) as {
            radiusAxis: Array<{ max?: number; interval?: number }>;
            series: Array<Record<string, unknown>>;
        };
        const budgetSeries = option.series.find(item => item['name'] === '预算金额') as {
            data: Array<{ value: [number, number, number, number]; itemStyle: { color: string } }>;
        } | undefined;

        expect(model.slots[0]).toMatchObject({
            budgetAmountCents: 0,
            spentAmountCents: 0,
            executionRate: 0,
            color: '#5470c6'
        });
        expect(Number.isFinite(model.amountAxisMax)).toBe(true);
        expect(Number.isFinite(model.amountAxisInterval)).toBe(true);
        expect(Number.isFinite(option.radiusAxis[0]?.max)).toBe(true);
        expect(Number.isFinite(option.radiusAxis[0]?.interval)).toBe(true);
        expect(budgetSeries?.data[0]?.value[1]).toBe(0);
        expect(budgetSeries?.data[0]?.itemStyle.color).toBe('rgba(84,112,198,0.28)');
        expect(option.series.find(item => item['name'] === '执行度')).toBeUndefined();
    });

    test('buildHistoricalPolarChartModel sorts ties by primary category and fallback category label', () => {
        const model = buildHistoricalPolarChartModel([
            {
                category: '香蕉',
                primaryCategory: '餐饮',
                secondaryCategory: '',
                budgetAmountCents: 100,
                spentAmountCents: 50,
                executionRate: 50,
                color: '#5470c6',
                groupOrder: 1,
                itemOrder: 1
            },
            {
                category: '苹果',
                primaryCategory: '餐饮',
                secondaryCategory: '',
                budgetAmountCents: 120,
                spentAmountCents: 60,
                executionRate: 50,
                color: '#5470c6',
                groupOrder: 1,
                itemOrder: 1
            },
            {
                category: '地铁',
                primaryCategory: '交通',
                secondaryCategory: '',
                budgetAmountCents: 90,
                spentAmountCents: 40,
                executionRate: 44.4,
                color: '#91cc75',
                groupOrder: 1,
                itemOrder: 1
            }
        ]);

        expect(model.legendGroups.map(group => group.primaryKey)).toStrictEqual(['餐饮', '交通']);
        expect(model.slots.filter(slot => slot.primaryKey === '餐饮').map(slot => slot.label)).toStrictEqual(['苹果', '香蕉']);
    });

    test('buildHistoricalPolarChartModel selects stable amount-axis intervals across interval bands', () => {
        const buildSinglePointModel = (budgetAmountCents: number) => buildHistoricalPolarChartModel([
            {
                category: '单项',
                primaryCategory: '测试',
                secondaryCategory: '单项',
                budgetAmountCents,
                spentAmountCents: budgetAmountCents / 2,
                executionRate: 50,
                color: '#5470c6',
                groupOrder: 1,
                itemOrder: 1
            }
        ]);

        expect(buildSinglePointModel(5).amountAxisInterval).toBe(2);
        expect(buildSinglePointModel(8.5).amountAxisInterval).toBe(2.5);
        expect(buildSinglePointModel(15).amountAxisInterval).toBe(5);
        expect(buildSinglePointModel(25).amountAxisInterval).toBe(10);
    });

    test('getHistoricalAmountAxisInterval covers each interval bucket directly', () => {
        expect(getHistoricalAmountAxisInterval(0)).toBe(25);
        expect(getHistoricalAmountAxisInterval(4)).toBe(1);
        expect(getHistoricalAmountAxisInterval(6)).toBe(2);
        expect(getHistoricalAmountAxisInterval(10)).toBe(2.5);
        expect(getHistoricalAmountAxisInterval(16)).toBe(5);
        expect(getHistoricalAmountAxisInterval(28)).toBe(10);
    });

    test('buildHistoricalPolarChartOption keeps equal-width bars, keeps labels on polar tracks, and uses the updated execution scale', () => {
        const selection = buildSelection({ '交通::地铁': false });
        const model = buildHistoricalPolarChartModel(SAMPLE_POINTS, selection);
        const option = buildHistoricalPolarChartOption(model, {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true
        }) as {
            polar: Array<{ radius?: readonly [string, string] | string[] }>;
            angleAxis: Array<{ splitLine?: { show?: boolean } }>;
            radiusAxis: Array<{
                max?: number;
                interval?: number;
                axisLabel?: { show?: boolean; margin?: number; formatter?: (value: number) => string };
                splitLine?: { show?: boolean; lineStyle?: { type?: string; width?: number; color?: string } };
            }>;
            graphic: Array<{
                id?: string;
                transition?: string[];
                enterFrom?: Record<string, unknown>;
                updateAnimation?: Record<string, unknown>;
                children?: Array<Record<string, unknown>>;
            }>;
            tooltip: {
                formatter?: (params: { dataIndex?: number }) => string;
            };
            series: Array<Record<string, unknown>>;
        };

        const primaryRingSeries = option.series.find(item => item['name'] === 'primary-ring') as Record<string, unknown> | undefined;
        const primaryLabelSeries = option.series.find(item => item['name'] === 'primary-labels') as {
            polarIndex?: number;
            animationDurationUpdate?: number;
            data: Array<{
                value: [number, number];
                label: {
                    formatter: string;
                    overflow?: string;
                    rotate: number;
                    width?: number;
                };
            }>;
        } | undefined;
        const budgetSeries = option.series.find(item => item['name'] === '预算金额') as {
            id?: string;
            type?: string;
            animation?: boolean;
            barWidth?: number;
            data: Array<{ value: [number, number, number, number] }>;
            renderItem?: (
                params: { dataIndex: number; coordSys: { cx: number; cy: number } },
                api: { coord: (value: number[]) => number[] }
            ) => Record<string, unknown>;
            animationDurationUpdate?: number;
            animationEasingUpdate?: string;
        } | undefined;
        const spentSeries = option.series.find(item => item['name'] === '已花费') as {
            id?: string;
            type?: string;
            animation?: boolean;
            barWidth?: number;
            data: Array<{ value: [number, number, number, number] }>;
            animationDurationUpdate?: number;
            animationEasingUpdate?: string;
        } | undefined;
        const secondaryLabelSeries = option.series.find(item => item['name'] === 'secondary-labels') as {
            polarIndex?: number;
            animationDurationUpdate?: number;
            data: Array<{
                value: [number, number];
                label: {
                    formatter: string;
                    rotate: number;
                };
            }>;
        } | undefined;
        const amountGridLineSeries = option.series.find(item => item['name'] === 'amount-grid-lines') as {
            id?: string;
            type?: string;
            animation?: boolean;
            data: Array<{ id?: string; value: [number, number] }>;
            renderItem?: (
                params: { dataIndex: number; coordSys: { cx: number; cy: number } },
                api: { coord: (value: number[]) => number[] }
            ) => Record<string, unknown>;
            animationDurationUpdate?: number;
            animationEasingUpdate?: string;
        } | undefined;
        const amountAxisLabelSeries = option.series.find(item => item['name'] === 'amount-axis-labels') as {
            id?: string;
            type?: string;
            data: Array<{ id?: string; value: [number, number]; label?: { formatter?: string; rotate?: number } }>;
            renderItem?: (
                params: { dataIndex: number; coordSys: { cx: number; cy: number } },
                api: { coord: (value: number[]) => number[] }
            ) => Record<string, unknown>;
            animationDurationUpdate?: number;
            animationEasingUpdate?: string;
        } | undefined;

        expect(primaryRingSeries).toBeDefined();
        expect(primaryLabelSeries).toBeDefined();
        expect(budgetSeries).toBeDefined();
        expect(spentSeries).toBeDefined();
        expect(secondaryLabelSeries).toBeDefined();
        expect(amountGridLineSeries).toBeDefined();
        expect(amountAxisLabelSeries).toBeDefined();

        expect(option.angleAxis[0]?.splitLine).toStrictEqual({ show: false });
        expect(option.radiusAxis[0]?.splitLine).toStrictEqual({ show: false });
        expect(option.radiusAxis[1]?.max).toBe(1);
        expect(option.radiusAxis[0]?.axisLabel?.show).toBe(false);
        expect(option.radiusAxis[0]?.axisLabel?.margin).toBe(10);
        expect(option.radiusAxis[0]?.axisLabel?.formatter?.(120)).toBe('¥1.20');
        expect(option.tooltip.formatter?.({ dataIndex: 1 })).toContain('预算金额: ¥15.00');
        expect(option.tooltip.formatter?.({ dataIndex: 1 })).toContain('已花费: ¥13.00');
        expect(option.tooltip.formatter?.({ dataIndex: 1 })).not.toContain('¥1500.00');
        expect(amountGridLineSeries?.id).toBe('amount-grid-lines');
        expect(amountGridLineSeries?.type).toBe('custom');
        expect(amountGridLineSeries?.animation).toBe(true);
        expect(amountGridLineSeries?.animationDurationUpdate).toBe(720);
        expect(amountGridLineSeries?.animationEasingUpdate).toBe('cubicInOut');
        expect(amountGridLineSeries?.data.length).toBe(4);
        amountGridLineSeries?.data.forEach((item, index) => {
            expect(item.value[0]).toBeCloseTo((model.amountAxisMax / 3) * index);
        });
        expect(amountGridLineSeries?.data[0]).toMatchObject({
            id: expect.stringMatching(/^grid:amount:0:axis-/),
            value: [0, 0]
        });
        expect(amountAxisLabelSeries?.id).toBe('amount-axis-labels');
        expect(amountAxisLabelSeries?.type).toBe('custom');
        expect(amountAxisLabelSeries?.animationDurationUpdate).toBe(720);
        expect(amountAxisLabelSeries?.animationEasingUpdate).toBe('cubicInOut');
        expect(amountAxisLabelSeries?.data.length).toBe(4);
        amountAxisLabelSeries?.data.forEach((item, index) => {
            expect(item.value[0]).toBeCloseTo((model.amountAxisMax / 3) * index);
        });
        expect(amountAxisLabelSeries?.data[0]).toMatchObject({
            id: expect.stringMatching(/^grid-label:amount:0:axis-/),
            value: [0, 0],
            label: {
                formatter: '¥0.00',
                rotate: 0
            }
        });
        expect(budgetSeries?.barWidth).toBe(spentSeries?.barWidth);
        expect(primaryRingSeries?.['id']).toBe('primary-ring');
        expect(primaryRingSeries?.['type']).toBe('custom');
        expect(primaryRingSeries?.['animation']).toBe(true);
        expect(primaryRingSeries?.['animationTypeUpdate']).toBe('transition');
        expect(primaryRingSeries?.['animationDurationUpdate']).toBe(720);
        expect(primaryRingSeries?.['animationEasingUpdate']).toBe('cubicInOut');
        expect(budgetSeries?.id).toBe('budget-bars');
        expect(budgetSeries?.type).toBe('custom');
        expect(budgetSeries?.animation).toBe(true);
        expect(budgetSeries?.animationDurationUpdate).toBe(720);
        expect(budgetSeries?.animationEasingUpdate).toBe('cubicInOut');
        expect(spentSeries?.id).toBe('spent-bars');
        expect(spentSeries?.type).toBe('custom');
        expect(spentSeries?.animation).toBe(true);
        expect(spentSeries?.animationDurationUpdate).toBe(720);
        expect(spentSeries?.animationEasingUpdate).toBe('cubicInOut');
        expect(primaryRingSeries?.['universalTransition']).toBeUndefined();
        expect((budgetSeries as Record<string, unknown> | undefined)?.['universalTransition']).toBeUndefined();
        expect((spentSeries as Record<string, unknown> | undefined)?.['universalTransition']).toBeUndefined();
        expect(option.series.find(item => item['name'] === '执行度')).toBeUndefined();
        expect((primaryLabelSeries as Record<string, unknown> | undefined)?.['universalTransition']).toBeUndefined();
        expect((secondaryLabelSeries as Record<string, unknown> | undefined)?.['universalTransition']).toBeUndefined();
        expect(primaryLabelSeries?.animationDurationUpdate).toBe(720);
        expect(secondaryLabelSeries?.animationDurationUpdate).toBe(720);
        expect(option.graphic[0]).toMatchObject({
            id: 'budget-history-center-group',
            transition: ['scaleX', 'scaleY'],
            enterFrom: { scaleX: 0.86, scaleY: 0.86 },
            updateAnimation: { duration: 650, easing: 'cubicInOut' }
        });
        expect(option.graphic[0]?.children?.[0]).toMatchObject({
            id: 'budget-history-center-label',
            y: -17,
            transition: ['y'],
            enterFrom: { y: -11, style: { opacity: 0 } }
        });
        expect(option.graphic[0]?.children?.[1]).toMatchObject({
            id: 'budget-history-center-value',
            y: 0,
            transition: ['y'],
            enterFrom: { y: 8, style: { opacity: 0 } }
        });
        expectNoLeaveTransition(option.graphic[0]);
        expectNoUnsupportedElementTransition(option.graphic[0]);
        expect(option.polar[1]?.radius).toStrictEqual(['80%', '86%']);
        expect(option.polar[2]?.radius).toStrictEqual(['88%', '96%']);
        expect(primaryLabelSeries?.polarIndex).toBe(2);
        expect(option.polar[3]?.radius).toStrictEqual(['20%', '74%']);
        expect(secondaryLabelSeries?.polarIndex).toBe(3);
        expect(primaryLabelSeries?.data.map(item => item.label.formatter)).toStrictEqual(['餐饮', '交通']);
        expect(primaryLabelSeries?.data[0]?.label.width).toBe(BUDGET_HISTORY_CHART_CONFIG.PRIMARY_LABEL_TRUNCATE_WIDTH);
        expect(primaryLabelSeries?.data[0]?.label.overflow).toBe('truncate');
        expect(Math.abs(primaryLabelSeries?.data[0]?.label.rotate ?? 0)).toBeLessThanOrEqual(90);
        expect(Math.abs(primaryLabelSeries?.data[1]?.label.rotate ?? 0)).toBeLessThanOrEqual(90);
        expect(primaryLabelSeries?.data[0]?.value[0]).toBeGreaterThan(0);
        expect(primaryLabelSeries?.data[0]?.value[1]).toBeGreaterThanOrEqual(0);

        expect(spentSeries!.data[0]?.value[1]).toBe(620);
        expect(spentSeries!.data[0]!.value[3] - spentSeries!.data[0]!.value[2])
            .toBeCloseTo(budgetSeries!.data[0]!.value[3] - budgetSeries!.data[0]!.value[2]);
        expect(spentSeries!.data[0]).not.toHaveProperty('label');
        const renderedBudgetBar = budgetSeries!.renderItem!(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            {
                coord: ([radius, angle]) => [
                    100 + (Number(radius) * Math.cos((Number(angle) * Math.PI) / 180)),
                    100 + (Number(radius) * Math.sin((Number(angle) * Math.PI) / 180))
                ]
            }
        );
        const renderedAmountGridLine = amountGridLineSeries!.renderItem!(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            {
                coord: ([radius]) => [
                    100 + Number(radius),
                    100
                ]
            }
        );
        const renderedAmountAxisLabel = amountAxisLabelSeries!.renderItem!(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            {
                coord: ([radius]) => [
                    100 + Number(radius),
                    100
                ]
            }
        );
        expect(renderedBudgetBar).toMatchObject({
            type: 'sector',
            id: expect.stringContaining(':budget'),
            updateAnimation: { duration: 720, easing: 'cubicInOut' },
            keyframeAnimation: expect.objectContaining({
                duration: 720,
                easing: 'cubicInOut',
                keyframes: expect.arrayContaining([
                    expect.objectContaining({
                        shape: expect.objectContaining({ r0: expect.any(Number), r: expect.any(Number) }),
                        style: expect.objectContaining({ opacity: expect.any(Number) })
                    })
                ])
            })
        });
        expect(renderedBudgetBar).not.toHaveProperty('transition');
        expectNoLeaveTransition(renderedBudgetBar);
        expectNoUnsupportedElementTransition(renderedBudgetBar);
        expect(renderedAmountGridLine).toMatchObject({
            type: 'circle',
            id: expect.stringContaining('grid:amount:'),
            updateAnimation: { duration: 720, easing: 'cubicInOut' },
            style: expect.objectContaining({
                fill: 'transparent',
                lineDash: [4, 4],
                opacity: expect.any(Number)
            }),
            keyframeAnimation: expect.objectContaining({
                duration: 720,
                easing: 'cubicInOut',
                keyframes: expect.arrayContaining([
                    expect.objectContaining({
                        shape: expect.objectContaining({ r: expect.any(Number) }),
                        style: expect.objectContaining({ opacity: expect.any(Number) })
                    })
                ])
            })
        });
        expectNoLeaveTransition(renderedAmountGridLine);
        expectNoUnsupportedElementTransition(renderedAmountGridLine);
        expect(renderedAmountAxisLabel).toMatchObject({
            type: 'text',
            id: expect.stringContaining('grid-label:amount:'),
            x: 100,
            rotation: 0,
            updateAnimation: { duration: 720, easing: 'cubicInOut' },
            style: expect.objectContaining({
                text: '¥0.00',
                align: 'center',
                verticalAlign: 'bottom',
                opacity: expect.any(Number)
            }),
            keyframeAnimation: expect.objectContaining({
                duration: 720,
                easing: 'cubicInOut',
                keyframes: expect.arrayContaining([
                    expect.objectContaining({
                        x: 100,
                        y: expect.any(Number),
                        style: expect.objectContaining({ opacity: expect.any(Number) })
                    })
                ])
            })
        });
        expect(renderedAmountAxisLabel?.['transition']).toStrictEqual(['x', 'y']);
        expect(renderedAmountAxisLabel?.['transition']).not.toContain('opacity');
        expectNoLeaveTransition(renderedAmountAxisLabel);
        expectNoUnsupportedElementTransition(renderedAmountAxisLabel);
        expect(secondaryLabelSeries!.data[0]?.value[0]).toBeCloseTo(model.slots[0]!.labelAnchorAmount + (model.amountAxisInterval * 0.18));
        expect(secondaryLabelSeries!.data[2]?.value[0]).toBeCloseTo(model.slots[2]!.labelAnchorAmount + (model.amountAxisInterval * 0.18));
        expect(secondaryLabelSeries!.data[0]?.value[0]).toBeLessThan(model.amountAxisMax);
        expect(secondaryLabelSeries!.data[0]?.value[1]).toBeGreaterThanOrEqual(0);
        expect(secondaryLabelSeries!.data[0]?.label?.formatter).toBe('早餐');
        expect(secondaryLabelSeries!.data[0]?.label?.rotate).toBeLessThan(0);
        expect(secondaryLabelSeries!.data[1]?.label?.formatter).toBe('午餐');
        expect(secondaryLabelSeries!.data[1]?.label?.rotate).toBeGreaterThanOrEqual(0);
        expect(secondaryLabelSeries!.data[2]?.label?.formatter).toBe('打车');
        expect(Math.abs(secondaryLabelSeries!.data[2]?.label?.rotate || 0)).toBeLessThanOrEqual(90);
        expect(Math.abs(secondaryLabelSeries!.data[1]?.label?.rotate || 0)).toBeLessThan(90);
    });

    test('buildHistoricalPolarChartOption restores primary ring as an appear animation after primary preview hides it', () => {
        const labelAnimationState = createHistoricalLabelAnimationState();
        const model = buildHistoricalPolarChartModel(SAMPLE_POINTS, buildSelection());
        const baseOptions = {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            labelAnimationState
        };

        buildHistoricalPolarChartOption(model, { ...baseOptions, showPrimaryRing: true });
        buildHistoricalPolarChartOption(model, { ...baseOptions, showPrimaryRing: false });
        const restoredOption = buildHistoricalPolarChartOption(model, { ...baseOptions, showPrimaryRing: true }) as {
            series: Array<{
                name?: string;
                renderItem?: (
                    params: { dataIndex: number; coordSys: { cx: number; cy: number } },
                    api: { coord: (value: number[]) => number[] }
                ) => {
                    shape?: { r0?: number; r?: number };
                    keyframeAnimation?: {
                        keyframes?: Array<{ shape?: { startAngle?: number; endAngle?: number } }>;
                    };
                };
            }>;
        };
        const primaryRing = restoredOption.series.find(item => item.name === 'primary-ring');
        const renderedRing = primaryRing?.renderItem?.(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            {
                coord: ([radius, angle]) => [
                    100 + ((50 + (Number(radius) * 50)) * Math.cos((Number(angle) * Math.PI) / 180)),
                    100 + ((50 + (Number(radius) * 50)) * Math.sin((Number(angle) * Math.PI) / 180))
                ]
            }
        );
        const firstKeyframe = renderedRing?.keyframeAnimation?.keyframes?.[0];
        const firstSpan = Math.abs((firstKeyframe?.shape?.endAngle ?? 0) - (firstKeyframe?.shape?.startAngle ?? 0));

        expect(firstSpan).toBeLessThan(0.01);
        expect(renderedRing?.shape?.r0).toBeGreaterThan(45);
        expect(renderedRing?.shape?.r).toBeGreaterThan(renderedRing?.shape?.r0 ?? 0);
    });

    test('buildHistoricalPolarChartOption animates bar height when filtered data changes the amount axis range', () => {
        const labelAnimationState = createHistoricalLabelAnimationState();
        const rangePoints: HistoricalCategoryChartPoint[] = [
            {
                category: '早餐',
                primaryCategory: '餐饮',
                secondaryCategory: '早餐',
                budgetAmountCents: 1000,
                spentAmountCents: 800,
                executionRate: 80,
                color: '#5470c6',
                groupOrder: 1,
                itemOrder: 1
            },
            {
                category: '大额',
                primaryCategory: '居住',
                secondaryCategory: '大额',
                budgetAmountCents: 3000,
                spentAmountCents: 3000,
                executionRate: 100,
                color: '#91cc75',
                groupOrder: 2,
                itemOrder: 1
            }
        ];
        const baseOptions = {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true,
            labelAnimationState
        };
        const fullModel = buildHistoricalPolarChartModel(rangePoints, syncHistoricalLegendSelection(rangePoints));
        buildHistoricalPolarChartOption(fullModel, baseOptions);
        const filteredModel = buildHistoricalPolarChartModel(
            rangePoints,
            syncHistoricalLegendSelection(rangePoints, { '居住::大额': false })
        );
        const filteredOption = buildHistoricalPolarChartOption(filteredModel, baseOptions) as {
            series: Array<{
                name?: string;
                data?: Array<{ id?: string; value?: [number, number] }>;
                renderItem?: (
                    params: { dataIndex: number; coordSys: { cx: number; cy: number } },
                    api: { coord: (value: number[]) => number[] }
                ) => {
                    keyframeAnimation?: {
                        keyframes?: Array<{
                            x?: number;
                            y?: number;
                            shape?: { r?: number; startAngle?: number };
                            style?: { opacity?: number };
                        }>;
                    };
                };
            }>;
        };
        const budgetSeries = filteredOption.series.find(item => item.name === '预算金额');
        const secondaryLabelSeries = filteredOption.series.find(item => item.name === 'secondary-labels');
        const amountGridLineSeries = filteredOption.series.find(item => item.name === 'amount-grid-lines');
        const amountAxisLabelSeries = filteredOption.series.find(item => item.name === 'amount-axis-labels');
        const polarCoord = ([radius, angle]: number[]) => {
            const pixelRadius = (Number(radius) / filteredModel.amountAxisMax) * 100;
            return [
                100 + (pixelRadius * Math.cos((Number(angle) * Math.PI) / 180)),
                100 + (pixelRadius * Math.sin((Number(angle) * Math.PI) / 180))
            ];
        };
        const renderedBar = budgetSeries?.renderItem?.(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            { coord: polarCoord }
        );
        const renderedLabel = secondaryLabelSeries?.renderItem?.(
            { dataIndex: 0, coordSys: { cx: 100, cy: 100 } },
            { coord: polarCoord }
        );
        const renderedGridIds = amountGridLineSeries?.data
            ?.map(item => item.id)
            .filter((id): id is string => Boolean(
                id
                && id.startsWith('grid:amount:')
                && !id.startsWith('grid:amount:0')
            ))
            .slice(0, 3) ?? [];
        const renderedGridLines = renderedGridIds.map(gridId => {
            const gridIndex = amountGridLineSeries?.data?.findIndex(item => item.id === gridId) ?? -1;
            const renderedGridLine = gridIndex >= 0
                ? amountGridLineSeries?.renderItem?.(
                    { dataIndex: gridIndex, coordSys: { cx: 100, cy: 100 } },
                    { coord: polarCoord }
                )
                : undefined;
            const gridKeyframes = renderedGridLine?.keyframeAnimation?.keyframes ?? [];

            return {
                gridId,
                gridIndex,
                firstRadius: gridKeyframes[0]?.shape?.r ?? 0,
                lastRadius: gridKeyframes[gridKeyframes.length - 1]?.shape?.r ?? 0
            };
        });
        const renderedAmountLabels = renderedGridIds.map(gridId => {
            const labelId = gridId.replace('grid:amount:', 'grid-label:amount:');
            const labelIndex = amountAxisLabelSeries?.data?.findIndex(item => item.id === labelId) ?? -1;
            const renderedAmountLabel = labelIndex >= 0
                ? amountAxisLabelSeries?.renderItem?.(
                    { dataIndex: labelIndex, coordSys: { cx: 100, cy: 100 } },
                    { coord: polarCoord }
                )
                : undefined;
            const labelKeyframes = renderedAmountLabel?.keyframeAnimation?.keyframes ?? [];

            return {
                labelId,
                labelIndex,
                firstY: labelKeyframes[0]?.y ?? 0,
                lastY: labelKeyframes[labelKeyframes.length - 1]?.y ?? 0,
                rotation: (renderedAmountLabel as Record<string, unknown> | undefined)?.['rotation']
            };
        });
        const keyframes = renderedBar?.keyframeAnimation?.keyframes ?? [];
        const firstRadius = keyframes[0]?.shape?.r ?? 0;
        const lastRadius = keyframes[keyframes.length - 1]?.shape?.r ?? 0;
        const labelKeyframes = renderedLabel?.keyframeAnimation?.keyframes ?? [];
        const firstLabelRadius = Math.hypot(
            (labelKeyframes[0]?.x ?? 100) - 100,
            (labelKeyframes[0]?.y ?? 100) - 100
        );
        const lastLabelRadius = Math.hypot(
            (labelKeyframes[labelKeyframes.length - 1]?.x ?? 100) - 100,
            (labelKeyframes[labelKeyframes.length - 1]?.y ?? 100) - 100
        );

        expect(filteredModel.amountAxisMax).toBeLessThan(fullModel.amountAxisMax);
        expect(lastRadius).toBeGreaterThan(firstRadius);
        expect(lastLabelRadius).toBeGreaterThan(firstLabelRadius);
        expect(amountGridLineSeries?.data?.length).toBe(5);
        expect(amountAxisLabelSeries?.data?.length).toBe(8);
        expect(new Set(amountGridLineSeries?.data?.map(item => item.id)).size).toBe(amountGridLineSeries?.data?.length);
        expect(new Set(amountAxisLabelSeries?.data?.map(item => item.id)).size).toBe(amountAxisLabelSeries?.data?.length);
        amountGridLineSeries?.data?.slice(0, 4).forEach((item, index) => {
            expect(item.value?.[0]).toBeCloseTo((filteredModel.amountAxisMax / 3) * index);
        });
        amountAxisLabelSeries?.data?.slice(0, 4).forEach((item, index) => {
            expect(item.value?.[0]).toBeCloseTo((filteredModel.amountAxisMax / 3) * index);
        });
        expect(renderedGridLines.every(item => item.gridIndex >= 0)).toBe(true);
        expect(renderedGridLines.every(item => item.lastRadius > item.firstRadius)).toBe(true);
        expect(renderedAmountLabels.every(item => item.labelIndex >= 0)).toBe(true);
        renderedAmountLabels.forEach(item => {
            const renderedAmountLabel = amountAxisLabelSeries?.renderItem?.(
                { dataIndex: item.labelIndex, coordSys: { cx: 100, cy: 100 } },
                { coord: polarCoord }
            );
            const amountLabelKeyframes = renderedAmountLabel?.keyframeAnimation?.keyframes ?? [];

            expect(amountLabelKeyframes[0]?.style?.opacity).toBe(0);
            expect(amountLabelKeyframes.at(-1)?.style?.opacity).toBe(1);
        });
        expect(renderedAmountLabels.every(item => item.rotation === 0)).toBe(true);
        const leavingGridIndex = (amountGridLineSeries?.data?.length ?? 0) - 1;
        const leavingGrid = amountGridLineSeries?.renderItem?.(
            { dataIndex: leavingGridIndex, coordSys: { cx: 100, cy: 100 } },
            { coord: polarCoord }
        );
        const leavingGridKeyframes = leavingGrid?.keyframeAnimation?.keyframes ?? [];
        expect(amountGridLineSeries?.data?.[leavingGridIndex]?.id).toMatch(/^grid:amount:3(?::|$)/);
        expect(leavingGridKeyframes.at(-1)?.shape?.r).toBeGreaterThan(leavingGridKeyframes[0]?.shape?.r ?? 0);
        expect(leavingGridKeyframes[0]?.style?.opacity).toBe(1);
        expect(leavingGridKeyframes.at(-1)?.style?.opacity).toBe(0);
        amountAxisLabelSeries?.data?.slice(4).forEach((_, offset) => {
            const renderedGhostLabel = amountAxisLabelSeries.renderItem?.(
                { dataIndex: 4 + offset, coordSys: { cx: 100, cy: 100 } },
                { coord: polarCoord }
            );
            const ghostLabelKeyframes = renderedGhostLabel?.keyframeAnimation?.keyframes ?? [];

            expect(ghostLabelKeyframes[0]?.style?.opacity).toBe(1);
            expect(ghostLabelKeyframes.at(-1)?.style?.opacity).toBe(0);
        });
    });

    test('buildHistoricalPolarChartOption keeps disappearing bars on the current radius scale', () => {
        const labelAnimationState = createHistoricalLabelAnimationState();
        const rangePoints: HistoricalCategoryChartPoint[] = [
            {
                category: '早餐',
                primaryCategory: '餐饮',
                secondaryCategory: '早餐',
                budgetAmountCents: 1000,
                spentAmountCents: 800,
                executionRate: 80,
                color: '#5470c6',
                groupOrder: 1,
                itemOrder: 1
            },
            {
                category: '大额',
                primaryCategory: '居住',
                secondaryCategory: '大额',
                budgetAmountCents: 3000,
                spentAmountCents: 3000,
                executionRate: 100,
                color: '#91cc75',
                groupOrder: 2,
                itemOrder: 1
            }
        ];
        const baseOptions = {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true,
            labelAnimationState
        };
        const fullModel = buildHistoricalPolarChartModel(rangePoints, syncHistoricalLegendSelection(rangePoints));
        buildHistoricalPolarChartOption(fullModel, baseOptions);
        const filteredModel = buildHistoricalPolarChartModel(
            rangePoints,
            syncHistoricalLegendSelection(rangePoints, { '居住::大额': false })
        );
        const filteredOption = buildHistoricalPolarChartOption(filteredModel, baseOptions) as {
            series: Array<{
                name?: string;
                data?: Array<{ id?: string }>;
                renderItem?: (
                    params: { dataIndex: number; coordSys: { cx: number; cy: number } },
                    api: { coord: (value: number[]) => number[] }
                ) => {
                    keyframeAnimation?: {
                        keyframes?: Array<{
                            shape?: { r?: number };
                        }>;
                    };
                };
            }>;
        };
        const budgetSeries = filteredOption.series.find(item => item.name === '预算金额');
        const leavingIndex = budgetSeries?.data?.findIndex(item => item.id === '居住::大额:budget') ?? -1;
        const polarCoord = ([radius, angle]: number[]) => {
            const pixelRadius = (Number(radius) / filteredModel.amountAxisMax) * 100;
            return [
                100 + (pixelRadius * Math.cos((Number(angle) * Math.PI) / 180)),
                100 + (pixelRadius * Math.sin((Number(angle) * Math.PI) / 180))
            ];
        };
        const renderedLeavingBar = leavingIndex >= 0
            ? budgetSeries?.renderItem?.(
                { dataIndex: leavingIndex, coordSys: { cx: 100, cy: 100 } },
                { coord: polarCoord }
            )
            : undefined;
        const keyframes = renderedLeavingBar?.keyframeAnimation?.keyframes ?? [];
        const firstRadius = keyframes[0]?.shape?.r ?? 0;
        const lastRadius = keyframes[keyframes.length - 1]?.shape?.r ?? 0;

        expect(filteredModel.amountAxisMax).toBeLessThan(fullModel.amountAxisMax);
        expect(firstRadius).toBeLessThanOrEqual(101);
        expect(lastRadius).toBeLessThan(firstRadius);
    });

    test('buildHistoricalPolarChartOption unwraps sector keyframes across the canvas angle seam', () => {
        const labelAnimationState = createHistoricalLabelAnimationState();
        const seamPoints: HistoricalCategoryChartPoint[] = Array.from({ length: 18 }, (_, index) => ({
            category: `项${index}`,
            primaryCategory: '全部',
            secondaryCategory: `项${index}`,
            budgetAmountCents: 100 + index,
            spentAmountCents: 50 + index,
            executionRate: 50,
            color: '#5470c6',
            groupOrder: 1,
            itemOrder: index + 1
        }));
        const baseOptions = {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true,
            labelAnimationState
        };
        buildHistoricalPolarChartOption(
            buildHistoricalPolarChartModel(seamPoints, syncHistoricalLegendSelection(seamPoints)),
            baseOptions
        );
        const filteredModel = buildHistoricalPolarChartModel(
            seamPoints,
            syncHistoricalLegendSelection(seamPoints, { '全部::项0': false })
        );
        const filteredOption = buildHistoricalPolarChartOption(filteredModel, baseOptions) as {
            series: Array<{
                name?: string;
                data?: Array<{ id?: string }>;
                renderItem?: (
                    params: { dataIndex: number; coordSys: { cx: number; cy: number } },
                    api: { coord: (value: number[]) => number[] }
                ) => {
                    keyframeAnimation?: {
                        keyframes?: Array<{
                            shape?: { startAngle?: number };
                        }>;
                    };
                };
            }>;
        };
        const budgetSeries = filteredOption.series.find(item => item.name === '预算金额');
        const seamIndex = budgetSeries?.data?.findIndex(item => item.id === '全部::项9:budget') ?? -1;
        const polarCoord = ([radius, angle]: number[]) => {
            const pixelRadius = (Number(radius) / filteredModel.amountAxisMax) * 100;
            return [
                100 + (pixelRadius * Math.cos((Number(angle) * Math.PI) / 180)),
                100 + (pixelRadius * Math.sin((Number(angle) * Math.PI) / 180))
            ];
        };
        const renderedSeamBar = seamIndex >= 0
            ? budgetSeries?.renderItem?.(
                { dataIndex: seamIndex, coordSys: { cx: 100, cy: 100 } },
                { coord: polarCoord }
            )
            : undefined;
        const keyframes = renderedSeamBar?.keyframeAnimation?.keyframes ?? [];
        const maxStep = keyframes.slice(1).reduce((max, keyframe, index) => {
            const previousAngle = keyframes[index]?.shape?.startAngle ?? 0;
            const currentAngle = keyframe.shape?.startAngle ?? 0;

            return Math.max(max, Math.abs(currentAngle - previousAngle));
        }, 0);

        expect(seamIndex).toBeGreaterThanOrEqual(0);
        expect(maxStep).toBeLessThan(Math.PI / 2);
    });

    test('buildHistoricalPolarChartOption keeps empty and stale custom-render paths safe', () => {
        const emptyOption = buildHistoricalPolarChartOption(buildHistoricalPolarChartModel([]), {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true
        }) as {
            tooltip: { show?: boolean };
            series: Array<Record<string, unknown>>;
        };
        const populatedOption = buildHistoricalPolarChartOption(buildHistoricalPolarChartModel(SAMPLE_POINTS), {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true
        }) as {
            series: Array<{
                name?: string;
                data?: Array<{ id?: string }>;
                renderItem?: (
                    params: { dataIndex: number },
                    api: { coord: (value: number[]) => number[] }
                ) => Record<string, unknown>;
            }>;
        };
        const secondaryLabelSeries = populatedOption.series.find(item => item.name === 'secondary-labels');

        expect(emptyOption.tooltip.show).toBe(false);
        expect(emptyOption.series).toStrictEqual([]);
        expect(secondaryLabelSeries?.renderItem?.({ dataIndex: 999 }, { coord: () => [10, 20] })).toMatchObject({
            type: 'text',
            opacity: 0,
            style: expect.objectContaining({ text: '' })
        });
        expect(secondaryLabelSeries?.renderItem?.({ dataIndex: 0 }, { coord: () => [Number.NaN, 20] })).toMatchObject({
            type: 'text',
            opacity: 0,
            style: expect.objectContaining({ text: '' })
        });
        expect(secondaryLabelSeries?.renderItem?.({ dataIndex: 0 }, { coord: () => [10, 20] })).toMatchObject({
            type: 'text',
            id: expect.stringContaining(':label'),
            name: expect.stringContaining(':label'),
            z2: expect.any(Number),
            transition: expect.arrayContaining(['x', 'y']),
            enterFrom: { style: { opacity: 0 } },
            updateAnimation: { duration: 720, easing: 'cubicInOut' },
            style: expect.objectContaining({
                text: expect.any(String),
                opacity: 1
            }),
            keyframeAnimation: expect.objectContaining({
                duration: 720,
                easing: 'cubicInOut',
                keyframes: expect.arrayContaining([
                    expect.objectContaining({ style: { opacity: 0 } }),
                    expect.objectContaining({ style: { opacity: 1 } })
                ])
            })
        });
        const renderedSafeLabel = secondaryLabelSeries?.renderItem?.({ dataIndex: 0 }, { coord: () => [10, 20] });
        expect(renderedSafeLabel?.['transition']).not.toContain('rotation');
        expect(renderedSafeLabel?.['transition']).not.toContain('scaleX');
        expect(renderedSafeLabel?.['transition']).not.toContain('scaleY');
        expect(renderedSafeLabel?.['transition']).not.toContain('style');
        expect(renderedSafeLabel?.['transition']).not.toContain('opacity');
        expectNoLeaveTransition(renderedSafeLabel);
        expectNoUnsupportedElementTransition(renderedSafeLabel);
    });

    test('buildHistoricalPolarChartOption keeps custom label update animations without leave transitions after filtering', () => {
        const labelAnimationState = createHistoricalLabelAnimationState();
        const baseOptions = {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true,
            labelAnimationState
        };
        const fullOption = buildHistoricalPolarChartOption(
            buildHistoricalPolarChartModel(SAMPLE_POINTS, buildSelection()),
            baseOptions
        ) as {
            graphic: Array<Record<string, unknown>>;
            series: Array<Record<string, unknown>>;
        };
        const filteredOption = buildHistoricalPolarChartOption(
            buildHistoricalPolarChartModel(
                SAMPLE_POINTS,
                buildSelection({
                    '餐饮::午餐': false,
                    '交通::地铁': false
                })
            ),
            baseOptions
        ) as {
            graphic: Array<Record<string, unknown>>;
            series: Array<{
                name?: string;
                data?: Array<{ id?: string }>;
                renderItem?: (
                    params: { dataIndex: number },
                    api: { coord: (value: number[]) => number[] }
                ) => Record<string, unknown>;
            }>;
        };
        const secondaryLabelSeries = filteredOption.series.find(item => item.name === 'secondary-labels');
        const hiddenLabelIndex = secondaryLabelSeries?.data?.findIndex(
            (item: { id?: string }) => item.id === '餐饮::午餐:label'
        ) ?? -1;
        const renderedLabel = secondaryLabelSeries?.renderItem?.(
            { dataIndex: 0 },
            {
                coord: value => [Number(value[1] ?? 0), Number(value[0] ?? 0)]
            }
        );
        const renderedHiddenLabel = hiddenLabelIndex >= 0
            ? secondaryLabelSeries?.renderItem?.(
                { dataIndex: hiddenLabelIndex },
                { coord: value => [Number(value[1] ?? 0), Number(value[0] ?? 0)] }
            )
            : undefined;

        [...fullOption.graphic, ...fullOption.series, ...filteredOption.graphic, ...filteredOption.series].forEach(
            item => {
                expectNoLeaveTransition(item);
                expectNoUnsupportedElementTransition(item);
            }
        );
        expect(renderedLabel).toMatchObject({
            type: 'text',
            id: expect.stringContaining(':label'),
            name: expect.stringContaining(':label'),
            z2: expect.any(Number),
            transition: expect.arrayContaining(['x', 'y']),
            enterFrom: { style: { opacity: 0 } },
            updateAnimation: { duration: 720, easing: 'cubicInOut' },
            style: expect.objectContaining({
                text: expect.any(String),
                opacity: 1
            }),
            keyframeAnimation: expect.objectContaining({
                duration: 720,
                easing: 'cubicInOut',
                keyframes: expect.arrayContaining([
                    expect.objectContaining({ style: { opacity: 1 } })
                ])
            })
        });
        expect(renderedLabel).not.toHaveProperty('scaleX');
        expect(renderedLabel).not.toHaveProperty('scaleY');
        expect(renderedLabel?.['transition']).not.toContain('rotation');
        expect(renderedLabel?.['transition']).not.toContain('scaleX');
        expect(renderedLabel?.['transition']).not.toContain('scaleY');
        expect(renderedLabel?.['transition']).not.toContain('style');
        expect(renderedLabel?.['transition']).not.toContain('opacity');
        const visibleLabelKeyframes = (renderedLabel?.['keyframeAnimation'] as {
            keyframes?: Array<{ percent?: number; style?: { opacity?: number } }>;
        } | undefined)?.keyframes ?? [];
        expect(visibleLabelKeyframes.map(keyframe => keyframe.percent)).toStrictEqual([0, 0.35, 0.7, 1]);
        expect(visibleLabelKeyframes.every(keyframe => keyframe.style?.opacity === 1)).toBe(true);
        expectNoLeaveTransition(renderedLabel);
        expectNoUnsupportedElementTransition(renderedLabel);
        expect(renderedHiddenLabel).toMatchObject({
            type: 'text',
            id: '餐饮::午餐:label',
            style: expect.objectContaining({
                text: '午餐',
                opacity: 0
            }),
            keyframeAnimation: expect.objectContaining({
                keyframes: expect.arrayContaining([
                    expect.objectContaining({ style: { opacity: 1 } }),
                    expect.objectContaining({ style: { opacity: 0 } })
                ])
            })
        });
        expect(renderedHiddenLabel).not.toHaveProperty('scaleX');
        expect(renderedHiddenLabel).not.toHaveProperty('scaleY');
        expect(renderedHiddenLabel?.['transition']).not.toContain('rotation');
        expect(renderedHiddenLabel?.['transition']).not.toContain('scaleX');
        expect(renderedHiddenLabel?.['transition']).not.toContain('scaleY');
        expect(renderedHiddenLabel?.['transition']).not.toContain('style');
        expect(renderedHiddenLabel?.['transition']).not.toContain('opacity');
        const hiddenLabelKeyframes = (renderedHiddenLabel?.['keyframeAnimation'] as {
            keyframes?: Array<{ x?: number; y?: number; rotation?: number; style?: { opacity?: number } }>;
        } | undefined)?.keyframes ?? [];
        const hiddenLabelLastKeyframe = hiddenLabelKeyframes[hiddenLabelKeyframes.length - 1];
        expect(new Set(hiddenLabelKeyframes.map(item => Number((item.x ?? 0).toFixed(4)))).size).toBe(1);
        expect(new Set(hiddenLabelKeyframes.map(item => Number((item.y ?? 0).toFixed(4)))).size).toBe(1);
        expect(new Set(hiddenLabelKeyframes.map(item => Number((item.rotation ?? 0).toFixed(4)))).size).toBe(1);
        expect(renderedHiddenLabel?.['x']).toBeCloseTo(hiddenLabelLastKeyframe?.x ?? 0);
        expect(renderedHiddenLabel?.['y']).toBeCloseTo(hiddenLabelLastKeyframe?.y ?? 0);
        expectNoLeaveTransition(renderedHiddenLabel);
        expectNoUnsupportedElementTransition(renderedHiddenLabel);
    });

    test('buildHistoricalPolarChartOption fades the primary ring when showPrimaryRing is false', () => {
        const model = buildHistoricalPolarChartModel(SAMPLE_POINTS, buildSelection());
        const option = buildHistoricalPolarChartOption(model, {
            isDarkMode: true,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: false
        }) as {
            radiusAxis: Array<{ axisLabel?: { color?: string } }>;
            series: Array<Record<string, unknown>>;
            tooltip: { formatter: (params: { dataIndex?: number }) => string };
        };

        const primaryRing = option.series.find(item => item['name'] === 'primary-ring') as {
            data?: Array<{ itemStyle?: { opacity?: number; borderWidth?: number } }>;
        } | undefined;
        const primaryLabels = option.series.find(item => item['name'] === 'primary-labels') as {
            data?: unknown[];
        } | undefined;

        expect(primaryRing).toBeDefined();
        expect(primaryRing?.data).toStrictEqual([]);
        expect(primaryLabels).toBeDefined();
        expect(primaryLabels?.data).toStrictEqual([]);
        expect(option.radiusAxis[0]?.axisLabel?.color).toBe('#888');
        expect(option.tooltip.formatter({ dataIndex: 0 })).toContain('预算金额: ¥8.00');
        expect(option.tooltip.formatter({ dataIndex: 0 })).not.toContain('¥800.00');
        expect(option.tooltip.formatter({ dataIndex: 99 })).toBe('');
    });

    test('buildHistoricalPolarChartOption falls back to rgba when the source color is invalid', () => {
        const model = buildHistoricalPolarChartModel([
            {
                category: '其他',
                primaryCategory: '杂项',
                secondaryCategory: '其他',
                budgetAmountCents: 120,
                spentAmountCents: 60,
                executionRate: 50,
                color: 'bad-color',
                groupOrder: 1,
                itemOrder: 1
            }
        ]);
        const option = buildHistoricalPolarChartOption(model, {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: true
        }) as {
            series: Array<Record<string, unknown>>;
        };

        const budgetSeries = option.series.find(item => item['name'] === '预算金额') as {
            data: Array<{ itemStyle: { color: string } }>;
        } | undefined;

        expect(budgetSeries?.data[0]?.itemStyle.color).toBe('rgba(0,0,0,0.28)');
    });
});
