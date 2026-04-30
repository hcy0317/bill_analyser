export interface ReceiptImageProvenance {
    readonly provider: string;
    readonly model?: string;
    readonly requestId: string;
}

export interface RecognizedReceiptImageResponse {
    readonly amount: number | null;        // yuan (per Bill Analyser OCR contract); store layer keeps as-is, ListPage caller converts to cents.
    readonly tradeTime: string | null;     // ISO 8601 string, may be null when provider cannot determine.
    readonly description: string | null;
    readonly provenance: ReceiptImageProvenance;
    readonly confidence: number | null;    // 0..1

    // Deprecated legacy ezbookkeeping fields. Kept optional purely so that the C3 mobile slice
    // can still type-check against this contract while it migrates. Always undefined at runtime
    // under the new C1/C2 backend contract; mobile consumers will be rewritten in slice C3.
    readonly type?: number;
    readonly time?: number;
    readonly categoryId?: string;
    readonly sourceAccountId?: string;
    readonly destinationAccountId?: string;
    readonly sourceAmount?: number;
    readonly destinationAmount?: number;
    readonly tagIds?: string[];
    readonly comment?: string;
}

export type ReceiptImageErrorCode =
    | 'provider_unconfigured'
    | 'timeout'
    | 'parse_error'
    | 'cancelled'
    | 'rate_limited'
    | 'unknown';

export interface RecognizeReceiptImageError {
    readonly errorCode: ReceiptImageErrorCode;
    readonly message: string;
    readonly status: number;
    readonly originalError?: unknown;
}

export const RECEIPT_IMAGE_LOW_CONFIDENCE_THRESHOLD = 0.6;
