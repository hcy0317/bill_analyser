export interface ReceiptImageProvenance {
    readonly provider: string;
    readonly model?: string;
    readonly requestId: string;
}

export type ReceiptDraftScalarValue = string | number;
export type ReceiptDraftValue = ReceiptDraftScalarValue | string[];

export interface ReceiptDraftField<T = ReceiptDraftValue> {
    readonly value: T;
    readonly confidence: number;
    readonly reason: string;
    readonly evidence: string[];
    readonly label?: string;
    readonly unit?: string;
}

export interface ReceiptDraftAutoFill {
    readonly type?: ReceiptDraftField<string | number>;
    readonly amount?: ReceiptDraftField<number>;
    readonly time?: ReceiptDraftField<string>;
    readonly description?: ReceiptDraftField<string>;
    readonly categoryId?: ReceiptDraftField<string>;
    readonly sourceAccountId?: ReceiptDraftField<string>;
    readonly destinationAccountId?: ReceiptDraftField<string>;
    readonly tagIds?: ReceiptDraftField<string[]>;
}

export interface ReceiptDraftCandidates {
    readonly type?: ReceiptDraftField<string | number>[];
    readonly amount?: ReceiptDraftField<number>[];
    readonly time?: ReceiptDraftField<string>[];
    readonly description?: ReceiptDraftField<string>[];
    readonly categoryId?: ReceiptDraftField<string>[];
    readonly sourceAccountId?: ReceiptDraftField<string>[];
    readonly destinationAccountId?: ReceiptDraftField<string>[];
    readonly tagIds?: ReceiptDraftField<string[]>[];
}

export interface ReceiptTransactionDraft {
    readonly autoFill: ReceiptDraftAutoFill;
    readonly candidates: ReceiptDraftCandidates;
}

export interface RecognizedReceiptImageResponse {
    readonly amount: number | null;        // yuan (per Bill Analyser OCR contract); store layer keeps as-is, ListPage caller converts to cents.
    readonly tradeTime: string | null;     // ISO 8601 string, may be null when provider cannot determine.
    readonly description: string | null;
    readonly paymentPlatform: string | null;
    readonly provenance: ReceiptImageProvenance;
    readonly confidence: number | null;    // 0..1
    readonly draft?: ReceiptTransactionDraft;
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
