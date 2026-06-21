import type {
    HistoricalCategoryChartPoint,
    HistoricalChartSlot,
    HistoricalLegendGroup,
    HistoricalLegendSelection,
    HistoricalPolarChartModel,
    HistoricalPrimaryBand,
    HistoricalPrimaryLegendState
} from './types.ts';
import {
    PRIMARY_RING_PAD_ANGLE,
    START_ANGLE
} from './constants.ts';
import {
    normalizeText,
    toNonNegativeFiniteNumber,
    toSortOrder
} from './math.ts';
import { getHistoricalAmountAxisInterval } from './amountAxis.ts';
import { buildPrimaryLabels } from './geometry.ts';

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

function normalizeHistoricalChartPoint(point: HistoricalCategoryChartPoint): HistoricalCategoryChartPoint {
    return {
        category: normalizeText(point.category),
        primaryCategory: normalizeText(point.primaryCategory),
        secondaryCategory: normalizeText(point.secondaryCategory),
        budgetAmountCents: toNonNegativeFiniteNumber(point.budgetAmountCents),
        spentAmountCents: toNonNegativeFiniteNumber(point.spentAmountCents),
        executionRate: toNonNegativeFiniteNumber(point.executionRate),
        color: normalizeText(point.color) || '#5470c6',
        groupOrder: toSortOrder(point.groupOrder),
        itemOrder: toSortOrder(point.itemOrder)
    };
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
    const sortedPoints = points
        .map(normalizeHistoricalChartPoint)
        .filter(point => point.primaryCategory || point.secondaryCategory || point.category)
        .sort(comparePoints);

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
    const primaryRingSlotAngle = usableAngle / totalVisibleSlotCount;
    const barSlotAngle = 360 / totalVisibleSlotCount;
    const primaryBands: HistoricalPrimaryBand[] = [];
    const slots: HistoricalChartSlot[] = [];

    let cursorAngle = START_ANGLE;
    let visibleSlotIndex = 0;
    for (const group of groups) {
        const spanAngle = group.items.length * primaryRingSlotAngle;
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

        for (const item of group.items) {
            const angle = START_ANGLE - (barSlotAngle * (visibleSlotIndex + 0.5));
            slots.push({
                key: item.secondaryKey,
                label: item.secondaryCategory || item.category,
                primaryKey: group.key,
                secondaryKey: item.secondaryKey,
                budgetAmountCents: item.budgetAmountCents,
                spentAmountCents: item.spentAmountCents,
                executionRate: item.executionRate,
                labelAnchorAmount: Math.max(item.budgetAmountCents, item.spentAmountCents),
                color: item.color,
                angle
            });
            visibleSlotIndex++;
        }

        cursorAngle = endAngle - primaryPadAngle;
    }

    return {
        primaryBands,
        slots,
        primaryPadAngle
    };
}

/**
 * 根据图例选择状态同步主分类和子分类的可见性。
 */
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

/**
 * 切换单个子分类图例选择状态。
 */
export function toggleHistoricalSecondarySelection(
    currentSelection: HistoricalLegendSelection,
    secondaryKey: string
): HistoricalLegendSelection {
    return {
        ...currentSelection,
        [secondaryKey]: currentSelection[secondaryKey] === false
    };
}

/**
 * 切换主分类图例选择状态，并级联影响其子分类。
 */
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

/**
 * 将历史预算分组转换为极坐标图模型，包括图例、环带、槽位和金额轴范围。
 */
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
    const maxAmount = Math.max(0, ...slots.flatMap(slot => [slot.budgetAmountCents, slot.spentAmountCents]));
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
        primaryPadAngle
    };
}
