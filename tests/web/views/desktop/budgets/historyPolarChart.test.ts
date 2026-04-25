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
        budgetAmount: 800,
        spentAmount: 620,
        executionRate: 77.5,
        color: '#5470c6',
        groupOrder: 1,
        itemOrder: 1
    },
    {
        category: '午餐',
        primaryCategory: '餐饮',
        secondaryCategory: '午餐',
        budgetAmount: 1500,
        spentAmount: 1300,
        executionRate: 86.7,
        color: '#5470c6',
        groupOrder: 1,
        itemOrder: 2
    },
    {
        category: '地铁',
        primaryCategory: '交通',
        secondaryCategory: '地铁',
        budgetAmount: 500,
        spentAmount: 260,
        executionRate: 52,
        color: '#91cc75',
        groupOrder: 2,
        itemOrder: 1
    },
    {
        category: '打车',
        primaryCategory: '交通',
        secondaryCategory: '打车',
        budgetAmount: 700,
        spentAmount: 820,
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
        expect(model.primaryLabels[0]?.rotate).toBeLessThan(0);
        expect(model.primaryLabels[1]?.rotate).toBeGreaterThan(0);
        expect(model.primaryBands[0]?.startAngle).toBeGreaterThan(model.primaryBands[0]?.endAngle || -Infinity);
        expect(model.amountAxisMax).toBeGreaterThan(1500);
    });

    test('getTangentialTextRotation mirrors left-right labels and only flips the lower 120 degree sector', () => {
        expect(getTangentialTextRotation(45)).toBeCloseTo(-45, 1);
        expect(getTangentialTextRotation(135)).toBeCloseTo(45, 1);
        expect(getTangentialTextRotation(200)).toBeGreaterThan(0);
        expect(getTangentialTextRotation(220)).toBeLessThan(0);
        expect(getTangentialTextRotation(270)).toBeCloseTo(0, 1);
        expect(getTangentialTextRotation(320)).toBeGreaterThan(0);
        expect(getTangentialTextRotation(340)).toBeLessThan(0);
        expect(Math.abs(getTangentialTextRotation(220))).toBeLessThan(90);
        expect(Math.abs(getTangentialTextRotation(320))).toBeLessThan(90);
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
        expect(visibleAfterHide).toHaveLength(1);
        expect(visibleAfterHide[0]?.previous?.polarAngleValue).toBeCloseTo(120);

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
                budgetAmount: 10,
                spentAmount: 5,
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

    test('buildHistoricalPolarChartModel sorts ties by primary category and fallback category label', () => {
        const model = buildHistoricalPolarChartModel([
            {
                category: '香蕉',
                primaryCategory: '餐饮',
                secondaryCategory: '',
                budgetAmount: 100,
                spentAmount: 50,
                executionRate: 50,
                color: '#5470c6',
                groupOrder: 1,
                itemOrder: 1
            },
            {
                category: '苹果',
                primaryCategory: '餐饮',
                secondaryCategory: '',
                budgetAmount: 120,
                spentAmount: 60,
                executionRate: 50,
                color: '#5470c6',
                groupOrder: 1,
                itemOrder: 1
            },
            {
                category: '地铁',
                primaryCategory: '交通',
                secondaryCategory: '',
                budgetAmount: 90,
                spentAmount: 40,
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
        const buildSinglePointModel = (budgetAmount: number) => buildHistoricalPolarChartModel([
            {
                category: '单项',
                primaryCategory: '测试',
                secondaryCategory: '单项',
                budgetAmount,
                spentAmount: budgetAmount / 2,
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
            radiusAxis: Array<{
                max?: number;
                interval?: number;
                axisLabel?: { margin?: number; formatter?: (value: number) => string };
            }>;
            series: Array<Record<string, unknown>>;
        };

        const primaryRingSeries = option.series.find(item => item['name'] === 'primary-ring') as Record<string, unknown> | undefined;
        const primaryLabelSeries = option.series.find(item => item['name'] === 'primary-labels') as {
            polarIndex?: number;
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
            barWidth?: number;
        } | undefined;
        const spentSeries = option.series.find(item => item['name'] === '已花费') as {
            barWidth?: number;
            data: Array<{ value: number }>;
        } | undefined;
        const secondaryLabelSeries = option.series.find(item => item['name'] === 'secondary-labels') as {
            polarIndex?: number;
            data: Array<{
                value: [number, number];
                label: {
                    formatter: string;
                    rotate: number;
                };
            }>;
        } | undefined;
        const lineSeries = option.series.find(item => item['name'] === '执行度') as {
            smooth?: boolean;
            data: Array<{ value: number }>;
        } | undefined;

        expect(primaryRingSeries).toBeDefined();
        expect(primaryLabelSeries).toBeDefined();
        expect(budgetSeries).toBeDefined();
        expect(spentSeries).toBeDefined();
        expect(secondaryLabelSeries).toBeDefined();
        expect(lineSeries).toBeDefined();

        expect(option.radiusAxis[1]?.max).toBe(125);
        expect(option.radiusAxis[1]?.interval).toBe(25);
        expect(option.radiusAxis[0]?.axisLabel?.margin).toBe(10);
        expect(option.radiusAxis[0]?.axisLabel?.formatter?.(120)).toBe('¥120.00');
        expect(budgetSeries?.barWidth).toBe(spentSeries?.barWidth);
        expect(lineSeries?.smooth).toBe(true);
        expect(primaryRingSeries?.['label']).toMatchObject({ show: false });
        expect(primaryRingSeries?.['radius']).toStrictEqual(['80%', '86%']);
        expect(option.polar[2]?.radius).toStrictEqual(['88%', '96%']);
        expect(primaryLabelSeries?.polarIndex).toBe(2);
        expect(option.polar[3]?.radius).toStrictEqual(['20%', '74%']);
        expect(secondaryLabelSeries?.polarIndex).toBe(3);
        expect(primaryLabelSeries?.data.map(item => item.label.formatter)).toStrictEqual(['餐饮', '交通']);
        expect(primaryLabelSeries?.data[0]?.label.width).toBe(BUDGET_HISTORY_CHART_CONFIG.PRIMARY_LABEL_TRUNCATE_WIDTH);
        expect(primaryLabelSeries?.data[0]?.label.overflow).toBe('truncate');
        expect(primaryLabelSeries?.data[0]?.label.rotate).toBeLessThan(0);
        expect(primaryLabelSeries?.data[1]?.label.rotate).toBeGreaterThan(0);
        expect(primaryLabelSeries?.data[0]?.value[0]).toBeGreaterThan(0);
        expect(primaryLabelSeries?.data[0]?.value[1]).toBeGreaterThanOrEqual(0);

        expect(spentSeries!.data[0]?.value).toBe(620);
        expect(secondaryLabelSeries!.data[0]?.value[0]).toBeGreaterThan(800);
        expect(secondaryLabelSeries!.data[0]?.value[0]).toBeLessThan(model.amountAxisMax);
        expect(secondaryLabelSeries!.data[0]?.value[1]).toBeGreaterThanOrEqual(0);
        expect(secondaryLabelSeries!.data[0]?.label?.formatter).toBe('早餐');
        expect(secondaryLabelSeries!.data[0]?.label?.rotate).toBeLessThan(0);
        expect(secondaryLabelSeries!.data[1]?.label?.formatter).toBe('午餐');
        expect(secondaryLabelSeries!.data[1]?.label?.rotate).toBeGreaterThanOrEqual(0);
        expect(secondaryLabelSeries!.data[2]?.label?.formatter).toBe('打车');
        expect(secondaryLabelSeries!.data[2]?.label?.rotate).toBeGreaterThan(0);
        expect(Math.abs(secondaryLabelSeries!.data[1]?.label?.rotate || 0)).toBeLessThan(90);
        expect(lineSeries!.data[2]?.value).toBeCloseTo(117.1, 1);
    });

    test('buildHistoricalPolarChartOption omits the primary ring when showPrimaryRing is false', () => {
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

        expect(option.series[0]?.['name']).not.toBe('主分类环');
        expect(option.series.some(item => item['name'] === 'primary-labels')).toBe(false);
        expect(option.radiusAxis[0]?.axisLabel?.color).toBe('#888');
        expect(option.tooltip.formatter({ dataIndex: 0 })).toContain('预算金额: ¥800.00');
        expect(option.tooltip.formatter({ dataIndex: 99 })).toBe('');
    });

    test('buildHistoricalPolarChartOption falls back to rgba when the source color is invalid', () => {
        const model = buildHistoricalPolarChartModel([
            {
                category: '其他',
                primaryCategory: '杂项',
                secondaryCategory: '其他',
                budgetAmount: 120,
                spentAmount: 60,
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
