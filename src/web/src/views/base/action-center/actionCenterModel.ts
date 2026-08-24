export type ActionCenterAnomalyType = 'large_transaction' | 'duplicate_charge' | 'category_spike';
export type ActionCenterSeverity = 'error' | 'warning' | 'info';
export type ActionCenterPlatform = 'desktop' | 'mobile';

export interface ActionCenterAnomaly {
    key: string;
    type: ActionCenterAnomalyType;
    severity: ActionCenterSeverity;
    billIds: number[];
    amountCents: number;
    occurredOn: string;
    category: string;
    counterparty: string;
    description: string;
    message: string;
}

export interface ActionCenterAnomalyResponse {
    items: ActionCenterAnomaly[];
    totalCount: number;
    analyzedBills: number;
    analyzedMonths: number;
    startDate: string;
    endDate: string;
}

export interface DesktopActionTarget {
    path: string;
    query?: Record<string, string>;
}

export interface ActionCenterSummary {
    total: number;
    anomalyCount: number;
    recurringCount: number;
    hasWork: boolean;
}

function toRecord(value: unknown): Record<string, unknown> {
    return value && typeof value === 'object' && !Array.isArray(value)
        ? value as Record<string, unknown>
        : {};
}

function toString(value: unknown): string {
    return typeof value === 'string' ? value.trim() : '';
}

function toNonNegativeInteger(value: unknown): number {
    const parsed = typeof value === 'number'
        ? value
        : typeof value === 'string' && /^\d+$/u.test(value.trim())
            ? Number(value)
            : 0;
    return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : 0;
}

function toPositiveId(value: unknown): number | null {
    const parsed = toNonNegativeInteger(value);
    return parsed > 0 ? parsed : null;
}

function anomalyType(value: unknown): ActionCenterAnomalyType | null {
    return value === 'large_transaction' || value === 'duplicate_charge' || value === 'category_spike'
        ? value
        : null;
}

function severity(value: unknown): ActionCenterSeverity {
    return value === 'error' || value === 'warning' || value === 'info' ? value : 'info';
}

function billIds(record: Record<string, unknown>): number[] {
    const candidates = Array.isArray(record['billIds'])
        ? record['billIds']
        : [record['billId']];
    return Array.from(new Set(candidates.map(toPositiveId).filter((id): id is number => id !== null)));
}

function occurredOn(record: Record<string, unknown>, type: ActionCenterAnomalyType): string {
    if (type === 'category_spike') return toString(record['month']);
    const dates = Array.isArray(record['dates'])
        ? record['dates'].map(toString).filter(Boolean)
        : [];
    return dates.at(-1) || toString(record['date']);
}

function amountCents(record: Record<string, unknown>, type: ActionCenterAnomalyType): number {
    return toNonNegativeInteger(type === 'category_spike'
        ? record['currentAmountCents']
        : record['amountCents']);
}

function anomalyKey(
    type: ActionCenterAnomalyType,
    record: Record<string, unknown>,
    ids: number[],
    date: string
): string {
    if (type === 'category_spike') {
        return `${type}:${date}:${toString(record['category'])}`;
    }
    return `${type}:${ids.join(':') || date || 'unknown'}`;
}

function normalizeAnomaly(value: unknown): ActionCenterAnomaly | null {
    const record = toRecord(value);
    const type = anomalyType(record['type']);
    if (!type) return null;

    const ids = billIds(record);
    const date = occurredOn(record, type);
    return {
        key: anomalyKey(type, record, ids, date),
        type,
        severity: severity(record['severity']),
        billIds: ids,
        amountCents: amountCents(record, type),
        occurredOn: date,
        category: toString(record['category']),
        counterparty: toString(record['counterparty']),
        description: toString(record['description']),
        message: toString(record['message'])
    };
}

export function normalizeActionCenterAnomalyResponse(payload: unknown): ActionCenterAnomalyResponse {
    const record = toRecord(payload);
    const rawItems = Array.isArray(record['anomalies']) ? record['anomalies'] : [];
    const items = rawItems.map(normalizeAnomaly).filter((item): item is ActionCenterAnomaly => item !== null);
    return {
        items,
        totalCount: toNonNegativeInteger(record['totalCount']) || items.length,
        analyzedBills: toNonNegativeInteger(record['analyzedBills']),
        analyzedMonths: toNonNegativeInteger(record['analyzedMonths']),
        startDate: toString(record['startDate']),
        endDate: toString(record['endDate'])
    };
}

function transactionKeyword(anomaly: ActionCenterAnomaly): string {
    return anomaly.counterparty || anomaly.description || anomaly.category;
}

export function buildAnomalyActionTarget(
    anomaly: ActionCenterAnomaly,
    platform: ActionCenterPlatform
): DesktopActionTarget | string {
    const statisticsPath = platform === 'mobile' ? '/statistic/transaction' : '/statistics/transaction';
    if (anomaly.type === 'category_spike') {
        return platform === 'mobile' ? statisticsPath : { path: statisticsPath };
    }

    const keyword = transactionKeyword(anomaly);
    if (platform === 'desktop') {
        return keyword
            ? { path: '/transaction/list', query: { keyword } }
            : { path: '/transaction/list' };
    }
    return keyword
        ? `/transaction/list?keyword=${encodeURIComponent(keyword)}`
        : '/transaction/list';
}

export function summarizeActionCenter(counts: {
    anomalyCount: number;
    recurringCount: number;
}): ActionCenterSummary {
    const anomalyCount = toNonNegativeInteger(counts.anomalyCount);
    const recurringCount = toNonNegativeInteger(counts.recurringCount);
    const total = anomalyCount + recurringCount;
    return { total, anomalyCount, recurringCount, hasWork: total > 0 };
}
