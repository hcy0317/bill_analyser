import { BUDGET_HISTORY_CHART_CONFIG } from '@/config/budget.ts';

export interface HistoricalCategoryChartPoint {
    category: string;
    primaryCategory: string;
    secondaryCategory: string;
    budgetAmount: number;
    spentAmount: number;
    executionRate: number;
    color: string;
    groupOrder: number;
    itemOrder: number;
}

export type HistoricalLegendSelection = Record<string, boolean>;

export type HistoricalPrimaryLegendState = 'all' | 'partial' | 'none';

export interface HistoricalChartSlot {
    key: string;
    label: string;
    primaryKey: string;
    secondaryKey: string;
    budgetAmount: number;
    spentAmount: number;
    executionRate: number;
    labelAnchorAmount: number;
    color: string;
    angle: number;
}

export interface HistoricalLegendItem {
    key: string;
    label: string;
    color: string;
    selected: boolean;
}

export interface HistoricalLegendGroup {
    primaryKey: string;
    primaryLabel: string;
    color: string;
    state: HistoricalPrimaryLegendState;
    secondaryItems: HistoricalLegendItem[];
}

export interface HistoricalPrimaryBand {
    key: string;
    label: string;
    color: string;
    secondaryKeys: string[];
    startAngle: number;
    endAngle: number;
    spanAngle: number;
    state: HistoricalPrimaryLegendState;
}

export interface HistoricalPrimaryLabel {
    key: string;
    label: string;
    rotate: number;
    fontSize: number;
    color: string;
    angle: number;
    radiusValue: number;
}

export interface HistoricalPolarChartModel {
    slots: HistoricalChartSlot[];
    primaryBands: HistoricalPrimaryBand[];
    legendGroups: HistoricalLegendGroup[];
    amountAxisMax: number;
    amountAxisInterval: number;
    averageExecutionRate: number;
    primaryLabels: HistoricalPrimaryLabel[];
    primaryPadAngle: number;
    executionAxisMax: number;
}

export interface HistoricalPolarChartOptionArgs {
    isDarkMode: boolean;
    accentColor: string;
    budgetAmountLabel: string;
    spentAmountLabel: string;
    executionRateLabel: string;
    formatAmount: (amount: number) => string;
    showPrimaryRing: boolean;
}

const {
    START_ANGLE,
    MAX_EXECUTION_RATE,
    EXECUTION_AXIS_MAX,
    EXECUTION_AXIS_INTERVAL,
    PRIMARY_RING_PAD_ANGLE,
    POLAR_CENTER,
    BAR_POLAR_RADIUS,
    EXECUTION_POLAR_RADIUS,
    PRIMARY_RING_RADIUS,
    PRIMARY_LABEL_POLAR_RADIUS,
    PRIMARY_LABEL_RADIUS_AXIS_MAX,
    PRIMARY_LABEL_RADIUS_VALUE,
    PRIMARY_LABEL_MIN_FONT_SIZE,
    PRIMARY_LABEL_MAX_FONT_SIZE,
    PRIMARY_LABEL_TRUNCATE_WIDTH,
    SECONDARY_LABEL_BUFFER_INTERVAL_RATIO,
    SECONDARY_LABEL_MAX_AXIS_RATIO,
    BOTTOM_LABEL_FLIP_START,
    BOTTOM_LABEL_FLIP_END,
    AMOUNT_AXIS_LABEL_DARK_COLOR,
    AMOUNT_AXIS_LABEL_LIGHT_COLOR,
    EXECUTION_SPLIT_LINE_DARK_COLOR,
    EXECUTION_SPLIT_LINE_LIGHT_COLOR
} = BUDGET_HISTORY_CHART_CONFIG;

interface InternalPrimaryGroupItem extends HistoricalCategoryChartPoint {
    secondaryKey: string;
    selected: boolean;
}

interface InternalPrimaryGroup {
    key: string;
    label: string;
    color: string;
    items: InternalPrimaryGroupItem[];
}

interface VisiblePrimaryGroup {
    key: string;
    label: string;
    color: string;
    items: InternalPrimaryGroupItem[];
}

function normalizeSecondaryKey(point: HistoricalCategoryChartPoint): string {
    return `${point.primaryCategory}::${point.secondaryCategory || point.category}`;
}

function comparePoints(left: HistoricalCategoryChartPoint, right: HistoricalCategoryChartPoint): number {
    if (left.groupOrder !== right.groupOrder) {
        return left.groupOrder - right.groupOrder;
    }
    if (left.itemOrder !== right.itemOrder) {
        return left.itemOrder - right.itemOrder;
    }
    if (left.primaryCategory !== right.primaryCategory) {
        return left.primaryCategory.localeCompare(right.primaryCategory, 'zh-CN');
    }

    const leftLabel = left.secondaryCategory || left.category;
    const rightLabel = right.secondaryCategory || right.category;
    return leftLabel.localeCompare(rightLabel, 'zh-CN');
}

function clamp(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
}

function withAlpha(color: string, alpha: number): string {
    const normalizedColor = color.replace('#', '').trim();
    if (!/^[0-9a-fA-F]{6}$/.test(normalizedColor)) {
        return `rgba(0,0,0,${alpha})`;
    }

    const matched = normalizedColor.match(/.{2}/g)!;
    const [red, green, blue] = matched.map(item => parseInt(item, 16));
    return `rgba(${red},${green},${blue},${alpha})`;
}

function normalizeCircleAngle(angle: number): number {
    return ((angle % 360) + 360) % 360;
}

function shouldFlipTangentialLabel(angle: number): boolean {
    const normalized = normalizeCircleAngle(angle);

    return normalized >= BOTTOM_LABEL_FLIP_START && normalized <= BOTTOM_LABEL_FLIP_END;
}

function normalizeRotation(rotation: number): number {
    return ((rotation + 180) % 360 + 360) % 360 - 180;
}

export function getTangentialTextRotation(angle: number): number {
    const normalizedAngle = normalizeCircleAngle(angle);
    const baseRotation = normalizeRotation(normalizedAngle - 90);
    const flippedRotation = shouldFlipTangentialLabel(normalizedAngle)
        ? normalizeRotation(baseRotation + 180)
        : baseRotation;

    return Number(clamp(flippedRotation, -90, 90).toFixed(2));
}

function buildPrimaryLabels(primaryBands: HistoricalPrimaryBand[]): HistoricalPrimaryLabel[] {
    return primaryBands
        .map(band => {
            const label = band.label.trim();
            if (!label) {
                return null;
            }

            const angle = band.startAngle - (band.spanAngle / 2);
            const fontSize = Number(clamp(
                10.6 + (Math.min(band.spanAngle, 180) * 0.02),
                PRIMARY_LABEL_MIN_FONT_SIZE,
                PRIMARY_LABEL_MAX_FONT_SIZE
            ).toFixed(2));

            return {
                key: band.key,
                label,
                rotate: getTangentialTextRotation(angle),
                fontSize,
                color: band.color,
                angle,
                radiusValue: Number(PRIMARY_LABEL_RADIUS_VALUE)
            };
        })
        .filter((label): label is HistoricalPrimaryLabel => label !== null);
}

function getPolarAngleAxisValue(angle: number): number {
    return Number(normalizeCircleAngle(START_ANGLE - angle).toFixed(2));
}

function getSecondaryLabelValue(slot: HistoricalChartSlot, model: HistoricalPolarChartModel): number {
    const bufferedValue = slot.labelAnchorAmount + (model.amountAxisInterval * SECONDARY_LABEL_BUFFER_INTERVAL_RATIO);
    const cappedValue = model.amountAxisMax * SECONDARY_LABEL_MAX_AXIS_RATIO;

    return Math.min(bufferedValue, cappedValue);
}

export function getHistoricalAmountAxisInterval(maxAmount: number): number {
    if (maxAmount <= 0) {
        return 25;
    }

    const roughInterval = maxAmount / 4;
    const magnitude = Math.pow(10, Math.floor(Math.log10(roughInterval)));
    const normalized = roughInterval / magnitude;

    if (normalized <= 1) {
        return magnitude;
    }
    if (normalized <= 2) {
        return 2 * magnitude;
    }
    if (normalized <= 2.5) {
        return 2.5 * magnitude;
    }
    if (normalized <= 5) {
        return 5 * magnitude;
    }

    return 10 * magnitude;
}

function getPrimaryLegendState(selection: HistoricalLegendSelection, secondaryKeys: string[]): HistoricalPrimaryLegendState {
    const visibleCount = secondaryKeys.filter(key => selection[key] !== false).length;
    if (visibleCount === 0) {
        return 'none';
    }
    if (visibleCount === secondaryKeys.length) {
        return 'all';
    }
    return 'partial';
}

function buildInternalPrimaryGroups(
    points: HistoricalCategoryChartPoint[],
    selection: HistoricalLegendSelection
): InternalPrimaryGroup[] {
    const grouped = new Map<string, InternalPrimaryGroup>();
    const sortedPoints = [...points].sort(comparePoints);

    for (const point of sortedPoints) {
        const primaryKey = point.primaryCategory;
        const secondaryKey = normalizeSecondaryKey(point);
        const selected = selection[secondaryKey] !== false;

        if (!grouped.has(primaryKey)) {
            grouped.set(primaryKey, {
                key: primaryKey,
                label: point.primaryCategory,
                color: point.color,
                items: []
            });
        }

        const group = grouped.get(primaryKey)!;
        group.color = group.color || point.color;
        group.items.push({ ...point, secondaryKey, selected });
    }

    return Array.from(grouped.values());
}

function buildVisiblePrimaryGroups(groups: InternalPrimaryGroup[]): VisiblePrimaryGroup[] {
    return groups
        .map(group => ({
            key: group.key,
            label: group.label,
            color: group.color,
            items: group.items.filter(item => item.selected)
        }))
        .filter(group => group.items.length > 0);
}

function buildPrimaryBandsAndSlots(
    groups: VisiblePrimaryGroup[]
): { primaryBands: HistoricalPrimaryBand[]; slots: HistoricalChartSlot[]; primaryPadAngle: number } {
    if (!groups.length) {
        return { primaryBands: [], slots: [], primaryPadAngle: 0 };
    }

    const primaryPadAngle = groups.length > 1 ? PRIMARY_RING_PAD_ANGLE : 0;
    const totalVisibleSlotCount = groups.reduce((sum, group) => sum + group.items.length, 0);

    const totalPaddingAngle = primaryPadAngle * groups.length;
    const usableAngle = 360 - totalPaddingAngle;
    const slotAngle = usableAngle / totalVisibleSlotCount;
    const primaryBands: HistoricalPrimaryBand[] = [];
    const slots: HistoricalChartSlot[] = [];

    let cursorAngle = START_ANGLE;
    for (const group of groups) {
        const spanAngle = group.items.length * slotAngle;
        const startAngle = cursorAngle;
        const endAngle = startAngle - spanAngle;

        primaryBands.push({
            key: group.key,
            label: group.label,
            color: group.color,
            secondaryKeys: group.items.map(item => item.secondaryKey),
            startAngle,
            endAngle,
            spanAngle,
            state: group.items.length === 0 ? 'none' : 'all'
        });

        for (const [index, item] of group.items.entries()) {
            const angle = startAngle - (slotAngle * (index + 0.5));
            slots.push({
                key: item.secondaryKey,
                label: item.secondaryCategory || item.category,
                primaryKey: group.key,
                secondaryKey: item.secondaryKey,
                budgetAmount: item.budgetAmount,
                spentAmount: item.spentAmount,
                executionRate: item.executionRate,
                labelAnchorAmount: Math.max(item.budgetAmount, item.spentAmount),
                color: item.color,
                angle
            });
        }

        cursorAngle = endAngle - primaryPadAngle;
    }

    return {
        primaryBands,
        slots,
        primaryPadAngle
    };
}

export function syncHistoricalLegendSelection(
    points: HistoricalCategoryChartPoint[],
    currentSelection: HistoricalLegendSelection = {}
): HistoricalLegendSelection {
    const nextSelection: HistoricalLegendSelection = {};

    for (const point of points) {
        const secondaryKey = normalizeSecondaryKey(point);
        nextSelection[secondaryKey] = currentSelection[secondaryKey] !== false;
    }

    return nextSelection;
}

export function toggleHistoricalSecondarySelection(
    currentSelection: HistoricalLegendSelection,
    secondaryKey: string
): HistoricalLegendSelection {
    return {
        ...currentSelection,
        [secondaryKey]: currentSelection[secondaryKey] === false
    };
}

export function toggleHistoricalPrimarySelection(
    currentSelection: HistoricalLegendSelection,
    model: HistoricalPolarChartModel,
    primaryKey: string
): HistoricalLegendSelection {
    const legendGroup = model.legendGroups.find(item => item.primaryKey === primaryKey);
    if (!legendGroup) {
        return currentSelection;
    }

    const shouldEnable = legendGroup.state === 'none';
    const nextSelection = { ...currentSelection };

    for (const secondaryItem of legendGroup.secondaryItems) {
        nextSelection[secondaryItem.key] = shouldEnable;
    }

    return nextSelection;
}

export function buildHistoricalPolarChartModel(
    points: HistoricalCategoryChartPoint[],
    selection: HistoricalLegendSelection = {}
): HistoricalPolarChartModel {
    const groups = buildInternalPrimaryGroups(points, selection);
    const legendGroups: HistoricalLegendGroup[] = groups.map(group => {
        const secondaryItems = group.items.map(item => ({
            key: item.secondaryKey,
            label: item.secondaryCategory || item.category,
            color: item.color,
            selected: item.selected
        }));

        return {
            primaryKey: group.key,
            primaryLabel: group.label,
            color: group.color,
            state: getPrimaryLegendState(selection, secondaryItems.map(item => item.key)),
            secondaryItems
        };
    });

    const visibleGroups = buildVisiblePrimaryGroups(groups);
    const { primaryBands, slots, primaryPadAngle } = buildPrimaryBandsAndSlots(visibleGroups);
    const maxAmount = Math.max(0, ...slots.flatMap(slot => [slot.budgetAmount, slot.spentAmount]));
    const longestLabelLength = Math.max(0, ...slots.map(slot => slot.label.length));
    const amountBufferRatio = 0.1 + Math.min(0.16, longestLabelLength * 0.012);
    const interval = getHistoricalAmountAxisInterval(maxAmount > 0 ? maxAmount * (1 + amountBufferRatio) : 100);
    const amountAxisMax = maxAmount > 0 ? Math.ceil((maxAmount * (1 + amountBufferRatio)) / interval) * interval : 100;
    const averageExecutionRate = slots.length > 0
        ? Number((slots.reduce((sum, slot) => sum + slot.executionRate, 0) / slots.length).toFixed(1))
        : 0;
    const primaryLabels = buildPrimaryLabels(primaryBands);

    return {
        slots,
        primaryBands,
        legendGroups,
        amountAxisMax,
        amountAxisInterval: interval,
        averageExecutionRate,
        primaryLabels,
        primaryPadAngle,
        executionAxisMax: EXECUTION_AXIS_MAX
    };
}

export function buildHistoricalPolarChartOption(
    model: HistoricalPolarChartModel,
    args: HistoricalPolarChartOptionArgs
): Record<string, unknown> {
    const slotLabels = model.slots.map(slot => slot.key);
    const series: Array<Record<string, unknown>> = [];
    const polar: Array<Record<string, unknown>> = [
        { center: POLAR_CENTER, radius: BAR_POLAR_RADIUS },
        { center: POLAR_CENTER, radius: EXECUTION_POLAR_RADIUS }
    ];
    const angleAxis: Array<Record<string, unknown>> = [
        {
            type: 'category',
            data: slotLabels,
            startAngle: START_ANGLE,
            clockwise: true,
            boundaryGap: true,
            polarIndex: 0,
            axisLine: { show: false },
            axisTick: { show: false },
            axisLabel: { show: false }
        },
        {
            type: 'category',
            data: slotLabels,
            startAngle: START_ANGLE,
            clockwise: true,
            boundaryGap: true,
            polarIndex: 1,
            show: false
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
                color: args.isDarkMode ? AMOUNT_AXIS_LABEL_DARK_COLOR : AMOUNT_AXIS_LABEL_LIGHT_COLOR,
                margin: 10,
                fontSize: 10,
                fontWeight: 700,
                align: 'center',
                verticalAlign: 'bottom',
                formatter: (value: number) => args.formatAmount(value)
            },
            splitLine: { show: false }
        },
        {
            type: 'value',
            min: 0,
            max: model.executionAxisMax,
            interval: EXECUTION_AXIS_INTERVAL,
            splitNumber: 5,
            polarIndex: 1,
            axisLine: { show: false },
            axisTick: { show: false },
            axisLabel: { show: false },
            splitLine: {
                lineStyle: {
                    color: args.isDarkMode ? EXECUTION_SPLIT_LINE_DARK_COLOR : EXECUTION_SPLIT_LINE_LIGHT_COLOR,
                    type: 'dashed'
                }
            }
        }
    ];

    if (args.showPrimaryRing) {
        series.push({
            name: 'primary-ring',
            type: 'pie',
            radius: PRIMARY_RING_RADIUS,
            center: POLAR_CENTER,
            startAngle: START_ANGLE,
            clockwise: true,
            silent: true,
            z: 1,
            padAngle: model.primaryPadAngle,
            avoidLabelOverlap: false,
            label: {
                show: false
            },
            labelLine: { show: false },
            tooltip: { show: false },
            universalTransition: { enabled: true },
            data: model.primaryBands.map(band => ({
                name: band.label,
                value: band.secondaryKeys.length,
                itemStyle: {
                    color: band.color,
                    borderColor: args.isDarkMode ? '#121212' : '#ffffff',
                    borderWidth: 1.5
                }
            }))
        });

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
            polarIndex: 2,
            axisLine: { show: false },
            axisTick: { show: false },
            axisLabel: { show: false },
            splitLine: { show: false }
        });

        radiusAxis.push({
            type: 'value',
            min: 0,
            max: PRIMARY_LABEL_RADIUS_AXIS_MAX,
            polarIndex: 2,
            axisLine: { show: false },
            axisTick: { show: false },
            axisLabel: { show: false },
            splitLine: { show: false }
        });

        series.push({
            name: 'primary-labels',
            type: 'scatter',
            coordinateSystem: 'polar',
            polarIndex: 2,
            symbolSize: 1,
            z: 4,
            silent: true,
            animationDurationUpdate: 720,
            animationEasingUpdate: 'cubicInOut',
            universalTransition: { enabled: true },
            itemStyle: {
                color: 'rgba(0,0,0,0)'
            },
            tooltip: { show: false },
            data: model.primaryLabels.map(label => ({
                value: [label.radiusValue, getPolarAngleAxisValue(label.angle)],
                label: {
                    show: true,
                    position: 'inside',
                    distance: 0,
                    color: label.color,
                    fontSize: label.fontSize,
                    fontWeight: 700,
                    width: PRIMARY_LABEL_TRUNCATE_WIDTH,
                    overflow: 'truncate',
                    rotate: label.rotate,
                    align: 'center',
                    verticalAlign: 'middle',
                    formatter: label.label
                }
            }))
        });
    }

    series.push(
        {
            name: args.budgetAmountLabel,
            type: 'bar',
            coordinateSystem: 'polar',
            polarIndex: 0,
            roundCap: true,
            barWidth: 14,
            barGap: '-100%',
            z: 2,
            universalTransition: { enabled: true },
            data: model.slots.map(slot => ({
                value: slot.budgetAmount,
                itemStyle: {
                    color: withAlpha(slot.color, 0.28)
                }
            }))
        },
        {
            name: args.spentAmountLabel,
            type: 'bar',
            coordinateSystem: 'polar',
            polarIndex: 0,
            roundCap: true,
            barWidth: 14,
            barGap: '-100%',
            z: 3,
            universalTransition: { enabled: true },
            data: model.slots.map(slot => ({
                value: slot.spentAmount,
                itemStyle: {
                    color: slot.color
                }
            }))
        },
        {
            name: 'secondary-labels',
            type: 'scatter',
            coordinateSystem: 'polar',
            polarIndex: 0,
            symbolSize: 1,
            z: 4,
            animationDurationUpdate: 720,
            animationEasingUpdate: 'cubicInOut',
            universalTransition: { enabled: true },
            itemStyle: {
                color: 'rgba(0,0,0,0)'
            },
            data: model.slots.map(slot => ({
                value: getSecondaryLabelValue(slot, model),
                label: {
                    show: true,
                    position: 'inside',
                    distance: 0,
                    color: args.isDarkMode ? '#e6e6e6' : '#3f3f46',
                    fontSize: 10,
                    fontWeight: 600,
                    rotate: getTangentialTextRotation(slot.angle),
                    align: 'center',
                    verticalAlign: 'middle',
                    formatter: slot.label
                }
            }))
        },
        {
            name: args.executionRateLabel,
            type: 'line',
            coordinateSystem: 'polar',
            polarIndex: 1,
            smooth: true,
            connectNulls: false,
            symbol: 'circle',
            symbolSize: 6,
            z: 5,
            universalTransition: { enabled: true },
            lineStyle: { width: 2.5, color: args.accentColor },
            itemStyle: { color: args.accentColor },
            data: model.slots.map(slot => clamp(slot.executionRate, 0, MAX_EXECUTION_RATE))
        }
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
                const slot = model.slots[params.dataIndex || 0];
                if (!slot) {
                    return '';
                }

                return [
                    `<b>${slot.primaryKey} / ${slot.label}</b>`,
                    `${args.budgetAmountLabel}: ${args.formatAmount(slot.budgetAmount)}`,
                    `${args.spentAmountLabel}: ${args.formatAmount(slot.spentAmount)}`,
                    `${args.executionRateLabel}: ${slot.executionRate}%`
                ].join('<br/>');
            }
        },
        polar,
        angleAxis,
        radiusAxis,
        graphic: [
            {
                type: 'group',
                left: 'center',
                top: '46%',
                bounding: 'raw',
                children: [
                    {
                        type: 'text',
                        x: 0,
                        y: -9,
                        style: {
                            text: args.executionRateLabel,
                            fill: args.isDarkMode ? '#bdbdbd' : '#666',
                            fontSize: 10,
                            fontWeight: 600,
                            textAlign: 'center',
                            textVerticalAlign: 'middle'
                        }
                    },
                    {
                        type: 'text',
                        x: 0,
                        y: 10,
                        style: {
                            text: `${model.averageExecutionRate.toFixed(1)}%`,
                            fill: args.accentColor,
                            fontSize: 18,
                            fontWeight: 700,
                            textAlign: 'center',
                            textVerticalAlign: 'middle'
                        }
                    }
                ]
            }
        ],
        series
    };
}
