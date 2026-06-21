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

export function buildRecognizeReceiptImageError(errorCode: ReceiptImageErrorCode, message: string, status: number, originalError?: unknown): RecognizeReceiptImageError {
    return {
        errorCode,
        message,
        status,
        originalError
    };
}

type ReceiptDraftFieldKind = 'string' | 'number' | 'stringArray' | 'transactionType';

function isRecord(value: unknown): value is Record<string, unknown> {
    return !!value && typeof value === 'object' && !Array.isArray(value);
}

function normalizeReceiptDraftString(value: unknown): string | undefined {
    if (typeof value === 'string' && value) {
        return value;
    }

    if (typeof value === 'number' && Number.isFinite(value)) {
        return String(value);
    }

    return undefined;
}

function normalizeReceiptDraftStringArray(value: unknown): string[] | undefined {
    if (!Array.isArray(value)) {
        return undefined;
    }

    const normalized = value
        .map(item => normalizeReceiptDraftString(item))
        .filter((item): item is string => !!item);
    return normalized.length ? Array.from(new Set(normalized)) : undefined;
}

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

function normalizeReceiptDraftFieldList(raw: unknown, kind: ReceiptDraftFieldKind): ReceiptDraftField[] | undefined {
    if (!Array.isArray(raw)) {
        return undefined;
    }

    const fields = raw
        .map(item => normalizeReceiptDraftField(item, kind))
        .filter((item): item is ReceiptDraftField => !!item);
    return fields.length ? fields : undefined;
}

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
