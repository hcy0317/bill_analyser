import type {
    ReceiptDraftAutoFill,
    ReceiptDraftCandidates,
    ReceiptDraftField,
    ReceiptTransactionDraft,
    ReceiptImageErrorCode,
    RecognizeReceiptImageError
} from '@/models/large_language_model.ts';

const KNOWN_RECEIPT_IMAGE_ERROR_CODES: ReadonlySet<ReceiptImageErrorCode> = new Set<ReceiptImageErrorCode>([
    'provider_unconfigured',
    'timeout',
    'parse_error',
    'cancelled',
    'rate_limited',
    'unknown'
]);

/** 将后端 OCR 错误码和 HTTP 状态归一为前端展示使用的稳定错误枚举。 */
export function mapReceiptImageErrorCode(rawCode: string | undefined, status: number): ReceiptImageErrorCode {
    if (rawCode && KNOWN_RECEIPT_IMAGE_ERROR_CODES.has(rawCode as ReceiptImageErrorCode) && rawCode !== 'unknown') {
        return rawCode as ReceiptImageErrorCode;
    }

    if (status === 501) {
        return 'provider_unconfigured';
    }

    if (status === 504) {
        return 'timeout';
    }

    if (status === 422) {
        return 'parse_error';
    }

    if (status === 499) {
        return 'cancelled';
    }

    if (status === 429) {
        return 'rate_limited';
    }

    return 'unknown';
}

/** 构造图片 OCR 识别错误对象，保留原始异常供调用方记录日志。 */
export function buildRecognizeReceiptImageError(errorCode: ReceiptImageErrorCode, message: string, status: number, originalError?: unknown): RecognizeReceiptImageError {
    return {
        errorCode,
        message,
        status,
        originalError
    };
}

type ReceiptDraftFieldKind = 'string' | 'number' | 'stringArray' | 'transactionType';

/** 判断未知值是否是可读取字段的普通对象。 */
function isRecord(value: unknown): value is Record<string, unknown> {
    return !!value && typeof value === 'object' && !Array.isArray(value);
}

/** 把 OCR draft 中可展示的字符串或数字字段归一为非空字符串。 */
function normalizeReceiptDraftString(value: unknown): string | undefined {
    if (typeof value === 'string' && value) {
        return value;
    }

    if (typeof value === 'number' && Number.isFinite(value)) {
        return String(value);
    }

    return undefined;
}

/** 归一 OCR draft 候选标签数组，并去除空值和重复项。 */
function normalizeReceiptDraftStringArray(value: unknown): string[] | undefined {
    if (!Array.isArray(value)) {
        return undefined;
    }

    const normalized = value
        .map(item => normalizeReceiptDraftString(item))
        .filter((item): item is string => !!item);
    return normalized.length ? Array.from(new Set(normalized)) : undefined;
}

/** 按字段类型归一 OCR draft 字段值，避免把坏格式写入交易草稿。 */
function normalizeReceiptDraftFieldValue(value: unknown, kind: ReceiptDraftFieldKind): string | number | string[] | undefined {
    if (kind === 'number') {
        return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
    }

    if (kind === 'stringArray') {
        return normalizeReceiptDraftStringArray(value);
    }

    if (kind === 'transactionType') {
        if (typeof value === 'number' && Number.isFinite(value)) {
            return value;
        }

        return normalizeReceiptDraftString(value);
    }

    return normalizeReceiptDraftString(value);
}

/** 归一单个 OCR draft 字段，并保留置信度、原因、证据和展示单位。 */
function normalizeReceiptDraftField(raw: unknown, kind: ReceiptDraftFieldKind): ReceiptDraftField | undefined {
    if (!isRecord(raw)) {
        return undefined;
    }

    const value = normalizeReceiptDraftFieldValue(raw['value'], kind);
    if (value === undefined) {
        return undefined;
    }

    const rawConfidence = raw['confidence'];
    const rawReason = raw['reason'];
    const rawEvidence = raw['evidence'];
    const rawLabel = raw['label'];
    const rawUnit = raw['unit'];
    const field: ReceiptDraftField = {
        value,
        confidence: typeof rawConfidence === 'number' && Number.isFinite(rawConfidence) ? rawConfidence : 0,
        reason: typeof rawReason === 'string' ? rawReason : '',
        evidence: Array.isArray(rawEvidence) ? rawEvidence.filter((item): item is string => typeof item === 'string') : []
    };

    if (typeof rawLabel === 'string' && rawLabel) {
        return {
            ...field,
            label: rawLabel,
            unit: typeof rawUnit === 'string' && rawUnit ? rawUnit : undefined
        };
    }

    if (typeof rawUnit === 'string' && rawUnit) {
        return {
            ...field,
            unit: rawUnit
        };
    }

    return field;
}

/** 归一同一字段的多个 OCR 候选值。 */
function normalizeReceiptDraftFieldList(raw: unknown, kind: ReceiptDraftFieldKind): ReceiptDraftField[] | undefined {
    if (!Array.isArray(raw)) {
        return undefined;
    }

    const fields = raw
        .map(item => normalizeReceiptDraftField(item, kind))
        .filter((item): item is ReceiptDraftField => !!item);
    return fields.length ? fields : undefined;
}

/** 判断自动填充区是否至少包含一个可应用字段。 */
function hasReceiptDraftAutoFill(autoFill: ReceiptDraftAutoFill): boolean {
    return !!(
        autoFill.type
        || autoFill.amount
        || autoFill.time
        || autoFill.description
        || autoFill.categoryId
        || autoFill.sourceAccountId
        || autoFill.destinationAccountId
        || autoFill.tagIds
    );
}

/** 判断候选区是否至少包含一个可展示字段。 */
function hasReceiptDraftCandidates(candidates: ReceiptDraftCandidates): boolean {
    return !!(
        candidates.type?.length
        || candidates.amount?.length
        || candidates.time?.length
        || candidates.description?.length
        || candidates.categoryId?.length
        || candidates.sourceAccountId?.length
        || candidates.destinationAccountId?.length
        || candidates.tagIds?.length
    );
}

/** 将后端图片 OCR 原始 draft 归一为交易编辑页可消费的草稿结构。 */
export function normalizeReceiptTransactionDraft(raw: unknown): ReceiptTransactionDraft | undefined {
    if (!isRecord(raw)) {
        return undefined;
    }

    const rawAutoFill = isRecord(raw['auto_fill']) ? raw['auto_fill'] : (isRecord(raw['autoFill']) ? raw['autoFill'] : {});
    const rawCandidates = isRecord(raw['candidates']) ? raw['candidates'] : {};
    const autoFill: ReceiptDraftAutoFill = {
        type: normalizeReceiptDraftField(rawAutoFill['type'], 'transactionType') as ReceiptDraftField<string | number> | undefined,
        amount: normalizeReceiptDraftField(rawAutoFill['amount'], 'number') as ReceiptDraftField<number> | undefined,
        time: normalizeReceiptDraftField(rawAutoFill['time'], 'string') as ReceiptDraftField<string> | undefined,
        description: normalizeReceiptDraftField(rawAutoFill['description'], 'string') as ReceiptDraftField<string> | undefined,
        categoryId: normalizeReceiptDraftField(rawAutoFill['category_id'] ?? rawAutoFill['categoryId'], 'string') as ReceiptDraftField<string> | undefined,
        sourceAccountId: normalizeReceiptDraftField(rawAutoFill['source_account_id'] ?? rawAutoFill['sourceAccountId'], 'string') as ReceiptDraftField<string> | undefined,
        destinationAccountId: normalizeReceiptDraftField(rawAutoFill['destination_account_id'] ?? rawAutoFill['destinationAccountId'], 'string') as ReceiptDraftField<string> | undefined,
        tagIds: normalizeReceiptDraftField(rawAutoFill['tag_ids'] ?? rawAutoFill['tagIds'], 'stringArray') as ReceiptDraftField<string[]> | undefined
    };
    const candidates: ReceiptDraftCandidates = {
        type: normalizeReceiptDraftFieldList(rawCandidates['type'], 'transactionType') as ReceiptDraftField<string | number>[] | undefined,
        amount: normalizeReceiptDraftFieldList(rawCandidates['amount'], 'number') as ReceiptDraftField<number>[] | undefined,
        time: normalizeReceiptDraftFieldList(rawCandidates['time'], 'string') as ReceiptDraftField<string>[] | undefined,
        description: normalizeReceiptDraftFieldList(rawCandidates['description'], 'string') as ReceiptDraftField<string>[] | undefined,
        categoryId: normalizeReceiptDraftFieldList(rawCandidates['category_id'] ?? rawCandidates['categoryId'], 'string') as ReceiptDraftField<string>[] | undefined,
        sourceAccountId: normalizeReceiptDraftFieldList(rawCandidates['source_account_id'] ?? rawCandidates['sourceAccountId'], 'string') as ReceiptDraftField<string>[] | undefined,
        destinationAccountId: normalizeReceiptDraftFieldList(rawCandidates['destination_account_id'] ?? rawCandidates['destinationAccountId'], 'string') as ReceiptDraftField<string>[] | undefined,
        tagIds: normalizeReceiptDraftFieldList(rawCandidates['tag_ids'] ?? rawCandidates['tagIds'], 'stringArray') as ReceiptDraftField<string[]>[] | undefined
    };

    if (!hasReceiptDraftAutoFill(autoFill) && !hasReceiptDraftCandidates(candidates)) {
        return undefined;
    }

    return { autoFill, candidates };
}
