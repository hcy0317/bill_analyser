import type {
    HistoricalChartSlot,
    HistoricalPolarChartModel,
    HistoricalPrimaryBand,
    HistoricalPrimaryLabel
} from './types.ts';
import {
    BAR_SECTOR_MAX_HALF_ANGLE,
    BAR_SECTOR_MIN_HALF_ANGLE,
    BAR_SECTOR_WIDTH_RATIO,
    PRIMARY_LABEL_MAX_FONT_SIZE,
    PRIMARY_LABEL_MIN_FONT_SIZE,
    PRIMARY_LABEL_RADIUS_VALUE,
    PRIMARY_RING_FULL_CIRCLE_EPSILON,
    SECONDARY_LABEL_BUFFER_INTERVAL_RATIO,
    SECONDARY_LABEL_MAX_AXIS_RATIO,
    SECONDARY_LABEL_MIN_PADDING_RATIO,
    START_ANGLE
} from './constants.ts';
import {
    clamp,
    normalizeCircleAngle,
    normalizeRotation,
    toFiniteNumber
} from './math.ts';

export function getRenderablePrimaryRingSpanAngle(spanAngle: number): number {
    const normalizedSpan = clamp(toFiniteNumber(spanAngle), 0, 360);

    return normalizedSpan >= 360
        ? 360 - PRIMARY_RING_FULL_CIRCLE_EPSILON
        : normalizedSpan;
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

export function getTangentialTextRotation(angle: number): number {
    let rotation = normalizeRotation(normalizeCircleAngle(angle) - 90);

    if (rotation > 90) {
        rotation -= 180;
    } else if (rotation < -90) {
        rotation += 180;
    }

    return Number(rotation.toFixed(2));
}

export function getHistoricalLabelFacingBucket(polarAngleValue: number): number {
    const visualAngle = normalizeCircleAngle(START_ANGLE - normalizeCircleAngle(polarAngleValue));
    const rawTangentialRotation = normalizeRotation(visualAngle - 90);

    return rawTangentialRotation > 90 || rawTangentialRotation < -90 ? -1 : 1;
}

export function buildPrimaryLabels(primaryBands: HistoricalPrimaryBand[]): HistoricalPrimaryLabel[] {
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

export function getPolarAngleAxisValue(angle: number): number {
    return Number(normalizeCircleAngle(START_ANGLE - angle).toFixed(2));
}

export function getSecondaryLabelValue(slot: HistoricalChartSlot, model: HistoricalPolarChartModel): number {
    const padding = Math.max(
        model.amountAxisInterval * SECONDARY_LABEL_BUFFER_INTERVAL_RATIO,
        model.amountAxisMax * SECONDARY_LABEL_MIN_PADDING_RATIO
    );
    const bufferedValue = slot.labelAnchorAmount + padding;
    const cappedValue = model.amountAxisMax * SECONDARY_LABEL_MAX_AXIS_RATIO;

    return Math.min(bufferedValue, cappedValue);
}

export function getBarSectorHalfAngle(slotCount: number): number {
    if (slotCount <= 0) {
        return BAR_SECTOR_MIN_HALF_ANGLE;
    }

    return Number(clamp(
        (360 / slotCount) * BAR_SECTOR_WIDTH_RATIO / 2,
        BAR_SECTOR_MIN_HALF_ANGLE,
        BAR_SECTOR_MAX_HALF_ANGLE
    ).toFixed(4));
}
