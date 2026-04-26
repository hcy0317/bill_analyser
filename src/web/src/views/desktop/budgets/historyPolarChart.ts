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
}

export interface HistoricalPolarChartOptionArgs {
    isDarkMode: boolean;
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

interface HistoricalSectorAnimationSnapshot {
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

interface HistoricalGridLineAnimationSnapshot {
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

type HistoricalAnimationSnapshot =
    | HistoricalLabelAnimationSnapshot
    | HistoricalSectorAnimationSnapshot
    | HistoricalGridLineAnimationSnapshot;

export type HistoricalLabelAnimationState = Map<string, HistoricalAnimationSnapshot>;

const {
    START_ANGLE,
    PRIMARY_RING_PAD_ANGLE,
    POLAR_CENTER,
    BAR_POLAR_RADIUS,
    PRIMARY_RING_RADIUS,
    PRIMARY_LABEL_POLAR_RADIUS,
    PRIMARY_LABEL_RADIUS_AXIS_MAX,
    PRIMARY_LABEL_RADIUS_VALUE,
    PRIMARY_LABEL_MIN_FONT_SIZE,
    PRIMARY_LABEL_MAX_FONT_SIZE,
    PRIMARY_LABEL_TRUNCATE_WIDTH,
    SECONDARY_LABEL_BUFFER_INTERVAL_RATIO,
    SECONDARY_LABEL_MAX_AXIS_RATIO,
    AMOUNT_AXIS_LABEL_DARK_COLOR,
    AMOUNT_AXIS_LABEL_LIGHT_COLOR
} = BUDGET_HISTORY_CHART_CONFIG;

const HISTORICAL_LABEL_SERIES_Z = 8;
const HISTORICAL_LABEL_Z2 = 40;
const HISTORY_ANIMATION_DURATION = 720;
const HISTORY_ANIMATION_EASING = 'cubicInOut';
const HISTORY_LABEL_FADE_OUT_PERCENT = 0.16;
const HISTORY_LABEL_FADE_IN_START_PERCENT = 0.68;
const HISTORY_AMOUNT_AXIS_FADE_IN_START_PERCENT = 0.58;
const AMOUNT_AXIS_GRID_LINE_COUNT = 4;
const AMOUNT_AXIS_OUTER_TRANSITION_RADIUS_RATIO = 1.12;
const AMOUNT_AXIS_MAX_RENDER_RADIUS_RATIO = 1.14;
const SECONDARY_LABEL_MIN_PADDING_RATIO = 0.015;
const BAR_SECTOR_WIDTH_RATIO = 0.22;
const BAR_SECTOR_MIN_HALF_ANGLE = 2.4;
const BAR_SECTOR_MAX_HALF_ANGLE = 7.5;
const LABEL_ROTATION_CROSS_FADE_THRESHOLD = 90;
const AMOUNT_AXIS_SPLIT_LINE_DARK_COLOR = 'rgba(255, 255, 255, 0.14)';
const AMOUNT_AXIS_SPLIT_LINE_LIGHT_COLOR = 'rgba(15, 23, 42, 0.12)';
const AMOUNT_AXIS_LABEL_OFFSET_Y = 3;
const AMOUNT_AXIS_STATE_PREFIXES = ['grid:amount:', 'grid-label:amount:'];
const PRIMARY_RING_FULL_CIRCLE_EPSILON = 0.8;

function normalizeHistoricalAnimationScope(scope?: string): string {
    return normalizeText(scope ?? '').replace(/[^a-zA-Z0-9_-]+/g, '_');
}

function buildHistoricalScopedKey(prefix: string, key: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${prefix}${normalizedScope}:${key}` : `${prefix}${key}`;
}

function buildHistoricalScopedPrefix(prefix: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${prefix}${normalizedScope}:` : prefix;
}

function buildHistoricalAmountAxisRenderId(baseId: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${baseId}:${normalizedScope}` : baseId;
}

function buildHistoricalAmountAxisRenderScope(scope: string, amountAxisMax: number): string {
    const axisScope = `axis-${formatHistoricalGridAmountKey(amountAxisMax)}`;

    return normalizeHistoricalAnimationScope(scope ? `${scope}-${axisScope}` : axisScope);
}

function buildHistoricalScopedSeriesId(baseId: string, scope?: string): string {
    const normalizedScope = normalizeHistoricalAnimationScope(scope);

    return normalizedScope ? `${baseId}:${normalizedScope}` : baseId;
}

function getRenderablePrimaryRingSpanAngle(spanAngle: number): number {
    const normalizedSpan = clamp(toFiniteNumber(spanAngle), 0, 360);

    return normalizedSpan >= 360
        ? 360 - PRIMARY_RING_FULL_CIRCLE_EPSILON
        : normalizedSpan;
}

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

function toFiniteNumber(value: number, fallback = 0): number {
    const numericValue = Number(value);
    return Number.isFinite(numericValue) ? numericValue : fallback;
}

function toNonNegativeFiniteNumber(value: number): number {
    return Math.max(0, toFiniteNumber(value));
}

function toSortOrder(value: number): number {
    return toFiniteNumber(value, Number.MAX_SAFE_INTEGER);
}

function normalizeText(value: string): string {
    return String(value ?? '').trim();
}

function normalizeHistoricalChartPoint(point: HistoricalCategoryChartPoint): HistoricalCategoryChartPoint {
    return {
        category: normalizeText(point.category),
        primaryCategory: normalizeText(point.primaryCategory),
        secondaryCategory: normalizeText(point.secondaryCategory),
        budgetAmount: toNonNegativeFiniteNumber(point.budgetAmount),
        spentAmount: toNonNegativeFiniteNumber(point.spentAmount),
        executionRate: toNonNegativeFiniteNumber(point.executionRate),
        color: normalizeText(point.color) || '#5470c6',
        groupOrder: toSortOrder(point.groupOrder),
        itemOrder: toSortOrder(point.itemOrder)
    };
}

function withAlpha(color: string, alpha: number): string {
    const normalizedColor = normalizeText(color).replace('#', '');
    if (!/^[0-9a-fA-F]{6}$/.test(normalizedColor)) {
        return `rgba(0,0,0,${alpha})`;
    }

    const matched = normalizedColor.match(/.{2}/g)!;
    const [red, green, blue] = matched.map(item => parseInt(item, 16));
    return `rgba(${red},${green},${blue},${alpha})`;
}

function normalizeCircleAngle(angle: number): number {
    if (!Number.isFinite(angle)) {
        return 0;
    }

    return ((angle % 360) + 360) % 360;
}

function normalizeRotation(rotation: number): number {
    return ((rotation + 180) % 360 + 360) % 360 - 180;
}

export function createHistoricalLabelAnimationState(): HistoricalLabelAnimationState {
    return new Map();
}

export function resetHistoricalLabelAnimationState(state: HistoricalLabelAnimationState): void {
    state.clear();
}

export function resetHistoricalCategoryAnimationState(state: HistoricalLabelAnimationState): void {
    for (const key of Array.from(state.keys())) {
        if (!AMOUNT_AXIS_STATE_PREFIXES.some(prefix => key.startsWith(prefix))) {
            state.delete(key);
        }
    }
}

function isHistoricalLabelSnapshot(
    snapshot: HistoricalAnimationSnapshot | undefined
): snapshot is HistoricalLabelAnimationSnapshot {
    const labelSnapshot = snapshot as HistoricalLabelAnimationSnapshot | undefined;
    return !!snapshot
        && (snapshot as HistoricalSectorAnimationSnapshot).kind !== 'sector'
        && (snapshot as HistoricalGridLineAnimationSnapshot).kind !== 'grid-line'
        && typeof labelSnapshot?.radiusValue === 'number'
        && typeof labelSnapshot?.polarAngleValue === 'number'
        && typeof labelSnapshot?.rotate === 'number';
}

function isHistoricalSectorSnapshot(
    snapshot: HistoricalAnimationSnapshot | undefined
): snapshot is HistoricalSectorAnimationSnapshot {
    return !!snapshot
        && (snapshot as HistoricalSectorAnimationSnapshot).kind === 'sector'
        && typeof (snapshot as HistoricalSectorAnimationSnapshot).startAngleValue === 'number'
        && typeof (snapshot as HistoricalSectorAnimationSnapshot).endAngleValue === 'number';
}

function isHistoricalGridLineSnapshot(
    snapshot: HistoricalAnimationSnapshot | undefined
): snapshot is HistoricalGridLineAnimationSnapshot {
    return !!snapshot
        && (snapshot as HistoricalGridLineAnimationSnapshot).kind === 'grid-line'
        && typeof (snapshot as HistoricalGridLineAnimationSnapshot).radiusAxisMaxValue === 'number'
        && typeof (snapshot as HistoricalGridLineAnimationSnapshot).radiusRatio === 'number';
}

export function resolveNearestCircularAngle(targetAngle: number, previousAngle: number): number {
    if (!Number.isFinite(targetAngle) || !Number.isFinite(previousAngle)) {
        return Number.isFinite(targetAngle) ? targetAngle : 0;
    }

    return Number((previousAngle + normalizeRotation(targetAngle - previousAngle)).toFixed(4));
}

export function interpolateHistoricalPolarAngle(startAngle: number, endAngle: number, progress: number): number {
    const clampedProgress = clamp(progress, 0, 1);
    const unwrappedEndAngle = resolveNearestCircularAngle(endAngle, startAngle);

    return Number((startAngle + ((unwrappedEndAngle - startAngle) * clampedProgress)).toFixed(4));
}

export function resolveHistoricalLabelAnimationFrames(
    inputs: HistoricalLabelAnimationFrameInput[],
    state?: HistoricalLabelAnimationState,
    stateKeyPrefix?: string
): HistoricalLabelAnimationFrame[] {
    const visibleKeys = new Set(inputs.map(input => input.stateKey));
    const currentRadiusAxisMaxValue = inputs.reduce(
        (maxValue, input) => Math.max(
            maxValue,
            toNonNegativeFiniteNumber(input.radiusAxisMaxValue ?? input.radiusValue)
        ),
        0
    );
    const frames = inputs.map(input => {
        const previousSnapshot = state?.get(input.stateKey);
        const previous = isHistoricalLabelSnapshot(previousSnapshot) ? previousSnapshot : null;
        const radiusValue = toNonNegativeFiniteNumber(input.radiusValue);
        const radiusAxisMaxValue = Math.max(
            1,
            toNonNegativeFiniteNumber(input.radiusAxisMaxValue ?? radiusValue)
        );
        const radiusRatio = clamp(
            toFiniteNumber(input.radiusRatio ?? (radiusValue / radiusAxisMaxValue)),
            0,
            1
        );
        const inputPolarAngle = normalizeCircleAngle(input.polarAngleValue);
        const inputRotate = toFiniteNumber(input.rotate);
        const facingBucket = getHistoricalLabelFacingBucket(inputPolarAngle);
        const previousFacingBucket = previous?.facingBucket
            ?? (previous ? getHistoricalLabelFacingBucket(previous.polarAngleValue) : facingBucket);
        const polarAngleValue = previous
            ? resolveNearestCircularAngle(inputPolarAngle, previous.polarAngleValue)
            : inputPolarAngle;
        const rotate = previous
            ? resolveNearestCircularAngle(inputRotate, previous.rotate)
            : inputRotate;
        const crossFadeOnly = previous
            ? previousFacingBucket !== facingBucket
                || Math.abs(normalizeRotation(inputRotate - normalizeRotation(previous.rotate))) > LABEL_ROTATION_CROSS_FADE_THRESHOLD
            : false;
        const frame: HistoricalLabelAnimationFrame = {
            ...input,
            radiusValue,
            radiusAxisMaxValue,
            radiusRatio,
            polarAngleValue,
            rotate,
            previous,
            opacity: 1,
            scale: 1,
            leaving: false,
            crossFadeOnly,
            facingBucket
        };

        state?.set(input.stateKey, {
            radiusValue: frame.radiusValue,
            radiusAxisMaxValue: frame.radiusAxisMaxValue,
            radiusRatio: frame.radiusRatio,
            polarAngleValue: frame.polarAngleValue,
            rotate: frame.rotate,
            facingBucket: frame.facingBucket,
            dataId: frame.dataId,
            name: frame.name,
            text: frame.text,
            color: frame.color,
            fontSize: frame.fontSize,
            fontWeight: frame.fontWeight,
            width: frame.width,
            visible: true,
            ghostEmitted: false
        });

        return frame;
    });

    if (!state) {
        return frames;
    }

    for (const [stateKey, snapshot] of Array.from(state.entries())) {
        if (stateKeyPrefix && !stateKey.startsWith(stateKeyPrefix)) {
            continue;
        }

        if (visibleKeys.has(stateKey) || (snapshot.visible === false && snapshot.ghostEmitted)) {
            continue;
        }

        if (
            !isHistoricalLabelSnapshot(snapshot)
            || !snapshot.dataId
            || !snapshot.name
            || !snapshot.text
            || !snapshot.color
            || typeof snapshot.fontSize !== 'number'
            || typeof snapshot.fontWeight !== 'number'
        ) {
            continue;
        }

        const radiusAxisMaxValue = currentRadiusAxisMaxValue > 0
            ? currentRadiusAxisMaxValue
            : snapshot.radiusAxisMaxValue;
        const radiusRatio = clamp(toFiniteNumber(snapshot.radiusRatio, 0), 0, 1);

        frames.push({
            stateKey,
            dataId: snapshot.dataId,
            name: snapshot.name,
            text: snapshot.text,
            radiusValue: radiusAxisMaxValue * radiusRatio,
            radiusAxisMaxValue,
            radiusRatio,
            polarAngleValue: snapshot.polarAngleValue,
            rotate: snapshot.rotate,
            color: snapshot.color,
            fontSize: snapshot.fontSize,
            fontWeight: snapshot.fontWeight,
            width: snapshot.width,
            previous: snapshot,
            opacity: 0,
            scale: 1,
            leaving: true,
            crossFadeOnly: false,
            facingBucket: snapshot.facingBucket ?? getHistoricalLabelFacingBucket(snapshot.polarAngleValue)
        });

        state.set(stateKey, {
            ...snapshot,
            visible: false,
            ghostEmitted: true
        });
    }

    return frames;
}

export function getTangentialTextRotation(angle: number): number {
    let rotation = normalizeRotation(normalizeCircleAngle(angle) - 90);

    if (rotation > 90) {
        rotation -= 180;
    } else if (rotation < -90) {
        rotation += 180;
    }

    return Number(rotation.toFixed(2));
}

function getHistoricalLabelFacingBucket(polarAngleValue: number): number {
    const visualAngle = normalizeCircleAngle(START_ANGLE - normalizeCircleAngle(polarAngleValue));
    const rawTangentialRotation = normalizeRotation(visualAngle - 90);

    return rawTangentialRotation > 90 || rawTangentialRotation < -90 ? -1 : 1;
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
    const padding = Math.max(
        model.amountAxisInterval * SECONDARY_LABEL_BUFFER_INTERVAL_RATIO,
        model.amountAxisMax * SECONDARY_LABEL_MIN_PADDING_RATIO
    );
    const bufferedValue = slot.labelAnchorAmount + padding;
    const cappedValue = model.amountAxisMax * SECONDARY_LABEL_MAX_AXIS_RATIO;

    return Math.min(bufferedValue, cappedValue);
}

function getBarSectorHalfAngle(slotCount: number): number {
    if (slotCount <= 0) {
        return BAR_SECTOR_MIN_HALF_ANGLE;
    }

    return Number(clamp(
        (360 / slotCount) * BAR_SECTOR_WIDTH_RATIO / 2,
        BAR_SECTOR_MIN_HALF_ANGLE,
        BAR_SECTOR_MAX_HALF_ANGLE
    ).toFixed(4));
}

function degreesToRadians(degrees: number): number {
    return (degrees * Math.PI) / 180;
}

function normalizeRadians(radians: number): number {
    const fullCircle = Math.PI * 2;
    return ((radians + Math.PI) % fullCircle + fullCircle) % fullCircle - Math.PI;
}

function resolveNearestCircularRadian(targetAngle: number, previousAngle: number): number {
    if (!Number.isFinite(targetAngle) || !Number.isFinite(previousAngle)) {
        return Number.isFinite(targetAngle) ? targetAngle : 0;
    }

    return previousAngle + normalizeRadians(targetAngle - previousAngle);
}

function interpolateNumber(startValue: number, endValue: number, progress: number): number {
    return startValue + ((endValue - startValue) * progress);
}

interface HistoricalLabelRenderParams {
    dataIndex: number;
    dataIndexInside?: number;
}

interface HistoricalLabelRenderApi {
    coord: (value: number[]) => number[];
}

interface HistoricalLabelKeyframe {
    percent: number;
    x: number;
    y: number;
    rotation: number;
    style: {
        opacity: number;
    };
}

function getFrameByRenderParams(
    frames: HistoricalLabelAnimationFrame[],
    params: HistoricalLabelRenderParams
): HistoricalLabelAnimationFrame | null {
    return frames[params.dataIndex]
        ?? frames[params.dataIndexInside ?? -1]
        ?? null;
}

function getSafeRenderCoord(api: HistoricalLabelRenderApi, value: number[]): [number, number] | null {
    try {
        const [x = 0, y = 0] = api.coord(value);
        if (!Number.isFinite(x) || !Number.isFinite(y)) {
            return null;
        }

        return [x, y];
    } catch {
        return null;
    }
}

function buildHistoricalLabelArcKeyframes(
    frame: HistoricalLabelAnimationFrame,
    api: HistoricalLabelRenderApi
): HistoricalLabelKeyframe[] | undefined {
    const startRadiusRatio = clamp(
        toFiniteNumber(
            frame.previous?.radiusRatio
                ?? ((frame.previous?.radiusValue ?? frame.radiusValue)
                    / Math.max(1, toNonNegativeFiniteNumber(frame.previous?.radiusAxisMaxValue ?? frame.radiusAxisMaxValue))),
            frame.radiusRatio
        ),
        0,
        1
    );
    const startRadiusValue = frame.previous
        ? frame.radiusAxisMaxValue * startRadiusRatio
        : frame.radiusValue;
    const startPolarAngleValue = frame.previous?.polarAngleValue ?? frame.polarAngleValue;
    const startRotate = frame.previous?.rotate ?? frame.rotate;
    const startOpacity = frame.previous ? (frame.previous.visible === false ? 0 : 1) : 0;

    if (frame.leaving && frame.previous) {
        const startCoord = getSafeRenderCoord(api, [startRadiusValue, startPolarAngleValue]);
        if (!startCoord) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startCoord[0],
                y: startCoord[1],
                rotation: degreesToRadians(startRotate),
                style: { opacity: startOpacity }
            },
            {
                percent: HISTORY_LABEL_FADE_OUT_PERCENT,
                x: startCoord[0],
                y: startCoord[1],
                rotation: degreesToRadians(startRotate),
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: startCoord[0],
                y: startCoord[1],
                rotation: degreesToRadians(startRotate),
                style: { opacity: 0 }
            }
        ];
    }

    if (frame.crossFadeOnly && frame.previous) {
        const startCoord = getSafeRenderCoord(api, [startRadiusValue, startPolarAngleValue]);
        const endCoord = getSafeRenderCoord(api, [frame.radiusValue, frame.polarAngleValue]);
        if (!startCoord || !endCoord) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startCoord[0],
                y: startCoord[1],
                rotation: degreesToRadians(startRotate),
                style: { opacity: startOpacity }
            },
            {
                percent: HISTORY_LABEL_FADE_OUT_PERCENT,
                x: startCoord[0],
                y: startCoord[1],
                rotation: degreesToRadians(startRotate),
                style: { opacity: 0 }
            },
            {
                percent: HISTORY_LABEL_FADE_IN_START_PERCENT,
                x: endCoord[0],
                y: endCoord[1],
                rotation: degreesToRadians(frame.rotate),
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: endCoord[0],
                y: endCoord[1],
                rotation: degreesToRadians(frame.rotate),
                style: { opacity: frame.opacity }
            }
        ];
    }

    const startCoord = getSafeRenderCoord(api, [startRadiusValue, startPolarAngleValue]);
    const endCoord = getSafeRenderCoord(api, [frame.radiusValue, frame.polarAngleValue]);
    if (!startCoord || !endCoord) {
        return undefined;
    }

    if (!frame.previous) {
        return [
            {
                percent: 0,
                x: endCoord[0],
                y: endCoord[1],
                rotation: degreesToRadians(frame.rotate),
                style: { opacity: 0 }
            },
            {
                percent: HISTORY_LABEL_FADE_IN_START_PERCENT,
                x: endCoord[0],
                y: endCoord[1],
                rotation: degreesToRadians(frame.rotate),
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: endCoord[0],
                y: endCoord[1],
                rotation: degreesToRadians(frame.rotate),
                style: { opacity: frame.opacity }
            }
        ];
    }

    const motionKeyframes = [0.35, 0.7]
        .map(percent => {
            const radiusValue = interpolateNumber(startRadiusValue, frame.radiusValue, percent);
            const polarAngleValue = interpolateHistoricalPolarAngle(
                startPolarAngleValue,
                frame.polarAngleValue,
                percent
            );
            const rotate = interpolateHistoricalPolarAngle(startRotate, frame.rotate, percent);
            const coord = getSafeRenderCoord(api, [radiusValue, polarAngleValue]);

            if (!coord) {
                return null;
            }

            return {
                percent,
                x: coord[0],
                y: coord[1],
                rotation: degreesToRadians(rotate),
                style: {
                    opacity: interpolateNumber(startOpacity, frame.opacity, percent)
                }
            };
        })
        .filter((keyframe): keyframe is HistoricalLabelKeyframe => keyframe !== null);

    return [
        {
            percent: 0,
            x: startCoord[0],
            y: startCoord[1],
            rotation: degreesToRadians(startRotate),
            style: { opacity: startOpacity }
        },
        ...motionKeyframes,
        {
            percent: 1,
            x: endCoord[0],
            y: endCoord[1],
            rotation: degreesToRadians(frame.rotate),
            style: { opacity: frame.opacity }
        }
    ];
}

function createEmptyHistoricalLabelElement(id: string): Record<string, unknown> {
    return {
        type: 'text',
        id,
        name: id,
        z2: HISTORICAL_LABEL_Z2,
        x: 0,
        y: 0,
        opacity: 0,
        silent: true,
        style: {
            text: '',
            fill: 'transparent',
            fontSize: 1
        }
    };
}

function createHistoricalLabelRenderItem(
    frames: HistoricalLabelAnimationFrame[]
): (params: HistoricalLabelRenderParams, api: HistoricalLabelRenderApi) => Record<string, unknown> {
    return (params, api) => {
        const frame = getFrameByRenderParams(frames, params);
        if (!frame) {
            return createEmptyHistoricalLabelElement(`historical-label-empty-${params.dataIndex ?? 0}`);
        }

        const coord = getSafeRenderCoord(api, [frame.radiusValue, frame.polarAngleValue]);
        if (!coord) {
            return createEmptyHistoricalLabelElement(frame.dataId);
        }

        const [x, y] = coord;
        const keyframes = buildHistoricalLabelArcKeyframes(frame, api);
        const displayKeyframe = keyframes?.[keyframes.length - 1];
        const textAnimation = {
            duration: HISTORY_ANIMATION_DURATION,
            easing: HISTORY_ANIMATION_EASING
        };

        return {
            type: 'text',
            id: frame.dataId,
            name: frame.dataId,
            z2: HISTORICAL_LABEL_Z2,
            x: displayKeyframe?.x ?? x,
            y: displayKeyframe?.y ?? y,
            rotation: displayKeyframe?.rotation ?? degreesToRadians(frame.rotate),
            silent: true,
            transition: ['x', 'y'],
            enterFrom: {
                style: {
                    opacity: 0
                }
            },
            enterAnimation: {
                duration: 360,
                easing: 'cubicOut'
            },
            updateAnimation: textAnimation,
            style: {
                text: frame.text,
                fill: frame.color,
                fontSize: frame.fontSize,
                fontWeight: frame.fontWeight,
                width: frame.width,
                overflow: frame.width ? 'truncate' : undefined,
                align: 'center',
                verticalAlign: 'middle',
                opacity: frame.opacity
            },
            keyframeAnimation: keyframes
                ? {
                    duration: HISTORY_ANIMATION_DURATION,
                    easing: HISTORY_ANIMATION_EASING,
                    keyframes
                }
                : undefined
        };
    };
}

function buildHistoricalLabelCustomSeries(
    name: string,
    polarIndex: number,
    z: number,
    frames: HistoricalLabelAnimationFrame[],
    id = name
): Record<string, unknown> {
    return {
        id,
        name,
        type: 'custom',
        coordinateSystem: 'polar',
        polarIndex,
        z: Math.max(z, HISTORICAL_LABEL_SERIES_Z),
        silent: true,
        clip: false,
        tooltip: { show: false },
        animation: true,
        animationDuration: 520,
        animationDurationUpdate: HISTORY_ANIMATION_DURATION,
        animationEasing: 'cubicOut',
        animationEasingUpdate: HISTORY_ANIMATION_EASING,
        dimensions: ['radius', 'angle', 'rotate'],
        encode: { radius: 0, angle: 1 },
        renderItem: createHistoricalLabelRenderItem(frames),
        data: frames.map(frame => ({
            id: frame.dataId,
            name: frame.name,
            value: [frame.radiusValue, frame.polarAngleValue, frame.rotate],
            label: {
                formatter: frame.text,
                rotate: frame.rotate,
                width: frame.width,
                overflow: frame.width ? 'truncate' : undefined
            }
        }))
    };
}

type HistoricalSectorCollapseMode = 'angle' | 'radius';

interface HistoricalSectorAnimationFrameInput {
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

interface HistoricalSectorAnimationFrame extends HistoricalSectorAnimationFrameInput {
    previous: HistoricalSectorAnimationSnapshot | null;
    leaving: boolean;
}

interface HistoricalSectorRenderParams {
    dataIndex: number;
    dataIndexInside?: number;
    coordSys?: {
        cx?: number;
        cy?: number;
    };
}

type HistoricalSectorRenderApi = HistoricalLabelRenderApi;

interface HistoricalSectorShape {
    cx: number;
    cy: number;
    r0: number;
    r: number;
    startAngle: number;
    endAngle: number;
    clockwise: boolean;
}

interface HistoricalSectorKeyframe {
    percent: number;
    shape: HistoricalSectorShape;
    style: {
        opacity: number;
    };
}

function getClockwiseAngleSpan(startAngleValue: number, endAngleValue: number): number {
    const rawSpan = toFiniteNumber(endAngleValue - startAngleValue, 0);
    if (rawSpan > 0) {
        return rawSpan;
    }

    return normalizeCircleAngle(endAngleValue - startAngleValue) || 0.1;
}

function getSectorCollapsedSnapshot(
    frame: HistoricalSectorAnimationFrameInput
): HistoricalSectorAnimationSnapshot {
    const span = getClockwiseAngleSpan(frame.startAngleValue, frame.endAngleValue);
    const midpoint = frame.startAngleValue + (span / 2);
    const isRadiusCollapse = frame.collapseMode === 'radius';

    return {
        kind: 'sector',
        startAngleValue: isRadiusCollapse ? frame.startAngleValue : midpoint,
        endAngleValue: isRadiusCollapse ? frame.endAngleValue : midpoint,
        innerRadiusValue: frame.innerRadiusValue,
        outerRadiusValue: isRadiusCollapse ? frame.innerRadiusValue : frame.outerRadiusValue,
        radiusAxisMaxValue: frame.radiusAxisMaxValue,
        innerRadiusRatio: frame.innerRadiusRatio ?? 0,
        outerRadiusRatio: isRadiusCollapse
            ? (frame.innerRadiusRatio ?? 0)
            : (frame.outerRadiusRatio ?? 1),
        dataId: frame.dataId,
        name: frame.name,
        color: frame.color,
        opacity: 0,
        borderColor: frame.borderColor,
        borderWidth: frame.borderWidth,
        collapseMode: frame.collapseMode,
        visible: false,
        ghostEmitted: false
    };
}

function resolveHistoricalSectorAnimationFrames(
    inputs: HistoricalSectorAnimationFrameInput[],
    state?: HistoricalLabelAnimationState,
    stateKeyPrefix?: string
): HistoricalSectorAnimationFrame[] {
    const visibleKeys = new Set(inputs.map(input => input.stateKey));
    const currentRadiusAxisMaxValue = inputs.reduce(
        (maxValue, input) => Math.max(maxValue, toNonNegativeFiniteNumber(input.radiusAxisMaxValue)),
        0
    );
    const frames = inputs.map(input => {
        const previousSnapshot = state?.get(input.stateKey);
        const previous = isHistoricalSectorSnapshot(previousSnapshot) && previousSnapshot.visible !== false
            ? previousSnapshot
            : null;
        const normalizedStartAngle = normalizeCircleAngle(input.startAngleValue);
        const spanAngle = getClockwiseAngleSpan(input.startAngleValue, input.endAngleValue);
        const startAngleValue = previous
            ? resolveNearestCircularAngle(normalizedStartAngle, previous.startAngleValue)
            : normalizedStartAngle;
        const radiusAxisMaxValue = Math.max(1, toNonNegativeFiniteNumber(input.radiusAxisMaxValue));
        const innerRadiusValue = toNonNegativeFiniteNumber(input.innerRadiusValue);
        const outerRadiusValue = Math.max(innerRadiusValue, toNonNegativeFiniteNumber(input.outerRadiusValue));
        const innerRadiusRatio = clamp(
            toFiniteNumber(input.innerRadiusRatio ?? (innerRadiusValue / radiusAxisMaxValue)),
            0,
            1
        );
        const outerRadiusRatio = Math.max(
            innerRadiusRatio,
            clamp(toFiniteNumber(input.outerRadiusRatio ?? (outerRadiusValue / radiusAxisMaxValue)), 0, 1)
        );
        const frame: HistoricalSectorAnimationFrame = {
            ...input,
            startAngleValue,
            endAngleValue: startAngleValue + spanAngle,
            innerRadiusValue,
            outerRadiusValue,
            radiusAxisMaxValue,
            innerRadiusRatio,
            outerRadiusRatio,
            opacity: clamp(toFiniteNumber(input.opacity, 1), 0, 1),
            previous,
            leaving: false
        };

        state?.set(input.stateKey, {
            kind: 'sector',
            startAngleValue: frame.startAngleValue,
            endAngleValue: frame.endAngleValue,
            innerRadiusValue: frame.innerRadiusValue,
            outerRadiusValue: frame.outerRadiusValue,
            radiusAxisMaxValue: frame.radiusAxisMaxValue,
            innerRadiusRatio,
            outerRadiusRatio,
            dataId: frame.dataId,
            name: frame.name,
            color: frame.color,
            opacity: frame.opacity,
            borderColor: frame.borderColor,
            borderWidth: frame.borderWidth,
            collapseMode: frame.collapseMode,
            visible: true,
            ghostEmitted: false
        });

        return frame;
    });

    if (!state) {
        return frames;
    }

    for (const [stateKey, snapshot] of Array.from(state.entries())) {
        if (stateKeyPrefix && !stateKey.startsWith(stateKeyPrefix)) {
            continue;
        }

        if (
            visibleKeys.has(stateKey)
            || !isHistoricalSectorSnapshot(snapshot)
            || (snapshot.visible === false && snapshot.ghostEmitted)
            || !snapshot.dataId
            || !snapshot.name
            || !snapshot.color
        ) {
            continue;
        }

        const collapseMode = snapshot.collapseMode ?? 'angle';
        const radiusAxisMaxValue = currentRadiusAxisMaxValue > 0
            ? currentRadiusAxisMaxValue
            : snapshot.radiusAxisMaxValue;
        const collapsed = getSectorCollapsedSnapshot({
            stateKey,
            dataId: snapshot.dataId,
            name: snapshot.name,
            startAngleValue: snapshot.startAngleValue,
            endAngleValue: snapshot.endAngleValue,
            innerRadiusValue: snapshot.innerRadiusValue,
            outerRadiusValue: snapshot.outerRadiusValue,
            radiusAxisMaxValue,
            innerRadiusRatio: snapshot.innerRadiusRatio,
            outerRadiusRatio: snapshot.outerRadiusRatio,
            color: snapshot.color,
            opacity: 0,
            borderColor: snapshot.borderColor,
            borderWidth: snapshot.borderWidth,
            collapseMode
        });

        frames.push({
            stateKey,
            dataId: snapshot.dataId,
            name: snapshot.name,
            startAngleValue: collapsed.startAngleValue,
            endAngleValue: collapsed.endAngleValue,
            innerRadiusValue: collapsed.innerRadiusValue,
            outerRadiusValue: collapsed.outerRadiusValue,
            radiusAxisMaxValue: collapsed.radiusAxisMaxValue,
            innerRadiusRatio: collapsed.innerRadiusRatio,
            outerRadiusRatio: collapsed.outerRadiusRatio,
            color: snapshot.color,
            opacity: 0,
            borderColor: snapshot.borderColor,
            borderWidth: snapshot.borderWidth,
            collapseMode,
            previous: snapshot,
            leaving: true
        });

        state.set(stateKey, {
            ...snapshot,
            visible: false,
            ghostEmitted: true
        });
    }

    return frames;
}

function getFrameBySectorRenderParams(
    frames: HistoricalSectorAnimationFrame[],
    params: HistoricalSectorRenderParams
): HistoricalSectorAnimationFrame | null {
    return frames[params.dataIndex]
        ?? frames[params.dataIndexInside ?? -1]
        ?? null;
}

function createEmptyHistoricalSectorElement(id: string): Record<string, unknown> {
    return {
        type: 'sector',
        id,
        name: id,
        silent: true,
        shape: {
            cx: 0,
            cy: 0,
            r0: 0,
            r: 0,
            startAngle: 0,
            endAngle: 0,
            clockwise: true
        },
        style: {
            fill: 'transparent',
            opacity: 0
        }
    };
}

function getSectorCenter(params: HistoricalSectorRenderParams): [number, number] | null {
    const cx = Number(params.coordSys?.cx);
    const cy = Number(params.coordSys?.cy);

    if (!Number.isFinite(cx) || !Number.isFinite(cy)) {
        return null;
    }

    return [cx, cy];
}

function getPolarRadiusPx(
    api: HistoricalSectorRenderApi,
    center: [number, number],
    radiusValue: number,
    angleValue: number
): number | null {
    const point = getSafeRenderCoord(api, [radiusValue, angleValue]);
    if (!point) {
        return null;
    }

    return Math.hypot(point[0] - center[0], point[1] - center[1]);
}

function getPolarCanvasAngle(
    api: HistoricalSectorRenderApi,
    center: [number, number],
    radiusValue: number,
    angleValue: number
): number | null {
    const point = getSafeRenderCoord(api, [radiusValue, angleValue]);
    if (!point) {
        return null;
    }

    return Math.atan2(point[1] - center[1], point[0] - center[0]);
}

function getHistoricalSectorShape(
    frame: HistoricalSectorAnimationFrameInput | HistoricalSectorAnimationSnapshot,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): HistoricalSectorShape | null {
    const center = getSectorCenter(params);
    if (!center) {
        return null;
    }

    const radiusAxisMaxValue = Math.max(1, toNonNegativeFiniteNumber(frame.radiusAxisMaxValue));
    const minRadius = getPolarRadiusPx(api, center, 0, frame.startAngleValue);
    const maxRadius = getPolarRadiusPx(api, center, radiusAxisMaxValue, frame.startAngleValue);
    const startAngle = getPolarCanvasAngle(api, center, radiusAxisMaxValue, frame.startAngleValue);
    if (minRadius === null || maxRadius === null || startAngle === null) {
        return null;
    }
    const innerRadiusRatio = clamp(toFiniteNumber(frame.innerRadiusRatio ?? 0), 0, 1);
    const outerRadiusRatio = Math.max(
        innerRadiusRatio,
        clamp(toFiniteNumber(frame.outerRadiusRatio ?? 1), 0, 1)
    );
    const radiusSpan = Math.max(0, maxRadius - minRadius);
    const innerRadius = minRadius + (radiusSpan * innerRadiusRatio);
    const outerRadius = minRadius + (radiusSpan * outerRadiusRatio);

    return {
        cx: center[0],
        cy: center[1],
        r0: Math.min(innerRadius, outerRadius),
        r: Math.max(innerRadius, outerRadius),
        startAngle,
        endAngle: startAngle + degreesToRadians(getClockwiseAngleSpan(frame.startAngleValue, frame.endAngleValue)),
        clockwise: true
    };
}

function interpolateHistoricalSectorSnapshot(
    start: HistoricalSectorAnimationSnapshot,
    end: HistoricalSectorAnimationFrame,
    progress: number
): HistoricalSectorAnimationSnapshot {
    return {
        kind: 'sector',
        startAngleValue: interpolateHistoricalPolarAngle(start.startAngleValue, end.startAngleValue, progress),
        endAngleValue: interpolateHistoricalPolarAngle(start.endAngleValue, end.endAngleValue, progress),
        innerRadiusValue: interpolateNumber(start.innerRadiusValue, end.innerRadiusValue, progress),
        outerRadiusValue: interpolateNumber(start.outerRadiusValue, end.outerRadiusValue, progress),
        radiusAxisMaxValue: end.radiusAxisMaxValue,
        innerRadiusRatio: interpolateNumber(start.innerRadiusRatio, end.innerRadiusRatio ?? 0, progress),
        outerRadiusRatio: interpolateNumber(start.outerRadiusRatio, end.outerRadiusRatio ?? 1, progress),
        dataId: end.dataId,
        name: end.name,
        color: end.color,
        opacity: interpolateNumber(start.opacity ?? 1, end.opacity, progress),
        borderColor: end.borderColor,
        borderWidth: end.borderWidth,
        collapseMode: end.collapseMode,
        visible: end.opacity > 0
    };
}

function unwrapHistoricalSectorShape(
    shape: HistoricalSectorShape,
    previousShape: HistoricalSectorShape | null
): HistoricalSectorShape {
    if (!previousShape) {
        return shape;
    }

    const span = Math.max(0.001, shape.endAngle - shape.startAngle);
    const startAngle = resolveNearestCircularRadian(shape.startAngle, previousShape.startAngle);

    return {
        ...shape,
        startAngle,
        endAngle: startAngle + span
    };
}

function buildHistoricalSectorKeyframes(
    frame: HistoricalSectorAnimationFrame,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): HistoricalSectorKeyframe[] | undefined {
    const startFrame = frame.previous ?? getSectorCollapsedSnapshot(frame);
    let previousShape: HistoricalSectorShape | null = null;

    const keyframes = [0, 0.25, 0.5, 0.75, 1].map(percent => {
        const snapshot = interpolateHistoricalSectorSnapshot(startFrame, frame, percent);
        const rawShape = getHistoricalSectorShape(snapshot, params, api);
        if (!rawShape) {
            return null;
        }
        const shape = unwrapHistoricalSectorShape(rawShape, previousShape);
        previousShape = shape;

        return {
            percent,
            shape,
            style: {
                opacity: snapshot.opacity ?? frame.opacity
            }
        };
    }).filter((item): item is HistoricalSectorKeyframe => item !== null);

    return keyframes.length ? keyframes : undefined;
}

function createHistoricalSectorRenderItem(
    frames: HistoricalSectorAnimationFrame[]
): (params: HistoricalSectorRenderParams, api: HistoricalSectorRenderApi) => Record<string, unknown> {
    return (params, api) => {
        const frame = getFrameBySectorRenderParams(frames, params);
        if (!frame) {
            return createEmptyHistoricalSectorElement(`historical-sector-empty-${params.dataIndex ?? 0}`);
        }

        const shape = getHistoricalSectorShape(frame, params, api);
        if (!shape) {
            return createEmptyHistoricalSectorElement(frame.dataId);
        }

        const keyframes = buildHistoricalSectorKeyframes(frame, params, api);
        const displayShape = keyframes?.[keyframes.length - 1]?.shape ?? shape;

        return {
            type: 'sector',
            id: frame.dataId,
            name: frame.dataId,
            silent: true,
            shape: displayShape,
            style: {
                fill: frame.color,
                opacity: frame.opacity,
                stroke: frame.borderColor,
                lineWidth: frame.borderWidth
            },
            enterFrom: {
                style: {
                    opacity: 0
                }
            },
            enterAnimation: {
                duration: 360,
                easing: 'cubicOut'
            },
            updateAnimation: {
                duration: HISTORY_ANIMATION_DURATION,
                easing: HISTORY_ANIMATION_EASING
            },
            keyframeAnimation: keyframes
                ? {
                    duration: HISTORY_ANIMATION_DURATION,
                    easing: HISTORY_ANIMATION_EASING,
                    keyframes
                }
                : undefined
        };
    };
}

function buildHistoricalSectorCustomSeries(
    id: string,
    name: string,
    polarIndex: number,
    z: number,
    frames: HistoricalSectorAnimationFrame[]
): Record<string, unknown> {
    return {
        id,
        name,
        type: 'custom',
        coordinateSystem: 'polar',
        polarIndex,
        z,
        silent: true,
        clip: false,
        tooltip: { show: false },
        animation: true,
        animationTypeUpdate: 'transition',
        animationDuration: HISTORY_ANIMATION_DURATION,
        animationDurationUpdate: HISTORY_ANIMATION_DURATION,
        animationEasing: 'cubicOut',
        animationEasingUpdate: HISTORY_ANIMATION_EASING,
        animationDelay: 0,
        animationDelayUpdate: 0,
        dimensions: ['innerRadius', 'outerRadius', 'startAngle', 'endAngle'],
        encode: { radius: 1, angle: 2 },
        renderItem: createHistoricalSectorRenderItem(frames),
        data: frames.map(frame => ({
            id: frame.dataId,
            name: frame.name,
            value: [
                frame.innerRadiusValue,
                frame.outerRadiusValue,
                frame.startAngleValue,
                frame.endAngleValue
            ],
            itemStyle: {
                color: frame.color,
                opacity: frame.opacity,
                borderColor: frame.borderColor,
                borderWidth: frame.borderWidth
            }
        }))
    };
}

interface HistoricalGridLineAnimationFrameInput {
    stateKey: string;
    dataId: string;
    name: string;
    axisIndex?: number;
    amountValue: number;
    radiusAxisMaxValue: number;
    color: string;
}

interface HistoricalGridLineAnimationFrame extends HistoricalGridLineAnimationFrameInput {
    axisIndex: number;
    radiusRatio: number;
    opacity: number;
    previous: HistoricalGridLineAnimationSnapshot | null;
    leaving: boolean;
    labelGhost: boolean;
    rangeChanged: boolean;
}

interface HistoricalGridLineShape {
    cx: number;
    cy: number;
    r: number;
}

interface HistoricalGridLineKeyframe {
    percent: number;
    shape: HistoricalGridLineShape;
    style: {
        opacity: number;
    };
}

interface HistoricalAmountAxisLabelKeyframe {
    percent: number;
    x: number;
    y: number;
    style: {
        opacity: number;
    };
}

function formatHistoricalGridAmountKey(amountValue: number): string {
    return Number(amountValue.toFixed(4)).toString();
}

function getHistoricalAmountGridLineValues(amountAxisMax: number): number[] {
    const maxValue = toNonNegativeFiniteNumber(amountAxisMax);
    if (maxValue <= 0) {
        return [];
    }

    const step = maxValue / (AMOUNT_AXIS_GRID_LINE_COUNT - 1);

    return Array.from({ length: AMOUNT_AXIS_GRID_LINE_COUNT }, (_, index) => (
        Number((step * index).toFixed(4))
    ));
}

type HistoricalAmountAxisFrameMode = 'line' | 'label';

function getHistoricalAmountAxisFrameIndex(stateKey: string): number {
    const matched = /:(\d+)$/.exec(stateKey);

    return matched ? Number(matched[1]) : 0;
}

function collectHistoricalAmountAxisSnapshots(
    state: HistoricalLabelAnimationState | undefined,
    stateKeyPrefix: string | undefined
): Map<number, HistoricalGridLineAnimationSnapshot> {
    const snapshots = new Map<number, HistoricalGridLineAnimationSnapshot>();
    if (!state || !stateKeyPrefix) {
        return snapshots;
    }

    for (const [stateKey, snapshot] of state.entries()) {
        if (
            !stateKey.startsWith(stateKeyPrefix)
            || !isHistoricalGridLineSnapshot(snapshot)
            || snapshot.visible === false
        ) {
            continue;
        }

        const axisIndex = Number.isFinite(snapshot.axisIndex)
            ? Number(snapshot.axisIndex)
            : getHistoricalAmountAxisFrameIndex(stateKey);
        snapshots.set(axisIndex, snapshot);
    }

    return snapshots;
}

function getHistoricalAmountAxisPreviousMaxValue(
    snapshots: Map<number, HistoricalGridLineAnimationSnapshot>
): number {
    return Math.max(
        0,
        ...Array.from(snapshots.values()).map(snapshot => toNonNegativeFiniteNumber(snapshot.radiusAxisMaxValue))
    );
}

function resolveHistoricalAmountAxisSourceIndex(
    axisIndex: number,
    previousMaxValue: number,
    currentMaxValue: number
): number | null {
    if (previousMaxValue <= 0 || Math.abs(previousMaxValue - currentMaxValue) < 0.0001) {
        return axisIndex;
    }

    if (currentMaxValue < previousMaxValue) {
        return Math.max(0, axisIndex - 1);
    }

    if (axisIndex === 0) {
        return 0;
    }

    if (axisIndex >= AMOUNT_AXIS_GRID_LINE_COUNT - 1) {
        return null;
    }

    return axisIndex + 1;
}

function buildHistoricalAmountAxisLeavingFrames(
    previousSnapshots: Map<number, HistoricalGridLineAnimationSnapshot>,
    previousMaxValue: number,
    currentMaxValue: number,
    color: string,
    mode: HistoricalAmountAxisFrameMode
): HistoricalGridLineAnimationFrame[] {
    if (previousMaxValue <= 0 || Math.abs(previousMaxValue - currentMaxValue) < 0.0001) {
        return [];
    }

    const leavingIndices = mode === 'label'
        ? Array.from(previousSnapshots.keys())
        : [currentMaxValue < previousMaxValue ? AMOUNT_AXIS_GRID_LINE_COUNT - 1 : 1];

    return leavingIndices.flatMap(axisIndex => {
        const previous = previousSnapshots.get(axisIndex);
        if (!previous?.dataId) {
            return [];
        }

        return [{
            stateKey: `amount-axis-ghost:${mode}:${axisIndex}:${previous.dataId}`,
            dataId: previous.dataId,
            name: previous.name ?? formatHistoricalGridAmountKey(previous.amountValue),
            axisIndex,
            amountValue: previous.amountValue,
            radiusAxisMaxValue: currentMaxValue > 0 ? currentMaxValue : previous.radiusAxisMaxValue,
            radiusRatio: clamp(toFiniteNumber(previous.radiusRatio, 0), 0, 1),
            color: previous.color ?? color,
            opacity: 0,
            previous,
            leaving: true,
            labelGhost: mode === 'label',
            rangeChanged: true
        }];
    });
}

function resolveHistoricalGridLineAnimationFrames(
    inputs: HistoricalGridLineAnimationFrameInput[],
    state?: HistoricalLabelAnimationState,
    stateKeyPrefix?: string,
    mode: HistoricalAmountAxisFrameMode = 'line'
): HistoricalGridLineAnimationFrame[] {
    const visibleKeys = new Set(inputs.map(input => input.stateKey));
    const currentRadiusAxisMaxValue = inputs.reduce(
        (maxValue, input) => Math.max(maxValue, toNonNegativeFiniteNumber(input.radiusAxisMaxValue)),
        0
    );
    const previousSnapshots = collectHistoricalAmountAxisSnapshots(state, stateKeyPrefix);
    const previousMaxValue = getHistoricalAmountAxisPreviousMaxValue(previousSnapshots);
    const rangeChanged = previousMaxValue > 0 && Math.abs(previousMaxValue - currentRadiusAxisMaxValue) >= 0.0001;
    const frames = inputs.map(input => {
        const axisIndex = Number.isFinite(input.axisIndex)
            ? Number(input.axisIndex)
            : getHistoricalAmountAxisFrameIndex(input.stateKey);
        const sourceIndex = resolveHistoricalAmountAxisSourceIndex(
            axisIndex,
            previousMaxValue,
            currentRadiusAxisMaxValue
        );
        const previousSnapshot = sourceIndex === null
            ? undefined
            : previousSnapshots.get(sourceIndex) ?? state?.get(input.stateKey);
        const previous = isHistoricalGridLineSnapshot(previousSnapshot) && previousSnapshot.visible !== false
            ? previousSnapshot
            : null;
        const radiusAxisMaxValue = Math.max(1, toNonNegativeFiniteNumber(input.radiusAxisMaxValue));
        const amountValue = toNonNegativeFiniteNumber(input.amountValue);
        const radiusRatio = clamp(amountValue / radiusAxisMaxValue, 0, 1);
        const frame: HistoricalGridLineAnimationFrame = {
            ...input,
            axisIndex,
            amountValue,
            radiusAxisMaxValue,
            radiusRatio,
            opacity: 1,
            previous,
            leaving: false,
            labelGhost: false,
            rangeChanged
        };

        state?.set(input.stateKey, {
            kind: 'grid-line',
            axisIndex,
            amountValue: frame.amountValue,
            radiusAxisMaxValue: frame.radiusAxisMaxValue,
            radiusRatio: frame.radiusRatio,
            dataId: frame.dataId,
            name: frame.name,
            color: frame.color,
            visible: true,
            ghostEmitted: false
        });

        return frame;
    });

    frames.push(...buildHistoricalAmountAxisLeavingFrames(
        previousSnapshots,
        previousMaxValue,
        currentRadiusAxisMaxValue,
        inputs[0]?.color ?? AMOUNT_AXIS_SPLIT_LINE_DARK_COLOR,
        mode
    ));

    if (!state) {
        return frames;
    }

    for (const [stateKey, snapshot] of Array.from(state.entries())) {
        if (stateKeyPrefix && !stateKey.startsWith(stateKeyPrefix)) {
            continue;
        }

        if (
            visibleKeys.has(stateKey)
            || !isHistoricalGridLineSnapshot(snapshot)
            || (snapshot.visible === false && snapshot.ghostEmitted)
            || !snapshot.dataId
            || !snapshot.name
            || !snapshot.color
        ) {
            continue;
        }

        const radiusAxisMaxValue = currentRadiusAxisMaxValue > 0
            ? currentRadiusAxisMaxValue
            : snapshot.radiusAxisMaxValue;
        const radiusRatio = clamp(toFiniteNumber(snapshot.radiusRatio, 0), 0, 1);

        frames.push({
            stateKey,
            dataId: snapshot.dataId,
            name: snapshot.name,
            axisIndex: Number.isFinite(snapshot.axisIndex)
                ? Number(snapshot.axisIndex)
                : getHistoricalAmountAxisFrameIndex(stateKey),
            amountValue: radiusAxisMaxValue * radiusRatio,
            radiusAxisMaxValue,
            radiusRatio,
            color: snapshot.color,
            opacity: 0,
            previous: snapshot,
            leaving: true,
            labelGhost: false,
            rangeChanged: false
        });

        state.set(stateKey, {
            ...snapshot,
            visible: false,
            ghostEmitted: true
        });
    }

    return frames;
}

function getFrameByGridLineRenderParams(
    frames: HistoricalGridLineAnimationFrame[],
    params: HistoricalSectorRenderParams
): HistoricalGridLineAnimationFrame | null {
    return frames[params.dataIndex]
        ?? frames[params.dataIndexInside ?? -1]
        ?? null;
}

function getHistoricalGridLineShape(
    frame: Pick<HistoricalGridLineAnimationFrame, 'radiusAxisMaxValue' | 'radiusRatio'>,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): HistoricalGridLineShape | null {
    const center = getSectorCenter(params);
    if (!center) {
        return null;
    }

    const radiusAxisMaxValue = Math.max(1, toNonNegativeFiniteNumber(frame.radiusAxisMaxValue));
    const minRadius = getPolarRadiusPx(api, center, 0, 0);
    const maxRadius = getPolarRadiusPx(api, center, radiusAxisMaxValue, 0);
    if (minRadius === null || maxRadius === null) {
        return null;
    }

    const radiusSpan = Math.max(0, maxRadius - minRadius);

    return {
        cx: center[0],
        cy: center[1],
        r: minRadius + (
            radiusSpan * clamp(toFiniteNumber(frame.radiusRatio, 0), 0, AMOUNT_AXIS_MAX_RENDER_RADIUS_RATIO)
        )
    };
}

function resolveHistoricalAmountAxisStartRadiusRatio(frame: HistoricalGridLineAnimationFrame): number {
    if (!frame.previous) {
        return clamp(toFiniteNumber(frame.radiusRatio, 0), 0, 1);
    }

    return clamp(toFiniteNumber(frame.previous.radiusRatio, frame.radiusRatio), 0, 1);
}

function getHistoricalAmountAxisTransitionEdgeRatio(radiusRatio: number): number {
    return clamp(toFiniteNumber(radiusRatio, 0), 0, 1) >= 0.5
        ? AMOUNT_AXIS_OUTER_TRANSITION_RADIUS_RATIO
        : 0;
}

function isHistoricalAmountAxisRenderRestart(frame: HistoricalGridLineAnimationFrame): boolean {
    return !!frame.previous && frame.previous.dataId !== frame.dataId;
}

function buildHistoricalGridLineKeyframes(
    frame: HistoricalGridLineAnimationFrame,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): HistoricalGridLineKeyframe[] | undefined {
    const startRadiusRatio = resolveHistoricalAmountAxisStartRadiusRatio(frame);
    const startOpacity = frame.previous ? (frame.previous.visible === false ? 0 : 1) : 0;

    if (frame.leaving) {
        const endRadiusRatio = getHistoricalAmountAxisTransitionEdgeRatio(startRadiusRatio);
        const startShape = getHistoricalGridLineShape(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: startRadiusRatio
            },
            params,
            api
        );
        const endShape = getHistoricalGridLineShape(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: endRadiusRatio
            },
            params,
            api
        );
        if (!startShape || !endShape) {
            return undefined;
        }

        return [
            {
                percent: 0,
                shape: startShape,
                style: { opacity: startOpacity }
            },
            {
                percent: 0.62,
                shape: getHistoricalGridLineShape(
                    {
                        radiusAxisMaxValue: frame.radiusAxisMaxValue,
                        radiusRatio: interpolateNumber(startRadiusRatio, endRadiusRatio, 0.62)
                    },
                    params,
                    api
                ) ?? startShape,
                style: { opacity: Math.max(0, startOpacity * 0.35) }
            },
            {
                percent: 1,
                shape: endShape,
                style: { opacity: 0 }
            }
        ];
    }

    if (!frame.previous) {
        const appearStartRatio = getHistoricalAmountAxisTransitionEdgeRatio(frame.radiusRatio);
        const startShape = getHistoricalGridLineShape(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: appearStartRatio
            },
            params,
            api
        );
        const endShape = getHistoricalGridLineShape(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: frame.radiusRatio
            },
            params,
            api
        );
        if (!startShape || !endShape) {
            return undefined;
        }

        return [
            {
                percent: 0,
                shape: startShape,
                style: { opacity: 0 }
            },
            {
                percent: HISTORY_AMOUNT_AXIS_FADE_IN_START_PERCENT,
                shape: startShape,
                style: { opacity: 0 }
            },
            {
                percent: 1,
                shape: endShape,
                style: { opacity: frame.opacity }
            }
        ];
    }

    const keyframes = [0, 0.25, 0.5, 0.75, 1].map(percent => {
        const shape = getHistoricalGridLineShape(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: interpolateNumber(startRadiusRatio, frame.radiusRatio, percent)
            },
            params,
            api
        );
        if (!shape) {
            return null;
        }

        return {
            percent,
            shape,
            style: {
                opacity: interpolateNumber(startOpacity, frame.opacity, percent)
            }
        };
    }).filter((item): item is HistoricalGridLineKeyframe => item !== null);

    return keyframes.length ? keyframes : undefined;
}

function createEmptyHistoricalGridLineElement(id: string): Record<string, unknown> {
    return {
        type: 'circle',
        id,
        name: id,
        silent: true,
        shape: {
            cx: 0,
            cy: 0,
            r: 0
        },
        style: {
            fill: 'transparent',
            stroke: 'transparent',
            opacity: 0
        }
    };
}

function createHistoricalGridLineRenderItem(
    frames: HistoricalGridLineAnimationFrame[]
): (params: HistoricalSectorRenderParams, api: HistoricalSectorRenderApi) => Record<string, unknown> {
    return (params, api) => {
        const frame = getFrameByGridLineRenderParams(frames, params);
        if (!frame) {
            return createEmptyHistoricalGridLineElement(`historical-grid-empty-${params.dataIndex ?? 0}`);
        }

        const shape = getHistoricalGridLineShape(frame, params, api);
        if (!shape) {
            return createEmptyHistoricalGridLineElement(frame.dataId);
        }

        const keyframes = buildHistoricalGridLineKeyframes(frame, params, api);
        const isNewRenderElement = !frame.previous || frame.previous.dataId !== frame.dataId;
        const displayKeyframe = isNewRenderElement
            ? keyframes?.[0]
            : keyframes?.[keyframes.length - 1];
        const displayShape = displayKeyframe?.shape ?? shape;
        const displayOpacity = displayKeyframe?.style?.opacity ?? frame.opacity;

        return {
            type: 'circle',
            id: frame.dataId,
            name: frame.dataId,
            silent: true,
            shape: displayShape,
            style: {
                fill: 'transparent',
                stroke: frame.color,
                lineWidth: 1,
                lineDash: [4, 4],
                opacity: displayOpacity
            },
            updateAnimation: {
                duration: HISTORY_ANIMATION_DURATION,
                easing: HISTORY_ANIMATION_EASING
            },
            keyframeAnimation: keyframes
                ? {
                    duration: HISTORY_ANIMATION_DURATION,
                    easing: HISTORY_ANIMATION_EASING,
                    keyframes
                }
                : undefined
        };
    };
}

function buildHistoricalGridLineCustomSeries(
    frames: HistoricalGridLineAnimationFrame[]
): Record<string, unknown> {
    return {
        id: 'amount-grid-lines',
        name: 'amount-grid-lines',
        type: 'custom',
        coordinateSystem: 'polar',
        polarIndex: 0,
        z: 0,
        silent: true,
        clip: false,
        tooltip: { show: false },
        animation: true,
        animationDuration: HISTORY_ANIMATION_DURATION,
        animationDurationUpdate: HISTORY_ANIMATION_DURATION,
        animationEasing: 'cubicOut',
        animationEasingUpdate: HISTORY_ANIMATION_EASING,
        dimensions: ['radius', 'angle'],
        encode: { radius: 0, angle: 1 },
        renderItem: createHistoricalGridLineRenderItem(frames),
        data: frames.map(frame => ({
            id: frame.dataId,
            name: frame.name,
            value: [frame.amountValue, 0],
            itemStyle: {
                color: frame.color,
                opacity: frame.opacity
            }
        }))
    };
}

function getHistoricalAmountAxisLabelPosition(
    frame: Pick<HistoricalGridLineAnimationFrame, 'radiusAxisMaxValue' | 'radiusRatio'>,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): { x: number; y: number } | null {
    const shape = getHistoricalGridLineShape(frame, params, api);
    if (!shape) {
        return null;
    }

    return {
        x: shape.cx,
        y: shape.cy - shape.r - AMOUNT_AXIS_LABEL_OFFSET_Y
    };
}

function buildHistoricalAmountAxisLabelKeyframes(
    frame: HistoricalGridLineAnimationFrame,
    params: HistoricalSectorRenderParams,
    api: HistoricalSectorRenderApi
): HistoricalAmountAxisLabelKeyframe[] | undefined {
    const startRadiusRatio = resolveHistoricalAmountAxisStartRadiusRatio(frame);
    const startOpacity = frame.previous ? (frame.previous.visible === false ? 0 : 1) : 0;

    if (frame.labelGhost) {
        const startPosition = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: startRadiusRatio
            },
            params,
            api
        );
        if (!startPosition) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: startOpacity }
            },
            {
                percent: HISTORY_LABEL_FADE_OUT_PERCENT,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: 0 }
            }
        ];
    }

    if (frame.leaving) {
        const endRadiusRatio = getHistoricalAmountAxisTransitionEdgeRatio(startRadiusRatio);
        const startPosition = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: startRadiusRatio
            },
            params,
            api
        );
        if (!startPosition) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: startOpacity }
            },
            {
                percent: 0.62,
                ...(
                    getHistoricalAmountAxisLabelPosition(
                        {
                            radiusAxisMaxValue: frame.radiusAxisMaxValue,
                            radiusRatio: interpolateNumber(startRadiusRatio, endRadiusRatio, 0.62)
                        },
                        params,
                        api
                    ) ?? startPosition
                ),
                style: { opacity: Math.max(0, startOpacity * 0.35) }
            },
            {
                percent: 1,
                ...(
                    getHistoricalAmountAxisLabelPosition(
                        {
                            radiusAxisMaxValue: frame.radiusAxisMaxValue,
                            radiusRatio: endRadiusRatio
                        },
                        params,
                        api
                    ) ?? startPosition
                ),
                style: { opacity: 0 }
            }
        ];
    }

    if (frame.rangeChanged) {
        const startPosition = frame.previous
            ? getHistoricalAmountAxisLabelPosition(
                {
                    radiusAxisMaxValue: frame.radiusAxisMaxValue,
                    radiusRatio: startRadiusRatio
                },
                params,
                api
            )
            : null;
        const endPosition = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: frame.radiusRatio
            },
            params,
            api
        );
        if (!endPosition) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startPosition?.x ?? endPosition.x,
                y: startPosition?.y ?? endPosition.y,
                style: { opacity: 0 }
            },
            {
                percent: HISTORY_LABEL_FADE_IN_START_PERCENT,
                x: endPosition.x,
                y: endPosition.y,
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: endPosition.x,
                y: endPosition.y,
                style: { opacity: frame.opacity }
            }
        ];
    }

    if (!frame.previous || isHistoricalAmountAxisRenderRestart(frame)) {
        const appearStartRatio = getHistoricalAmountAxisTransitionEdgeRatio(frame.radiusRatio);
        const startPosition = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: appearStartRatio
            },
            params,
            api
        );
        const endPosition = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: frame.radiusRatio
            },
            params,
            api
        );
        if (!startPosition || !endPosition) {
            return undefined;
        }

        return [
            {
                percent: 0,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: 0 }
            },
            {
                percent: HISTORY_LABEL_FADE_IN_START_PERCENT,
                x: startPosition.x,
                y: startPosition.y,
                style: { opacity: 0 }
            },
            {
                percent: 1,
                x: endPosition.x,
                y: endPosition.y,
                style: { opacity: frame.opacity }
            }
        ];
    }

    const keyframes = [0, 0.25, 0.5, 0.75, 1].map(percent => {
        const position = getHistoricalAmountAxisLabelPosition(
            {
                radiusAxisMaxValue: frame.radiusAxisMaxValue,
                radiusRatio: interpolateNumber(startRadiusRatio, frame.radiusRatio, percent)
            },
            params,
            api
        );
        if (!position) {
            return null;
        }

        return {
            percent,
            x: position.x,
            y: position.y,
            style: {
                opacity: interpolateNumber(startOpacity, frame.opacity, percent)
            }
        };
    }).filter((item): item is HistoricalAmountAxisLabelKeyframe => item !== null);

    return keyframes.length ? keyframes : undefined;
}

function createEmptyHistoricalAmountAxisLabelElement(id: string): Record<string, unknown> {
    return {
        type: 'text',
        id,
        name: id,
        z2: HISTORICAL_LABEL_Z2,
        x: 0,
        y: 0,
        opacity: 0,
        silent: true,
        style: {
            text: '',
            fill: 'transparent',
            fontSize: 1
        }
    };
}

function createHistoricalAmountAxisLabelRenderItem(
    frames: HistoricalGridLineAnimationFrame[],
    formatAmount: (amount: number) => string,
    labelColor: string
): (params: HistoricalSectorRenderParams, api: HistoricalSectorRenderApi) => Record<string, unknown> {
    return (params, api) => {
        const frame = getFrameByGridLineRenderParams(frames, params);
        if (!frame) {
            return createEmptyHistoricalAmountAxisLabelElement(`historical-amount-label-empty-${params.dataIndex ?? 0}`);
        }

        const position = getHistoricalAmountAxisLabelPosition(frame, params, api);
        if (!position) {
            return createEmptyHistoricalAmountAxisLabelElement(frame.dataId);
        }

        const keyframes = buildHistoricalAmountAxisLabelKeyframes(frame, params, api);
        const isNewRenderElement = !frame.previous || frame.previous.dataId !== frame.dataId;
        const displayKeyframe = isNewRenderElement
            ? keyframes?.[0]
            : keyframes?.[keyframes.length - 1];
        const displayOpacity = displayKeyframe?.style?.opacity ?? frame.opacity;

        return {
            type: 'text',
            id: frame.dataId,
            name: frame.dataId,
            z2: HISTORICAL_LABEL_Z2 - 1,
            x: displayKeyframe?.x ?? position.x,
            y: displayKeyframe?.y ?? position.y,
            rotation: 0,
            silent: true,
            transition: ['x', 'y'],
            updateAnimation: {
                duration: HISTORY_ANIMATION_DURATION,
                easing: HISTORY_ANIMATION_EASING
            },
            style: {
                text: formatAmount(frame.amountValue),
                fill: labelColor,
                fontSize: 10,
                fontWeight: 700,
                align: 'center',
                verticalAlign: 'bottom',
                opacity: displayOpacity
            },
            keyframeAnimation: keyframes
                ? {
                    duration: HISTORY_ANIMATION_DURATION,
                    easing: HISTORY_ANIMATION_EASING,
                    keyframes
                }
                : undefined
        };
    };
}

function buildHistoricalAmountAxisLabelCustomSeries(
    frames: HistoricalGridLineAnimationFrame[],
    formatAmount: (amount: number) => string,
    labelColor: string
): Record<string, unknown> {
    return {
        id: 'amount-axis-labels',
        name: 'amount-axis-labels',
        type: 'custom',
        coordinateSystem: 'polar',
        polarIndex: 0,
        z: 1,
        silent: true,
        clip: false,
        tooltip: { show: false },
        animation: true,
        animationDuration: HISTORY_ANIMATION_DURATION,
        animationDurationUpdate: HISTORY_ANIMATION_DURATION,
        animationEasing: 'cubicOut',
        animationEasingUpdate: HISTORY_ANIMATION_EASING,
        dimensions: ['radius', 'angle'],
        encode: { radius: 0, angle: 1 },
        renderItem: createHistoricalAmountAxisLabelRenderItem(frames, formatAmount, labelColor),
        data: frames.map(frame => ({
            id: frame.dataId,
            name: frame.name,
            value: [frame.amountValue, 0],
            label: {
                formatter: formatAmount(frame.amountValue),
                rotate: 0
            },
            itemStyle: {
                color: labelColor,
                opacity: frame.opacity
            }
        }))
    };
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
                budgetAmount: item.budgetAmount,
                spentAmount: item.spentAmount,
                executionRate: item.executionRate,
                labelAnchorAmount: Math.max(item.budgetAmount, item.spentAmount),
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
        primaryPadAngle
    };
}

export function buildHistoricalPolarChartOption(
    model: HistoricalPolarChartModel,
    args: HistoricalPolarChartOptionArgs
): Record<string, unknown> {
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
                formatter: (value: number) => args.formatAmount(value)
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
                    name: args.formatAmount(amountValue),
                    amountValue,
                    radiusAxisMaxValue: model.amountAxisMax,
                    color: amountAxisLabelColor
                };
            }),
            args.labelAnimationState,
            'grid-label:amount:',
            'label'
        ),
        args.formatAmount,
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
                        outerRadiusValue: slot.budgetAmount,
                        radiusAxisMaxValue: model.amountAxisMax,
                        innerRadiusRatio: 0,
                        outerRadiusRatio: model.amountAxisMax > 0 ? slot.budgetAmount / model.amountAxisMax : 0,
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
                        outerRadiusValue: slot.spentAmount,
                        radiusAxisMaxValue: model.amountAxisMax,
                        innerRadiusRatio: 0,
                        outerRadiusRatio: model.amountAxisMax > 0 ? slot.spentAmount / model.amountAxisMax : 0,
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
