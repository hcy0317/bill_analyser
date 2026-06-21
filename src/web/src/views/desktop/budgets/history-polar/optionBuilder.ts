import type {
    HistoricalPolarChartModel,
    HistoricalPolarChartOptionArgs
} from './types.ts';
import {
    AMOUNT_AXIS_LABEL_DARK_COLOR,
    AMOUNT_AXIS_LABEL_LIGHT_COLOR,
    AMOUNT_AXIS_SPLIT_LINE_DARK_COLOR,
    AMOUNT_AXIS_SPLIT_LINE_LIGHT_COLOR,
    BAR_POLAR_RADIUS,
    POLAR_CENTER,
    PRIMARY_LABEL_POLAR_RADIUS,
    PRIMARY_LABEL_RADIUS_AXIS_MAX,
    PRIMARY_LABEL_TRUNCATE_WIDTH,
    PRIMARY_RING_RADIUS,
    START_ANGLE
} from './constants.ts';
import {
    formatHistoricalGridAmountKey,
    withAlpha
} from './math.ts';
import {
    buildHistoricalAmountAxisRenderId,
    buildHistoricalAmountAxisRenderScope,
    buildHistoricalScopedKey,
    buildHistoricalScopedPrefix,
    buildHistoricalScopedSeriesId,
    normalizeHistoricalAnimationScope
} from './state.ts';
import {
    getBarSectorHalfAngle,
    getPolarAngleAxisValue,
    getRenderablePrimaryRingSpanAngle,
    getSecondaryLabelValue,
    getTangentialTextRotation
} from './geometry.ts';
import {
    getHistoricalAmountGridLineValues,
    resolveHistoricalGridLineAnimationFrames
} from './amountAxis.ts';
import { buildHistoricalGridLineCustomSeries } from './amountAxisGrid.ts';
import { buildHistoricalAmountAxisLabelCustomSeries } from './amountAxisLabels.ts';
import {
    buildHistoricalLabelCustomSeries,
    resolveHistoricalLabelAnimationFrames
} from './labelAnimation.ts';
import {
    buildHistoricalSectorCustomSeries,
    resolveHistoricalSectorAnimationFrames
} from './sectorAnimation.ts';

/**
 * 构建历史预算极坐标图的 ECharts option，组合扇区、标签和金额轴系列。
 */
export function buildHistoricalPolarChartOption(
    model: HistoricalPolarChartModel,
    args: HistoricalPolarChartOptionArgs
): Record<string, unknown> {
    const formatAmountCents = (amountCents: number): string => args.formatAmount(amountCents / 100);

    if (!model.slots.length || !model.primaryBands.length) {
        return {
            animation: false,
            tooltip: { show: false },
            polar: [],
            angleAxis: [],
            radiusAxis: [],
            series: []
        };
    }

    const series: Array<Record<string, unknown>> = [];
    const polar: Array<Record<string, unknown>> = [
        { center: POLAR_CENTER, radius: BAR_POLAR_RADIUS }
    ];
    const angleAxis: Array<Record<string, unknown>> = [
        {
            type: 'value',
            min: 0,
            max: 360,
            startAngle: START_ANGLE,
            clockwise: true,
            polarIndex: 0,
            axisLine: { show: false },
            axisTick: { show: false },
            axisLabel: { show: false },
            splitLine: { show: false }
        }
    ];
    const radiusAxis: Array<Record<string, unknown>> = [
        {
            type: 'value',
            min: 0,
            max: model.amountAxisMax,
            interval: model.amountAxisInterval,
            splitNumber: 4,
            polarIndex: 0,
            axisLine: { show: false },
            axisTick: { show: false },
            axisLabel: {
                show: false,
                color: args.isDarkMode ? AMOUNT_AXIS_LABEL_DARK_COLOR : AMOUNT_AXIS_LABEL_LIGHT_COLOR,
                margin: 10,
                fontSize: 10,
                fontWeight: 700,
                align: 'center',
                verticalAlign: 'bottom',
                formatter: (value: number) => formatAmountCents(value)
            },
            splitLine: {
                show: false
            }
        }
    ];

    const amountGridLineColor = args.isDarkMode
        ? AMOUNT_AXIS_SPLIT_LINE_DARK_COLOR
        : AMOUNT_AXIS_SPLIT_LINE_LIGHT_COLOR;

    const amountAxisLabelColor = args.isDarkMode
        ? AMOUNT_AXIS_LABEL_DARK_COLOR
        : AMOUNT_AXIS_LABEL_LIGHT_COLOR;
    const amountGridLineValues = getHistoricalAmountGridLineValues(model.amountAxisMax);
    const categoryAnimationScope = normalizeHistoricalAnimationScope(args.categoryAnimationScope);
    const amountAxisRenderScope = buildHistoricalAmountAxisRenderScope(
        normalizeHistoricalAnimationScope(args.amountAxisRenderScope),
        model.amountAxisMax
    );

    series.push(buildHistoricalGridLineCustomSeries(resolveHistoricalGridLineAnimationFrames(
        amountGridLineValues.map((amountValue, index) => {
            const amountKey = formatHistoricalGridAmountKey(amountValue);
            const lineKey = index.toString();
            const lineBaseId = `grid:amount:${lineKey}`;
            return {
                stateKey: lineBaseId,
                dataId: buildHistoricalAmountAxisRenderId(lineBaseId, amountAxisRenderScope),
                name: amountKey,
                amountValue,
                radiusAxisMaxValue: model.amountAxisMax,
                color: amountGridLineColor
            };
        }),
        args.labelAnimationState,
        'grid:amount:',
        'line'
    )));

    series.push(buildHistoricalAmountAxisLabelCustomSeries(
        resolveHistoricalGridLineAnimationFrames(
            amountGridLineValues.map((amountValue, index) => {
                const labelKey = index.toString();
                const labelBaseId = `grid-label:amount:${labelKey}`;
                return {
                    stateKey: labelBaseId,
                    dataId: buildHistoricalAmountAxisRenderId(labelBaseId, amountAxisRenderScope),
                    name: formatAmountCents(amountValue),
                    amountValue,
                    radiusAxisMaxValue: model.amountAxisMax,
                    color: amountAxisLabelColor
                };
            }),
            args.labelAnimationState,
            'grid-label:amount:',
            'label'
        ),
        formatAmountCents,
        amountAxisLabelColor
    ));

    const primaryRingOpacity = args.showPrimaryRing ? 1 : 0;
    const primaryRingPolarIndex = polar.length;

    polar.push({
        center: POLAR_CENTER,
        radius: PRIMARY_RING_RADIUS
    });

    angleAxis.push({
        type: 'value',
        min: 0,
        max: 360,
        startAngle: START_ANGLE,
        clockwise: true,
        polarIndex: primaryRingPolarIndex,
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: { show: false },
        splitLine: { show: false }
    });

    radiusAxis.push({
        type: 'value',
        min: 0,
        max: 1,
        polarIndex: primaryRingPolarIndex,
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: { show: false },
        splitLine: { show: false }
    });

    series.push(buildHistoricalSectorCustomSeries(
        buildHistoricalScopedSeriesId('primary-ring', categoryAnimationScope),
        'primary-ring',
        primaryRingPolarIndex,
        1,
        resolveHistoricalSectorAnimationFrames(
            args.showPrimaryRing
                ? model.primaryBands.map(band => {
                    const startAngleValue = getPolarAngleAxisValue(band.startAngle);
                    const ringKey = buildHistoricalScopedKey('ring:', band.key, categoryAnimationScope);
                    return {
                        stateKey: ringKey,
                        dataId: ringKey,
                        name: band.label,
                        startAngleValue,
                        endAngleValue: startAngleValue + getRenderablePrimaryRingSpanAngle(band.spanAngle),
                        innerRadiusValue: 0,
                        outerRadiusValue: 1,
                        radiusAxisMaxValue: 1,
                        innerRadiusRatio: 0,
                        outerRadiusRatio: 1,
                        color: band.color,
                        opacity: primaryRingOpacity,
                        borderColor: args.isDarkMode ? '#121212' : '#ffffff',
                        borderWidth: args.showPrimaryRing ? 1.5 : 0,
                        collapseMode: 'angle'
                    };
                })
                : [],
            args.labelAnimationState,
            buildHistoricalScopedPrefix('ring:', categoryAnimationScope)
        )
    ));

    const primaryLabelPolarIndex = polar.length;

    polar.push({
        center: POLAR_CENTER,
        radius: PRIMARY_LABEL_POLAR_RADIUS
    });

    angleAxis.push({
        type: 'value',
        min: 0,
        max: 360,
        startAngle: START_ANGLE,
        clockwise: true,
        polarIndex: primaryLabelPolarIndex,
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: { show: false },
        splitLine: { show: false }
    });

    radiusAxis.push({
        type: 'value',
        min: 0,
        max: PRIMARY_LABEL_RADIUS_AXIS_MAX,
        polarIndex: primaryLabelPolarIndex,
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: { show: false },
        splitLine: { show: false }
    });

    const primaryLabelFrames = resolveHistoricalLabelAnimationFrames(
        args.showPrimaryRing
            ? model.primaryLabels.map(label => ({
                stateKey: buildHistoricalScopedKey('primary:', label.key, categoryAnimationScope),
                dataId: `${buildHistoricalScopedKey('primary:', label.key, categoryAnimationScope)}:label`,
                name: label.label,
                text: label.label,
                radiusValue: label.radiusValue,
                radiusAxisMaxValue: PRIMARY_LABEL_RADIUS_AXIS_MAX,
                radiusRatio: label.radiusValue / PRIMARY_LABEL_RADIUS_AXIS_MAX,
                polarAngleValue: getPolarAngleAxisValue(label.angle),
                rotate: label.rotate,
                color: label.color,
                fontSize: label.fontSize,
                fontWeight: 700,
                width: PRIMARY_LABEL_TRUNCATE_WIDTH
            }))
            : [],
        args.labelAnimationState,
        buildHistoricalScopedPrefix('primary:', categoryAnimationScope)
    );

    series.push(buildHistoricalLabelCustomSeries(
        'primary-labels',
        primaryLabelPolarIndex,
        4,
        primaryLabelFrames,
        buildHistoricalScopedSeriesId('primary-labels', categoryAnimationScope)
    ));

    const secondaryLabelPolarIndex = polar.length;

    polar.push({
        center: POLAR_CENTER,
        radius: BAR_POLAR_RADIUS
    });

    angleAxis.push({
        type: 'value',
        min: 0,
        max: 360,
        startAngle: START_ANGLE,
        clockwise: true,
        polarIndex: secondaryLabelPolarIndex,
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: { show: false },
        splitLine: { show: false }
    });

    radiusAxis.push({
        type: 'value',
        min: 0,
        max: model.amountAxisMax,
        interval: model.amountAxisInterval,
        splitNumber: 4,
        polarIndex: secondaryLabelPolarIndex,
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: { show: false },
        splitLine: { show: false }
    });

    const barHalfAngle = getBarSectorHalfAngle(model.slots.length);

    series.push(
        buildHistoricalSectorCustomSeries(
            buildHistoricalScopedSeriesId('budget-bars', categoryAnimationScope),
            args.budgetAmountLabel,
            0,
            2,
            resolveHistoricalSectorAnimationFrames(
                model.slots.map(slot => {
                    const centerAngleValue = getPolarAngleAxisValue(slot.angle);
                    const budgetBarKey = buildHistoricalScopedKey('bar:budget:', slot.key, categoryAnimationScope);
                    return {
                        stateKey: budgetBarKey,
                        dataId: categoryAnimationScope ? budgetBarKey : `${slot.key}:budget`,
                        name: slot.key,
                        startAngleValue: centerAngleValue - barHalfAngle,
                        endAngleValue: centerAngleValue + barHalfAngle,
                        innerRadiusValue: 0,
                        outerRadiusValue: slot.budgetAmountCents,
                        radiusAxisMaxValue: model.amountAxisMax,
                        innerRadiusRatio: 0,
                        outerRadiusRatio: model.amountAxisMax > 0 ? slot.budgetAmountCents / model.amountAxisMax : 0,
                        color: withAlpha(slot.color, 0.28),
                        opacity: 1,
                        collapseMode: 'radius'
                    };
                }),
                args.labelAnimationState,
                buildHistoricalScopedPrefix('bar:budget:', categoryAnimationScope)
            )
        ),
        buildHistoricalSectorCustomSeries(
            buildHistoricalScopedSeriesId('spent-bars', categoryAnimationScope),
            args.spentAmountLabel,
            0,
            3,
            resolveHistoricalSectorAnimationFrames(
                model.slots.map(slot => {
                    const centerAngleValue = getPolarAngleAxisValue(slot.angle);
                    const spentBarKey = buildHistoricalScopedKey('bar:spent:', slot.key, categoryAnimationScope);
                    return {
                        stateKey: spentBarKey,
                        dataId: categoryAnimationScope ? spentBarKey : `${slot.key}:spent`,
                        name: slot.key,
                        startAngleValue: centerAngleValue - barHalfAngle,
                        endAngleValue: centerAngleValue + barHalfAngle,
                        innerRadiusValue: 0,
                        outerRadiusValue: slot.spentAmountCents,
                        radiusAxisMaxValue: model.amountAxisMax,
                        innerRadiusRatio: 0,
                        outerRadiusRatio: model.amountAxisMax > 0 ? slot.spentAmountCents / model.amountAxisMax : 0,
                        color: slot.color,
                        opacity: 1,
                        collapseMode: 'radius'
                    };
                }),
                args.labelAnimationState,
                buildHistoricalScopedPrefix('bar:spent:', categoryAnimationScope)
            )
        ),
        buildHistoricalLabelCustomSeries(
            'secondary-labels',
            secondaryLabelPolarIndex,
            4,
            resolveHistoricalLabelAnimationFrames(
                model.slots.map(slot => ({
                    stateKey: buildHistoricalScopedKey('secondary:', slot.key, categoryAnimationScope),
                    dataId: categoryAnimationScope
                        ? `${buildHistoricalScopedKey('secondary:', slot.key, categoryAnimationScope)}:label`
                        : `${slot.key}:label`,
                    name: slot.key,
                    text: slot.label,
                    radiusValue: getSecondaryLabelValue(slot, model),
                    radiusAxisMaxValue: model.amountAxisMax,
                    radiusRatio: model.amountAxisMax > 0 ? getSecondaryLabelValue(slot, model) / model.amountAxisMax : 0,
                    polarAngleValue: getPolarAngleAxisValue(slot.angle),
                    rotate: getTangentialTextRotation(slot.angle),
                    color: args.isDarkMode ? '#e6e6e6' : '#3f3f46',
                    fontSize: 10,
                    fontWeight: 600
                })),
                args.labelAnimationState,
                buildHistoricalScopedPrefix('secondary:', categoryAnimationScope)
            ),
            buildHistoricalScopedSeriesId('secondary-labels', categoryAnimationScope)
        )
    );

    return {
        animation: true,
        animationDuration: 500,
        animationDurationUpdate: 650,
        animationEasing: 'cubicOut',
        animationEasingUpdate: 'cubicInOut',
        tooltip: {
            trigger: 'item',
            backgroundColor: args.isDarkMode ? '#333' : '#fff',
            borderColor: args.isDarkMode ? '#333' : '#fff',
            textStyle: { color: args.isDarkMode ? '#eee' : '#333' },
            formatter: (params: { dataIndex?: number }) => {
                const dataIndex = Number.isInteger(params.dataIndex) ? params.dataIndex! : -1;
                const slot = model.slots[dataIndex];
                if (!slot) {
                    return '';
                }

                return [
                    `<b>${slot.primaryKey} / ${slot.label}</b>`,
                    `${args.budgetAmountLabel}: ${formatAmountCents(slot.budgetAmountCents)}`,
                    `${args.spentAmountLabel}: ${formatAmountCents(slot.spentAmountCents)}`,
                    `${args.executionRateLabel}: ${slot.executionRate}%`
                ].join('<br/>');
            }
        },
        polar,
        angleAxis,
        radiusAxis,
        graphic: [
            {
                id: 'budget-history-center-group',
                type: 'group',
                left: 'center',
                top: '46%',
                bounding: 'raw',
                scaleX: 1,
                scaleY: 1,
                transition: ['scaleX', 'scaleY'],
                enterFrom: {
                    scaleX: 0.86,
                    scaleY: 0.86
                },
                enterAnimation: {
                    duration: 360,
                    easing: 'cubicOut'
                },
                updateAnimation: {
                    duration: 650,
                    easing: 'cubicInOut'
                },
                children: [
                    {
                        id: 'budget-history-center-label',
                        name: 'budget-history-center-label',
                        type: 'text',
                        x: 0,
                        y: -17,
                        transition: ['y'],
                        enterFrom: {
                            y: -11,
                            style: { opacity: 0 }
                        },
                        style: {
                            text: args.executionRateLabel,
                            fill: args.isDarkMode ? '#bdbdbd' : '#666',
                            fontSize: 10,
                            fontWeight: 600,
                            textAlign: 'center',
                            textVerticalAlign: 'middle',
                            opacity: 1
                        }
                    },
                    {
                        id: 'budget-history-center-value',
                        name: 'budget-history-center-value',
                        type: 'text',
                        x: 0,
                        y: 0,
                        transition: ['y'],
                        enterFrom: {
                            y: 8,
                            style: { opacity: 0 }
                        },
                        style: {
                            text: `${model.averageExecutionRate.toFixed(1)}%`,
                            fill: args.accentColor,
                            fontSize: 18,
                            fontWeight: 700,
                            textAlign: 'center',
                            textVerticalAlign: 'middle',
                            opacity: 1
                        }
                    }
                ]
            }
        ],
        series
    };
}
