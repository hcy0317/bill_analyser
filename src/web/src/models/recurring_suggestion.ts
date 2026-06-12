/**
 * Recurring Suggestion (周期账单自动发现) — TypeScript 类型定义。
 *
 * 对应后端 /api/recurring/** 的数据结构。
 */

export type RecurringSuggestionStatus = 'pending' | 'accepted' | 'rejected' | string;

export interface RecurringSuggestion {
    id: number;
    patternHash: string;
    name: string;
    description: string;
    type: string;
    amountCents: number;
    sourceAccountId: number | null;
    destinationAccountId: string | null;
    counterparty: string;
    frequency: string;
    detectedIntervalDays: number;
    confidenceScore: number;
    sampleCount: number;
    sampleBillIds: number[];
    firstOccurrence: string;
    lastOccurrence: string;
    suggestedNextDate: string;
    status: RecurringSuggestionStatus;
    createdAt: string;
    updatedAt: string;
}

export interface RecurringSuggestionsResponse {
    items: RecurringSuggestion[];
    total: number;
}

export interface RecurringDetectResponse {
    detected: number;
    created: number;
    updated: number;
    skipped: number;
}

export interface RecurringAcceptResponse {
    recurringId: number;
    suggestionId: number;
    status: string;
}

// ── Normalizers ──────────────────────────────

function toRecord(value: unknown): Record<string, unknown> {
    if (value && typeof value === 'object' && !Array.isArray(value)) {
        return value as Record<string, unknown>;
    }
    return {};
}

function toNumber(value: unknown): number {
    if (typeof value === 'number' && Number.isFinite(value)) return value;
    if (typeof value === 'string') {
        const n = Number(value);
        if (Number.isFinite(n)) return n;
    }
    return 0;
}

function toIntegerCents(value: unknown): number {
    if (typeof value === 'number') {
        return Number.isSafeInteger(value) ? value : 0;
    }
    if (typeof value === 'string') {
        const text = value.trim();
        if (!/^[+-]?\d+$/u.test(text)) {
            return 0;
        }
        const n = Number(text);
        return Number.isSafeInteger(n) ? n : 0;
    }
    return 0;
}

function toNullableNumber(value: unknown): number | null {
    if (value === null || value === undefined || value === '') return null;
    const n = toNumber(value);
    return n > 0 ? n : null;
}

function toStr(value: unknown): string {
    return typeof value === 'string' ? value : '';
}

function normalizeItem<T>(raw: unknown, mapper: (rec: Record<string, unknown>) => T): T {
    return mapper(toRecord(raw));
}

function normalizeList<T>(raw: unknown, mapper: (rec: Record<string, unknown>) => T): T[] {
    const arr = Array.isArray(raw) ? raw : [];
    return arr.map(item => normalizeItem(item, mapper));
}

function mapSuggestion(r: Record<string, unknown>): RecurringSuggestion {
    const sampleBillIds = Array.isArray(r['sampleBillIds'])
        ? (r['sampleBillIds'] as unknown[]).map(id => toNumber(id))
        : [];

    return {
        id: toNumber(r['id']),
        patternHash: toStr(r['patternHash']),
        name: toStr(r['name']),
        description: toStr(r['description']),
        type: toStr(r['type']),
        amountCents: toIntegerCents(r['amountCents'] ?? r['amount_cents']),
        sourceAccountId: toNullableNumber(r['sourceAccountId']),
        destinationAccountId: r['destinationAccountId'] != null ? String(r['destinationAccountId']) : null,
        counterparty: toStr(r['counterparty']),
        frequency: toStr(r['frequency']),
        detectedIntervalDays: toNumber(r['detectedIntervalDays']),
        confidenceScore: toNumber(r['confidenceScore']),
        sampleCount: toNumber(r['sampleCount']),
        sampleBillIds,
        firstOccurrence: toStr(r['firstOccurrence']),
        lastOccurrence: toStr(r['lastOccurrence']),
        suggestedNextDate: toStr(r['suggestedNextDate']),
        status: toStr(r['status']) || 'pending',
        createdAt: toStr(r['createdAt']),
        updatedAt: toStr(r['updatedAt']),
    };
}

export function normalizeSuggestionsResponse(payload: unknown): RecurringSuggestionsResponse {
    const data = toRecord(payload);
    return {
        items: normalizeList(data['items'], mapSuggestion),
        total: toNumber(data['total']),
    };
}

export function normalizeDetectResponse(payload: unknown): RecurringDetectResponse {
    const data = toRecord(payload);
    return {
        detected: toNumber(data['detected']),
        created: toNumber(data['created']),
        updated: toNumber(data['updated']),
        skipped: toNumber(data['skipped']),
    };
}

export function getFrequencyLabel(frequency: string): string {
    const labels: Record<string, string> = {
        weekly: '每周',
        biweekly: '每两周',
        monthly: '每月',
        bimonthly: '每两月',
        quarterly: '每季度',
        semiannual: '每半年',
        annual: '每年',
    };
    return labels[frequency] || frequency;
}

export function getConfidenceColor(score: number): string {
    if (score >= 0.8) return 'success';
    if (score >= 0.5) return 'warning';
    return 'error';
}
