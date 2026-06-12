const INTEGER_PATTERN = /^[+-]?\d+$/u;

export function parseStrictIntegerCents(value: unknown): number | null {
    if (typeof value === 'number') {
        return Number.isSafeInteger(value) ? value : null;
    }

    if (typeof value === 'string') {
        const text = value.trim();
        if (!INTEGER_PATTERN.test(text)) {
            return null;
        }
        const parsed = Number(text);
        return Number.isSafeInteger(parsed) ? parsed : null;
    }

    return null;
}

export function normalizeStrictAbsoluteCents(value: unknown, fallbackCents: number): number {
    const parsed = parseStrictIntegerCents(value);
    return parsed === null ? fallbackCents : Math.abs(parsed);
}

export function requireStrictIntegerCents(value: unknown, fieldName: string): number {
    const parsed = parseStrictIntegerCents(value);
    if (parsed === null) {
        throw new TypeError(`${fieldName} must be integer cents.`);
    }
    return parsed;
}
