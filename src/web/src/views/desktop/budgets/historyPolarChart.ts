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

export interface HistoricalPrimaryLabelGlyph {
    key: string;
    character: string;
    x: number;
    y: number;
    rotate: number;
    fontSize: number;
    color: string;
    angle: number;
}

export interface HistoricalPolarChartModel {
    slots: HistoricalChartSlot[];
    primaryBands: HistoricalPrimaryBand[];
    legendGroups: HistoricalLegendGroup[];
    amountAxisMax: number;
    amountAxisInterval: number;
    averageExecutionRate: number;
    primaryLabelGlyphs: HistoricalPrimaryLabelGlyph[];
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
}

const START_ANGLE = 90;
const MAX_EXECUTION_RATE = 120;
const EXECUTION_AXIS_MAX = 125;
const EXECUTION_AXIS_INTERVAL = 25;
const PRIMARY_RING_PAD_ANGLE = 2.2;
const POLAR_CENTER = ['50%', '46%'] as const;
const BAR_POLAR_RADIUS = ['20%', '74%'] as const;
const EXECUTION_POLAR_RADIUS = ['20%', '74%'] as const;
const PRIMARY_RING_RADIUS = ['76%', '82%'] as const;
const SVG_CENTER_X = 50;
const SVG_CENTER_Y = 46;
const PRIMARY_LABEL_RADIUS = 43.3;

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
    const matched = color.replace('#', '').match(/.{2}/g);
    if (!matched) {
        return `rgba(0,0,0,${alpha})`;
    }

    const [red, green, blue] = matched.map(item => parseInt(item, 16));
    return `rgba(${red},${green},${blue},${alpha})`;
}

function normalizeAngle(angle: number): number {
    if (angle > 180) {
        return angle - 360;
    }
    if (angle < -180) {
        return angle + 360;
    }
    return angle;
}

function polarToSvgPoint(radius: number, angle: number): { x: number; y: number } {
    const radians = (angle * Math.PI) / 180;
    return {
        x: SVG_CENTER_X + radius * Math.cos(radians),
        y: SVG_CENTER_Y - radius * Math.sin(radians)
    };
}

function getHistoricalAmountAxisInterval(maxAmount: number): number {
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
    if (!totalVisibleSlotCount) {
        return { primaryBands: [], slots: [], primaryPadAngle };
    }

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

function buildPrimaryLabelGlyphs(primaryBands: HistoricalPrimaryBand[]): HistoricalPrimaryLabelGlyph[] {
    const glyphs: HistoricalPrimaryLabelGlyph[] = [];

    for (const band of primaryBands) {
        const characters = Array.from(band.label);
        if (!characters.length) {
            continue;
        }

        const paddingAngle = Math.min(1.1, band.spanAngle * 0.08);
        const effectiveStartAngle = band.startAngle - paddingAngle;
        const effectiveEndAngle = band.endAngle + paddingAngle;
        const usableSpanAngle = effectiveStartAngle - effectiveEndAngle;
        const textArcLength = (Math.abs(usableSpanAngle) * Math.PI / 180) * PRIMARY_LABEL_RADIUS;
        const fontSize = clamp(textArcLength / Math.max(characters.length * 1.36, 1), 1.75, 2.5);

        for (const [index, character] of characters.entries()) {
            const progress = characters.length === 1 ? 0.5 : index / (characters.length - 1);
            const angle = effectiveStartAngle - (usableSpanAngle * progress);
            const point = polarToSvgPoint(PRIMARY_LABEL_RADIUS, angle);

            glyphs.push({
                key: `${band.key}-${index}`,
                character,
                x: point.x,
                y: point.y,
                rotate: normalizeAngle(90 - angle),
                fontSize,
                color: band.color,
                angle
            });
        }
    }

    return glyphs;
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

    return {
        slots,
        primaryBands,
        legendGroups,
        amountAxisMax,
        amountAxisInterval: interval,
        averageExecutionRate,
        primaryLabelGlyphs: buildPrimaryLabelGlyphs(primaryBands),
        primaryPadAngle,
        executionAxisMax: EXECUTION_AXIS_MAX
    };
}

export function buildHistoricalPolarChartOption(
    model: HistoricalPolarChartModel,
    args: HistoricalPolarChartOptionArgs
): Record<string, unknown> {
    const slotLabels = model.slots.map(slot => slot.key);

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
        polar: [
            { center: POLAR_CENTER, radius: BAR_POLAR_RADIUS },
            { center: POLAR_CENTER, radius: EXECUTION_POLAR_RADIUS }
        ],
        angleAxis: [
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
        ],
        radiusAxis: [
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
                    color: args.isDarkMode ? '#888' : '#666',
                    margin: 8,
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
                        color: args.isDarkMode ? 'rgba(220, 220, 220, 0.18)' : 'rgba(79, 79, 79, 0.18)',
                        type: 'dashed'
                    }
                }
            }
        ],
        graphic: [
            {
                type: 'text',
                left: 'center',
                top: '39%',
                style: {
                    text: `${model.averageExecutionRate.toFixed(1)}%`,
                    fill: args.accentColor,
                    fontSize: 22,
                    fontWeight: 700,
                    textAlign: 'center'
                }
            },
            {
                type: 'text',
                left: 'center',
                top: '45%',
                style: {
                    text: args.executionRateLabel,
                    fill: args.isDarkMode ? '#bdbdbd' : '#666',
                    fontSize: 11,
                    textAlign: 'center'
                }
            }
        ],
        series: [
            {
                name: 'primary-ring',
                type: 'pie',
                radius: PRIMARY_RING_RADIUS,
                center: POLAR_CENTER,
                startAngle: START_ANGLE,
                clockwise: true,
                silent: true,
                z: 1,
                padAngle: model.primaryPadAngle,
                label: { show: false },
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
            },
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
                animationDurationUpdate: 650,
                itemStyle: {
                    color: 'rgba(0,0,0,0)'
                },
                label: {
                    show: true,
                    position: 'top',
                    distance: 2,
                    color: args.isDarkMode ? '#e6e6e6' : '#3f3f46',
                    fontSize: 11,
                    formatter: (params: { dataIndex?: number }) => model.slots[params.dataIndex || 0]?.label || ''
                },
                data: model.slots.map(slot => slot.labelAnchorAmount)
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
        ]
    };
}
