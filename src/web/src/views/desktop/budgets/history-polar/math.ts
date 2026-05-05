export function clamp(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
}

export function toFiniteNumber(value: number | undefined, fallback = 0): number {
    const numericValue = Number(value);
    return Number.isFinite(numericValue) ? numericValue : fallback;
}

export function toNonNegativeFiniteNumber(value: number | undefined): number {
    return Math.max(0, toFiniteNumber(value));
}

export function toSortOrder(value: number): number {
    return toFiniteNumber(value, Number.MAX_SAFE_INTEGER);
}

export function normalizeText(value: string): string {
    return String(value ?? '').trim();
}

export function withAlpha(color: string, alpha: number): string {
    const normalizedColor = normalizeText(color).replace('#', '');
    if (!/^[0-9a-fA-F]{6}$/.test(normalizedColor)) {
        return `rgba(0,0,0,${alpha})`;
    }

    const matched = normalizedColor.match(/.{2}/g)!;
    const [red, green, blue] = matched.map(item => parseInt(item, 16));
    return `rgba(${red},${green},${blue},${alpha})`;
}

export function normalizeCircleAngle(angle: number): number {
    if (!Number.isFinite(angle)) {
        return 0;
    }

    return ((angle % 360) + 360) % 360;
}

export function normalizeRotation(rotation: number): number {
    return ((rotation + 180) % 360 + 360) % 360 - 180;
}

export function degreesToRadians(degrees: number): number {
    return (degrees * Math.PI) / 180;
}

export function normalizeRadians(radians: number): number {
    const fullCircle = Math.PI * 2;
    return ((radians + Math.PI) % fullCircle + fullCircle) % fullCircle - Math.PI;
}

export function resolveNearestCircularRadian(targetAngle: number, previousAngle: number): number {
    if (!Number.isFinite(targetAngle) || !Number.isFinite(previousAngle)) {
        return Number.isFinite(targetAngle) ? targetAngle : 0;
    }

    return previousAngle + normalizeRadians(targetAngle - previousAngle);
}

export function interpolateNumber(startValue: number, endValue: number, progress: number): number {
    return startValue + ((endValue - startValue) * progress);
}

export function formatHistoricalGridAmountKey(amountValue: number): string {
    return Number(amountValue.toFixed(4)).toString();
}
