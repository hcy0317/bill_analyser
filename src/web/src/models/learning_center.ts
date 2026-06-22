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

export interface LearningFeatureChip {
    key: string;
    labelKey: string;
    value: string;
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

/**
 * 归一化学习建议分页响应，防御缺失字段和非数组 items。
 */
export function normalizeSuggestionsResponse(payload: unknown): LearningSuggestionsResponse {
    const data = toRecord(payload);
    return {
        items: normalizeList(data['items'], mapSuggestion),
        total: toNumber(data['total']),
        limit: toNumber(data['limit']),
        offset: toNumber(data['offset']),
    };
}

/**
 * 归一化学习规则分页响应，保持列表页依赖的 total/limit/offset 形状稳定。
 */
export function normalizeRulesResponse(payload: unknown): LearningRulesResponse {
    const data = toRecord(payload);
    return {
        items: normalizeList(data['items'], mapRule),
        total: toNumber(data['total']),
        limit: toNumber(data['limit']),
        offset: toNumber(data['offset']),
    };
}

/**
 * 归一化批量接受响应，拆分成功与失败项并补齐计数字段。
 */
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

/**
 * 归一化学习建议生成响应，兼容后端 snake_case skipped_existing 字段。
 */
export function normalizeGenerateResponse(payload: unknown): GenerateSuggestionsResponse {
    const data = toRecord(payload);
    return {
        mined: toNumber(data['mined']),
        created: toNumber(data['created']),
        updated: toNumber(data['updated']),
        skippedExisting: toNumber(data['skipped_existing']),
    };
}

const learningFeatureAliases: Record<string, string> = {
    c: 'counterparty',
    counterparty: 'counterparty',
    d: 'description',
    description: 'description',
    p: 'parser_id',
    parser: 'parser_id',
    parser_id: 'parser_id',
    m: 'payment_method',
    payment: 'payment_method',
    payment_method: 'payment_method',
};

const learningFeatureLabels: Record<string, string> = {
    counterparty: 'Counterparty',
    description: 'Description',
    parser_id: 'Parser',
    payment_method: 'Payment Method',
};

const learningFeatureDisplayOrder = [
    'counterparty',
    'description',
    'parser_id',
    'payment_method',
];

function normalizeLearningFeatureKey(key: string): string {
    return learningFeatureAliases[key.trim()] || key.trim();
}

function parseDelimitedFeatureString(source: string): Record<string, string> {
    const features: Record<string, string> = {};
    const keyMatches = Array.from(source.matchAll(/(?:^|\|)\s*([A-Za-z_]+)\s*=/g));

    for (let index = 0; index < keyMatches.length; index += 1) {
        const match = keyMatches[index]!;
        const rawKey = match[1] || '';
        const valueStart = (match.index ?? 0) + match[0].length;
        const nextMatch = keyMatches[index + 1];
        const valueEnd = nextMatch?.index ?? source.length;
        const value = source.slice(valueStart, valueEnd).replace(/\|\s*$/, '').trim();

        if (value) {
            features[normalizeLearningFeatureKey(rawKey)] = value;
        }
    }

    return features;
}

function parseLearningFeatures(source: string): Record<string, string> {
    const trimmed = source.trim();
    if (!trimmed) {
        return {};
    }

    try {
        const rawFeatures: Record<string, unknown> = JSON.parse(trimmed);
        return Object.entries(rawFeatures).reduce<Record<string, string>>((features, [key, value]) => {
            if (typeof value === 'string' && value.trim()) {
                features[normalizeLearningFeatureKey(key)] = value.trim();
            }
            return features;
        }, {});
    } catch {
        return parseDelimitedFeatureString(trimmed);
    }
}

function getLearningFeatureChips(featuresJson: string): LearningFeatureChip[] {
    const features = parseLearningFeatures(featuresJson || '');

    return learningFeatureDisplayOrder
        .filter(key => typeof features[key] === 'string' && features[key]!.trim().length > 0)
        .map(key => ({
            key,
            labelKey: learningFeatureLabels[key] || key,
            value: features[key]!.trim(),
        }));
}

/**
 * 解析学习建议特征为 chip 列表，统一字段别名和展示顺序。
 */
export function getSuggestionFeatureChips(suggestion: LearningSuggestion): LearningFeatureChip[] {
    return getLearningFeatureChips(suggestion.matchFeaturesJson);
}

/**
 * 解析学习规则特征为 chip 列表，统一字段别名和展示顺序。
 */
export function getRuleFeatureChips(rule: LearningRule): LearningFeatureChip[] {
    return getLearningFeatureChips(rule.matchFeaturesJson);
}

/**
 * 将学习建议 match_features_json 转为可读摘要字符串。
 */
export function getSuggestionFeatureSummary(suggestion: LearningSuggestion): string {
    return getSuggestionFeatureChips(suggestion)
        .map(chip => `${chip.labelKey}: ${chip.value}`)
        .join(' · ');
}

/**
 * 将学习规则 match_features_json 转为可读摘要字符串。
 */
export function getRuleFeatureSummary(rule: LearningRule): string {
    return getRuleFeatureChips(rule)
        .map(chip => `${chip.labelKey}: ${chip.value}`)
        .join(' · ');
}
