/**
 * Learning Center (学习中心) — TypeScript 类型定义。
 *
 * 对应后端 /api/learning/** 的数据结构。
 */

// ── Suggestion (学习建议) ─────────────────────

export type LearningSuggestionStatus = 'pending' | 'accepted' | 'rejected' | string;

export interface LearningSuggestion {
    id: number;
    matchType: string;
    matchValue: string;
    matchFeaturesJson: string;
    suggestedType: string;
    suggestedCategoryId: number | null;
    suggestedSourceAccountId: number | null;
    suggestedDestinationAccountId: number | null;
    sampleCount: number;
    status: LearningSuggestionStatus;
    summary: string;
    createdAt: string;
    updatedAt: string;
}

export interface LearningSuggestionsResponse {
    items: LearningSuggestion[];
    total: number;
    limit: number;
    offset: number;
}

// ── Batch-accept ─────────────────────────────

export interface BatchAcceptResultItem {
    id: number;
    ruleId?: number;
    error?: string;
}

export interface BatchAcceptResponse {
    accepted: BatchAcceptResultItem[];
    failed: BatchAcceptResultItem[];
    acceptedCount: number;
    failedCount: number;
}

// ── Rule (学习规则) ──────────────────────────

export interface LearningRule {
    id: number;
    matchType: string;
    matchValue: string;
    learnedType: string;
    learnedCategoryId: number | null;
    learnedSourceAccountId: number | null;
    learnedDestinationAccountId: number | null;
    enabled: boolean;
    appliedCount: number;
    lastAppliedAt: string;
    matchFeaturesJson: string;
    createdAt: string;
    updatedAt: string;
}

export interface LearningRulesResponse {
    items: LearningRule[];
    total: number;
    limit: number;
    offset: number;
}

// ── Generate (挖掘) ──────────────────────────

export interface GenerateSuggestionsResponse {
    mined: number;
    created: number;
    updated: number;
    skippedExisting: number;
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

function toNullableNumber(value: unknown): number | null {
    if (value === null || value === undefined || value === '') return null;
    const n = toNumber(value);
    return n > 0 ? n : null;
}

function toStr(value: unknown): string {
    return typeof value === 'string' ? value : '';
}

function toBool(value: unknown): boolean {
    return value === true || value === 1 || value === '1';
}

function normalizeItem<T>(raw: unknown, mapper: (rec: Record<string, unknown>) => T): T {
    return mapper(toRecord(raw));
}

function normalizeList<T>(raw: unknown, mapper: (rec: Record<string, unknown>) => T): T[] {
    const arr = Array.isArray(raw) ? raw : [];
    return arr.map(item => normalizeItem(item, mapper));
}

function mapSuggestion(r: Record<string, unknown>): LearningSuggestion {
    return {
        id: toNumber(r['id']),
        matchType: toStr(r['match_type']),
        matchValue: toStr(r['match_value']),
        matchFeaturesJson: toStr(r['match_features_json']),
        suggestedType: toStr(r['suggested_type']),
        suggestedCategoryId: toNullableNumber(r['suggested_category_id']),
        suggestedSourceAccountId: toNullableNumber(r['suggested_source_account_id']),
        suggestedDestinationAccountId: toNullableNumber(r['suggested_destination_account_id']),
        sampleCount: toNumber(r['sample_count']),
        status: toStr(r['status']) || 'pending',
        summary: toStr(r['summary']),
        createdAt: toStr(r['created_at']),
        updatedAt: toStr(r['updated_at']),
    };
}

function mapRule(r: Record<string, unknown>): LearningRule {
    return {
        id: toNumber(r['id']),
        matchType: toStr(r['match_type']),
        matchValue: toStr(r['match_value']),
        learnedType: toStr(r['learned_type']),
        learnedCategoryId: toNullableNumber(r['learned_category_id']),
        learnedSourceAccountId: toNullableNumber(r['learned_source_account_id']),
        learnedDestinationAccountId: toNullableNumber(r['learned_destination_account_id']),
        enabled: toBool(r['enabled']),
        appliedCount: toNumber(r['applied_count']),
        lastAppliedAt: toStr(r['last_applied_at']),
        matchFeaturesJson: toStr(r['match_features_json']),
        createdAt: toStr(r['created_at']),
        updatedAt: toStr(r['updated_at']),
    };
}

export function normalizeSuggestionsResponse(payload: unknown): LearningSuggestionsResponse {
    const data = toRecord(payload);
    return {
        items: normalizeList(data['items'], mapSuggestion),
        total: toNumber(data['total']),
        limit: toNumber(data['limit']),
        offset: toNumber(data['offset']),
    };
}

export function normalizeRulesResponse(payload: unknown): LearningRulesResponse {
    const data = toRecord(payload);
    return {
        items: normalizeList(data['items'], mapRule),
        total: toNumber(data['total']),
        limit: toNumber(data['limit']),
        offset: toNumber(data['offset']),
    };
}

export function normalizeBatchAcceptResponse(payload: unknown): BatchAcceptResponse {
    const data = toRecord(payload);
    return {
        accepted: normalizeList(data['accepted'], r => ({
            id: toNumber(r['id']),
            ruleId: toNullableNumber(r['ruleId']) ?? undefined,
        })),
        failed: normalizeList(data['failed'], r => ({
            id: toNumber(r['id']),
            error: toStr(r['error']) || undefined,
        })),
        acceptedCount: toNumber(data['acceptedCount']),
        failedCount: toNumber(data['failedCount']),
    };
}

export function normalizeGenerateResponse(payload: unknown): GenerateSuggestionsResponse {
    const data = toRecord(payload);
    return {
        mined: toNumber(data['mined']),
        created: toNumber(data['created']),
        updated: toNumber(data['updated']),
        skippedExisting: toNumber(data['skipped_existing']),
    };
}

/**
 * Parse match_features_json into a readable summary string.
 */
export function getSuggestionFeatureSummary(suggestion: LearningSuggestion): string {
    const labels: Record<string, string> = {
        parser_id: 'parser',
        counterparty: 'counterparty',
        description: 'description',
        payment_method: 'payment',
    };
    try {
        const features: Record<string, string> = JSON.parse(suggestion.matchFeaturesJson || '{}');
        return Object.entries(features)
            .filter(([, v]) => typeof v === 'string' && v.trim().length > 0)
            .map(([k, v]) => `${labels[k] || k}: ${v}`)
            .join(' · ');
    } catch {
        return '';
    }
}

/**
 * Parse match_features_json for a rule.
 */
export function getRuleFeatureSummary(rule: LearningRule): string {
    const labels: Record<string, string> = {
        parser_id: 'parser',
        counterparty: 'counterparty',
        description: 'description',
        payment_method: 'payment',
    };
    try {
        const features: Record<string, string> = JSON.parse(rule.matchFeaturesJson || '{}');
        return Object.entries(features)
            .filter(([, v]) => typeof v === 'string' && v.trim().length > 0)
            .map(([k, v]) => `${labels[k] || k}: ${v}`)
            .join(' · ');
    } catch {
        return '';
    }
}
