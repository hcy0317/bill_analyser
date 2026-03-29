import { describe, expect, test } from '@jest/globals';

import type { HistoricalCategoryChartPoint, HistoricalLegendSelection } from '@/views/desktop/budgets/historyPolarChart.ts';
import {
    buildHistoricalPolarChartModel,
    buildHistoricalPolarChartOption,
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
        expect(model.primaryBands).toStrictEqual([
            expect.objectContaining({ key: '餐饮', state: 'all' }),
            expect.objectContaining({ key: '交通', state: 'all' })
        ]);
        expect(model.primaryPadAngle).toBeGreaterThan(0);
        expect(model.primaryLabelGlyphs).toHaveLength(4);
        expect(model.primaryLabelGlyphs.every(glyph => {
            const radius = Math.hypot(glyph.x - 50, glyph.y - 46);
            return radius > 43;
        })).toBe(true);
        expect(model.primaryBands[0]?.startAngle).toBeGreaterThan(model.primaryBands[0]?.endAngle || -Infinity);
        expect(model.amountAxisMax).toBeGreaterThan(1500);
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
    });

    test('toggleHistoricalSecondarySelection only changes the requested secondary category', () => {
        const selection = toggleHistoricalSecondarySelection(buildSelection(), '交通::地铁');

        expect(selection['交通::地铁']).toBe(false);
        expect(selection['交通::打车']).toBe(true);
        expect(selection['餐饮::早餐']).toBe(true);
    });

    test('buildHistoricalPolarChartModel reflows remaining visible items to fill the circle', () => {
        const selection = buildSelection({
            '餐饮::早餐': false,
            '餐饮::午餐': false
        });
        const model = buildHistoricalPolarChartModel(SAMPLE_POINTS, selection);

        expect(model.slots).toHaveLength(2);
        expect(model.primaryBands).toHaveLength(1);
        expect(model.legendGroups[0]?.state).toBe('none');
        expect(model.legendGroups[1]?.state).toBe('all');
    });

    test('buildHistoricalPolarChartOption keeps equal-width bars, keeps bar labels inside the ring gap, and uses the updated execution scale', () => {
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
            radiusAxis: Array<{ max?: number; interval?: number; axisLabel?: { margin?: number } }>;
            series: Array<Record<string, unknown>>;
        };

        expect(option.radiusAxis[1]?.max).toBe(125);
        expect(option.radiusAxis[1]?.interval).toBe(25);
        expect(option.radiusAxis[0]?.axisLabel?.margin).toBe(10);
        expect(option.series[1]?.['barWidth']).toBe(option.series[2]?.['barWidth']);
        expect(option.series[4]?.['smooth']).toBe(true);
        expect(option.series[0]?.['label']).toMatchObject({ show: false });
        expect(option.series[0]?.['radius']).toStrictEqual(['80%', '86%']);

        const spentSeries = option.series[2] as {
            data: Array<{ value: number }>;
        };
        const labelSeries = option.series[3] as {
            data: Array<{
                value: number;
                label: {
                    formatter: string;
                    rotate: number;
                };
            }>;
        };
        const lineSeries = option.series[4] as {
            data: Array<number>;
        };

        expect(spentSeries.data[0]?.value).toBe(620);
        expect(labelSeries.data[0]?.value).toBeGreaterThan(800);
        expect(labelSeries.data[0]?.value).toBeLessThan(model.amountAxisMax);
        expect(labelSeries.data[0]?.label?.formatter).toBe('早餐');
        expect(Math.abs(labelSeries.data[0]?.label?.rotate || 0)).toBeGreaterThan(1);
        expect(Math.abs(labelSeries.data[0]?.label?.rotate || 0)).toBeLessThanOrEqual(90);
        expect(labelSeries.data[2]?.label?.formatter).toBe('打车');
        expect(Math.abs(labelSeries.data[2]?.label?.rotate || 0)).toBeGreaterThan(1);
        expect(Math.abs(labelSeries.data[2]?.label?.rotate || 0)).toBeLessThanOrEqual(90);
        expect(lineSeries.data[2]).toBeCloseTo(117.1, 1);
    });

    test('buildHistoricalPolarChartOption omits the primary ring when showPrimaryRing is false', () => {
        const model = buildHistoricalPolarChartModel(SAMPLE_POINTS, buildSelection());
        const option = buildHistoricalPolarChartOption(model, {
            isDarkMode: false,
            accentColor: '#5c6bc0',
            budgetAmountLabel: '预算金额',
            spentAmountLabel: '已花费',
            executionRateLabel: '执行度',
            formatAmount: (amount: number) => `¥${amount.toFixed(2)}`,
            showPrimaryRing: false
        }) as {
            series: Array<Record<string, unknown>>;
        };

        expect(option.series[0]?.['name']).not.toBe('主分类环');
    });
});
