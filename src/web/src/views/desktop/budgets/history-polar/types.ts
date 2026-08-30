import type { ApplicationThemeSemanticColors } from '@/core/theme.ts';

export interface HistoricalCategoryChartPoint {
    category: string;
    primaryCategory: string;
    secondaryCategory: string;
    budgetAmountCents: number;
    spentAmountCents: number;
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
    budgetAmountCents: number;
    spentAmountCents: number;
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
}

export interface HistoricalPolarChartOptionArgs {
    isDarkMode: boolean;
    themePalette?: Pick<ApplicationThemeSemanticColors, 'surface' | 'chartText' | 'chartMutedText' | 'chartGrid' | 'tooltipBackground' | 'tooltipText' | 'tooltipBorder'>;
    accentColor: string;
    budgetAmountLabel: string;
    spentAmountLabel: string;
    executionRateLabel: string;
    formatAmount: (amount: number) => string;
    showPrimaryRing: boolean;
    categoryAnimationScope?: string;
    amountAxisRenderScope?: string;
    labelAnimationState?: HistoricalLabelAnimationState;
}

export interface HistoricalLabelAnimationSnapshot {
    radiusValue: number;
    radiusAxisMaxValue: number;
    radiusRatio: number;
    polarAngleValue: number;
    rotate: number;
    facingBucket: number;
    dataId?: string;
    name?: string;
    text?: string;
    color?: string;
    fontSize?: number;
    fontWeight?: number;
    width?: number;
    visible?: boolean;
    ghostEmitted?: boolean;
}

export type HistoricalSectorCollapseMode = 'angle' | 'radius';

export interface HistoricalSectorAnimationSnapshot {
    kind: 'sector';
    startAngleValue: number;
    endAngleValue: number;
    innerRadiusValue: number;
    outerRadiusValue: number;
    radiusAxisMaxValue: number;
    innerRadiusRatio: number;
    outerRadiusRatio: number;
    dataId?: string;
    name?: string;
    color?: string;
    opacity?: number;
    borderColor?: string;
    borderWidth?: number;
    collapseMode?: HistoricalSectorCollapseMode;
    visible?: boolean;
    ghostEmitted?: boolean;
}

export interface HistoricalGridLineAnimationSnapshot {
    kind: 'grid-line';
    axisIndex?: number;
    amountValue: number;
    radiusAxisMaxValue: number;
    radiusRatio: number;
    dataId?: string;
    name?: string;
    color?: string;
    visible?: boolean;
    ghostEmitted?: boolean;
}

export interface HistoricalLabelAnimationFrameInput {
    stateKey: string;
    dataId: string;
    name: string;
    text: string;
    radiusValue: number;
    radiusAxisMaxValue?: number;
    radiusRatio?: number;
    polarAngleValue: number;
    rotate: number;
    color: string;
    fontSize: number;
    fontWeight: number;
    width?: number;
}

export interface HistoricalLabelAnimationFrame extends HistoricalLabelAnimationFrameInput {
    radiusAxisMaxValue: number;
    radiusRatio: number;
    previous: HistoricalLabelAnimationSnapshot | null;
    opacity: number;
    scale: number;
    leaving: boolean;
    crossFadeOnly: boolean;
    facingBucket: number;
}

export type HistoricalAnimationSnapshot =
    | HistoricalLabelAnimationSnapshot
    | HistoricalSectorAnimationSnapshot
    | HistoricalGridLineAnimationSnapshot;

export type HistoricalLabelAnimationState = Map<string, HistoricalAnimationSnapshot>;

export interface HistoricalLabelRenderParams {
    dataIndex: number;
    dataIndexInside?: number;
}

export interface HistoricalLabelRenderApi {
    coord: (value: number[]) => number[];
}

export interface HistoricalLabelKeyframe {
    percent: number;
    x: number;
    y: number;
    rotation: number;
    style: {
        opacity: number;
    };
}

export interface HistoricalSectorAnimationFrameInput {
    stateKey: string;
    dataId: string;
    name: string;
    startAngleValue: number;
    endAngleValue: number;
    innerRadiusValue: number;
    outerRadiusValue: number;
    radiusAxisMaxValue: number;
    innerRadiusRatio?: number;
    outerRadiusRatio?: number;
    color: string;
    opacity: number;
    borderColor?: string;
    borderWidth?: number;
    collapseMode: HistoricalSectorCollapseMode;
}

export interface HistoricalSectorAnimationFrame extends HistoricalSectorAnimationFrameInput {
    previous: HistoricalSectorAnimationSnapshot | null;
    leaving: boolean;
}

export interface HistoricalSectorRenderParams {
    dataIndex: number;
    dataIndexInside?: number;
    coordSys?: {
        cx?: number;
        cy?: number;
    };
}

export type HistoricalSectorRenderApi = HistoricalLabelRenderApi;

export interface HistoricalSectorShape {
    cx: number;
    cy: number;
    r0: number;
    r: number;
    startAngle: number;
    endAngle: number;
    clockwise: boolean;
}

export interface HistoricalSectorKeyframe {
    percent: number;
    shape: HistoricalSectorShape;
    style: {
        opacity: number;
    };
}

export interface HistoricalGridLineAnimationFrameInput {
    stateKey: string;
    dataId: string;
    name: string;
    axisIndex?: number;
    amountValue: number;
    radiusAxisMaxValue: number;
    color: string;
}

export interface HistoricalGridLineAnimationFrame extends HistoricalGridLineAnimationFrameInput {
    axisIndex: number;
    radiusRatio: number;
    opacity: number;
    previous: HistoricalGridLineAnimationSnapshot | null;
    leaving: boolean;
    labelGhost: boolean;
    rangeChanged: boolean;
}

export interface HistoricalGridLineShape {
    cx: number;
    cy: number;
    r: number;
}

export interface HistoricalGridLineKeyframe {
    percent: number;
    shape: HistoricalGridLineShape;
    style: {
        opacity: number;
    };
}

export interface HistoricalAmountAxisLabelKeyframe {
    percent: number;
    x: number;
    y: number;
    style: {
        opacity: number;
    };
}
