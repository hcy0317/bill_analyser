import { TransactionType } from '@/core/transaction.ts';

import type { ImportMatchingPayload } from '../import_matching.ts';

/**
 * 中文说明：按优先级读取第一个非空字符串，兼容导入 matching payload 的新旧候选字段。
 */
export function getFirstNonEmptyString(...values: Array<string | null | undefined>): string {
    for (const value of values) {
        if (typeof value === 'string' && value) {
            return value;
        }
    }

    return '';
}

/**
 * 中文说明：读取第一个可用 id 并转成字符串，保留数字 id 与历史字符串 id 的兼容性。
 */
export function getFirstDefinedIdString(...values: Array<number | string | null | undefined>): string {
    for (const value of values) {
        if (typeof value === 'number') {
            return String(value);
        }

        if (typeof value === 'string' && value) {
            return value;
        }
    }

    return '';
}

/**
 * 中文说明：把 matching candidate 中的转账类型提示转换为前端 TransactionType，用于预览行建议类型回填。
 */
export function getSuggestedTypeFromMatchingCandidate(candidateType: string | undefined): number | undefined {
    const normalizedCandidateType = (candidateType || '').trim().toLowerCase();
    if (
        normalizedCandidateType === '转账'
        || normalizedCandidateType === '4'
        || normalizedCandidateType.includes('transfer')
    ) {
        return TransactionType.Transfer;
    }

    return undefined;
}

/**
 * 中文说明：归一去重来源 id，支持数组、逗号字符串、数字字符串和非数字来源 token。
 */
export function normalizeDedupSourceIds(rawValue: Array<number | string> | string | undefined): Array<number | string> {
    if (Array.isArray(rawValue)) {
        return rawValue.map(value => {
            if (typeof value === 'string') {
                const trimmedValue = value.trim();
                const parsedValue = Number(trimmedValue);
                return Number.isNaN(parsedValue) ? trimmedValue : parsedValue;
            }

            return value;
        }).filter(value => value !== '');
    }

    if (typeof rawValue === 'string' && rawValue) {
        return rawValue.split(',').map(value => value.trim()).filter(value => !!value).map(value => {
            const parsedValue = Number(value);
            return Number.isNaN(parsedValue) ? value : parsedValue;
        });
    }

    return [];
}

/**
 * 中文说明：判断去重来源 id 是否存在，统一复用 normalizeDedupSourceIds 的兼容解析。
 */
export function hasDedupSourceIds(rawValue: Array<number | string> | string | undefined): boolean {
    return normalizeDedupSourceIds(rawValue).length > 0;
}

export type SparseImportMatchingPayload = {
    transfer?: Partial<ImportMatchingPayload['transfer']>;
    investment?: Partial<ImportMatchingPayload['investment']>;
    learning?: Partial<ImportMatchingPayload['learning']>;
    recurring?: Partial<ImportMatchingPayload['recurring']>;
    dedup?: Partial<ImportMatchingPayload['dedup']>;
    parser?: Partial<ImportMatchingPayload['parser']>;
    annotation?: Partial<ImportMatchingPayload['annotation']>;
    llm?: Partial<NonNullable<ImportMatchingPayload['llm']>>;
    reconciliation?: ImportMatchingPayload['reconciliation'];
    identity_validation?: ImportMatchingPayload['identity_validation'];
    stage2_baseline?: ImportMatchingPayload['stage2_baseline'];
};

const IMPORT_PREVIEW_SIGNAL_TRIM_REGEXP = /^[\u0009-\u000D\u0020\u0085\u00A0\u1680\u2000-\u200A\u2028\u2029\u202F\u205F\u3000]+|[\u0009-\u000D\u0020\u0085\u00A0\u1680\u2000-\u200A\u2028\u2029\u202F\u205F\u3000]+$/g;
const IMPORT_PREVIEW_DECIMAL_NUMERIC_STRING_REGEXP = /^[+-]?([0-9]+([.][0-9]+)?|[.][0-9]+)$/;

export function trimImportPreviewSignalText(value: string): string {
    return value.replace(IMPORT_PREVIEW_SIGNAL_TRIM_REGEXP, '');
}

export function isStrictImportPreviewDecimalString(value: string): boolean {
    return IMPORT_PREVIEW_DECIMAL_NUMERIC_STRING_REGEXP.test(trimImportPreviewSignalText(value));
}

function strictImportPreviewDecimalHasNonZeroDigit(value: string): boolean {
    return /[1-9]/.test(value);
}

export function isNonZeroImportPreviewDecimalString(value: string): boolean {
    const text = trimImportPreviewSignalText(value);
    return isStrictImportPreviewDecimalString(text)
        && strictImportPreviewDecimalHasNonZeroDigit(text);
}

export function isPositiveImportPreviewDecimalString(value: string): boolean {
    const text = trimImportPreviewSignalText(value);
    return isStrictImportPreviewDecimalString(text)
        && !text.startsWith('-')
        && strictImportPreviewDecimalHasNonZeroDigit(text);
}

export function isCanonicalTruthy(value: unknown): boolean {
    if (typeof value === 'boolean') {
        return value;
    }

    if (typeof value === 'number') {
        return Number.isFinite(value) && value !== 0;
    }

    if (typeof value === 'string') {
        const text = trimImportPreviewSignalText(value).toLowerCase();
        if (!text || text === '0' || text === 'false' || text === 'none' || text === 'suppressed' || text === 'null')
        {
            return false;
        }
        if (text === 'no' || text === 'n') {
            return false;
        }
        if (text === 'true' || text === '1' || text === 'yes' || text === 'y') {
            return true;
        }
        return isNonZeroImportPreviewDecimalString(text);
    }

    return false;
}

/**
 * 中文说明：把稀疏 matching payload 补齐为前端稳定结构，保证导入预览组件不直接处理缺失 section。
 */
export function normalizeImportMatchingPayload(matching?: SparseImportMatchingPayload): ImportMatchingPayload | undefined {
    if (!matching) {
        return undefined;
    }

    const parser = matching.parser;
    const parserTags = Array.isArray(parser?.tags) ? parser.tags : [];
    const dedupSourceIds = normalizeDedupSourceIds(matching.dedup?.source_ids);
    const normalizedTransferSuppressed = isCanonicalTruthy(matching.transfer?.suppressed);
    const normalizedInvestmentSuppressed = isCanonicalTruthy(matching.investment?.suppressed);
    const normalizedLearningSuppressed = isCanonicalTruthy(matching.learning?.suppressed);
    const normalizedLearningAutoApply = isCanonicalTruthy(matching.learning?.auto_apply);
    const normalizedLlmSuppressed = isCanonicalTruthy(matching.llm?.suppressed);

    return {
        transfer: {
            candidate_type: '',
            score: 0,
            level: '',
            reason: '',
            review_status: '',
            reviewed_type: '',
            pair_order: '',
            source_chain: [],
            ...matching.transfer,
            suppressed: normalizedTransferSuppressed,
        },
        investment: {
            score: 0,
            level: '',
            reason: '',
            platform: '',
            product: '',
            review_status: '',
            ...matching.investment,
            suppressed: normalizedInvestmentSuppressed,
        },
        learning: {
            rule_id: null,
            score: 0,
            level: '',
            reason: '',
            recommended_type: '',
            summary: '',
            review_status: '',
            status: '',
            source: '',
            mode: '',
            model_version: '',
            recommendation_key: '',
            lifecycle_status: '',
            signal_state: '',
            accepted_count: 0,
            rejected_count: 0,
            auto_applied_count: 0,
            ...matching.learning,
            suppressed: normalizedLearningSuppressed,
            auto_apply: normalizedLearningAutoApply,
        },
        recurring: {
            id: null,
            name: '',
            candidate_count: 0,
            match_score: 0,
            match_reasons: '',
            matched_date: '',
            ...matching.recurring,
        },
        dedup: {
            type: matching.dedup?.type || '',
            source_ids: dedupSourceIds,
            source_count: matching.dedup?.source_count ?? dedupSourceIds.length,
            source_labels: matching.dedup?.source_labels || [],
            sources: matching.dedup?.sources || [],
        },
        parser: {
            id: parser?.id || '',
            tags: parserTags,
            source_chain: parser?.source_chain || [],
        },
        annotation: {
            is_manually_annotated: isCanonicalTruthy(matching.annotation?.is_manually_annotated),
        },
        llm: matching.llm ? {
            suggested_type: matching.llm.suggested_type || '',
            suggested_category_id: matching.llm.suggested_category_id,
            suggested_main_category: matching.llm.suggested_main_category || '',
            suggested_sub_category: matching.llm.suggested_sub_category || '',
            suggested_source_account: matching.llm.suggested_source_account || '',
            suggested_destination_account: matching.llm.suggested_destination_account || '',
            confidence: Number(matching.llm.confidence || 0),
            reason: matching.llm.reason || '',
            review_status: matching.llm.review_status || '',
            status: matching.llm.status || '',
            lifecycle_status: matching.llm.lifecycle_status || '',
            signal_state: matching.llm.signal_state || '',
            suppressed: normalizedLlmSuppressed,
        } : undefined,
        reconciliation: matching.reconciliation || {},
        identity_validation: matching.identity_validation,
        stage2_baseline: matching.stage2_baseline,
    };
}
