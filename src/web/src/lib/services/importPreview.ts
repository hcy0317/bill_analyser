import axios from 'axios';

import type { ApiResponse } from '@/core/api.ts';
import { TransactionType } from '@/core/transaction.ts';
import { DEFAULT_UPLOAD_API_TIMEOUT } from '@/consts/api.ts';
import type { ImportTransactionResponsePageWrapper } from '@/models/imported_transaction.ts';
import type {
    ImportLearningPromoteResponse,
    ImportLearningSuggestionsResponse
} from '@/models/import_learning.ts';
import type {
    BillMatchingCandidatesResponse,
    BillMatchingFeedbackResponse,
    BillMatchingPairSummary,
    MatchingPairsResponse,
    ReconcileHistoryResponse
} from '@/models/bill_matching.ts';

import {
    buildApiResponse,
    type ApiDataResponse,
    type ApiRequestConfig,
    type ApiResponsePromise
} from './http.ts';

type MatchingCandidateActionName = 'accept' | 'reject' | 'clear';

interface MatchingCandidateActionResponse {
    candidateId: string;
    action: string;
    previewId?: number;
    sessionId?: string;
    recurringId?: number;
    keptBillId?: number;
    mergedBillId?: number;
    reviewStatus?: string;
    effect?: string;
    suppressed?: boolean;
    previewItem?: Record<string, unknown>;
    preview?: Array<Record<string, unknown>>;
    pair?: Record<string, unknown>;
    bill?: Record<string, unknown>;
}

interface MatchingSessionCandidatesResponse {
    session_id?: string;
    summary?: Record<string, unknown>;
    candidates?: Array<Record<string, unknown>>;
}

interface MatchingPairOperationResponse {
    pair?: BillMatchingPairSummary;
}

interface UpdateImportPreviewItemPayload {
    id: number;
    type?: string;
    amountCents?: number;
    destinationAmountCents?: number;
    mainCategory?: string;
    subCategory?: string;
    sourceAccountId?: number | null;
    destinationAccountId?: number | null;
    counterparty?: string;
    paymentMethod?: string;
    description?: string;
    isSelected?: boolean;
    responseMode?: string;
}

interface UpdateImportPreviewItemResponse {
    updated: boolean;
    previewItem?: Record<string, unknown>;
}

function postMatchingCandidateAction(
    action: MatchingCandidateActionName,
    candidateId: string,
    payload?: Record<string, unknown>
): ApiResponsePromise<MatchingCandidateActionResponse> {
    return axios.post<ApiDataResponse<MatchingCandidateActionResponse>>(
        `matching/candidates/${encodeURIComponent(candidateId)}/${action}`,
        payload ?? {}
    ).then(response => {
        return buildApiResponse(response, response.data?.data);
    });
}

const importPreviewServices = {
    parseImportTransaction: ({ fileType, fileEncoding, importFile, columnMapping, transactionTypeMapping, hasHeaderLine, timeFormat, timezoneFormat, amountDecimalSeparator, amountDigitGroupingSymbol, geoSeparator, geoOrder, tagSeparator, delimiter }: { fileType: string, fileEncoding?: string, importFile: File, columnMapping?: Record<number, number>, transactionTypeMapping?: Record<string, TransactionType>, hasHeaderLine?: boolean, timeFormat?: string, timezoneFormat?: string, amountDecimalSeparator?: string, amountDigitGroupingSymbol?: string, geoSeparator?: string, geoOrder?: string, tagSeparator?: string, delimiter?: string }): ApiResponsePromise<ImportTransactionResponsePageWrapper> => {
        let textualColumnMapping: string | undefined = undefined;
        let textualTransactionTypeMapping: string | undefined = undefined;
        let textualHasHeaderLine: string | undefined = undefined;

        if (columnMapping) {
            textualColumnMapping = JSON.stringify(columnMapping);
        }

        if (transactionTypeMapping) {
            textualTransactionTypeMapping = JSON.stringify(transactionTypeMapping);
        }

        if (hasHeaderLine !== undefined) {
            textualHasHeaderLine = hasHeaderLine ? 'true' : 'false';
        }

        return axios.postForm<ApiResponse<ImportTransactionResponsePageWrapper>>('bills/parse_import', {
            fileType: fileType,
            fileEncoding: fileEncoding,
            file: importFile,
            columnMapping: textualColumnMapping,
            transactionTypeMapping: textualTransactionTypeMapping,
            hasHeaderLine: textualHasHeaderLine,
            timeFormat: timeFormat,
            timezoneFormat: timezoneFormat,
            amountDecimalSeparator: amountDecimalSeparator,
            amountDigitGroupingSymbol: amountDigitGroupingSymbol,
            geoSeparator: geoSeparator,
            geoOrder: geoOrder,
            tagSeparator: tagSeparator,
            delimiter: delimiter
        }, {
            timeout: DEFAULT_UPLOAD_API_TIMEOUT
        } as ApiRequestConfig);
    },
    getImportLearningSuggestions: ({
        sessionId,
        previewUpdates,
        previewIds
    }: {
        sessionId: string,
        previewUpdates?: Array<Record<string, unknown>>,
        previewIds?: number[]
    }): ApiResponsePromise<ImportLearningSuggestionsResponse> => {
        const payload: Record<string, unknown> = {};

        if (previewUpdates !== undefined) {
            payload['preview_updates'] = previewUpdates;
        }
        if (previewIds !== undefined) {
            payload['previewIds'] = previewIds;
        }

        const request = Object.keys(payload).length > 0
            ? axios.post<ApiDataResponse<ImportLearningSuggestionsResponse>>(`bills/import/v2/learning/${sessionId}/suggestions`, payload)
            : axios.get<ApiDataResponse<ImportLearningSuggestionsResponse>>(`bills/import/v2/learning/${sessionId}/suggestions`);

        return request.then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    promoteImportLearning: ({
        sessionId,
        previewUpdates,
        previewIds
    }: {
        sessionId: string,
        previewUpdates?: Array<Record<string, unknown>>,
        previewIds?: number[]
    }): ApiResponsePromise<ImportLearningPromoteResponse> => {
        const payload: Record<string, unknown> = {};

        if (previewUpdates !== undefined) {
            payload['preview_updates'] = previewUpdates;
        }
        if (previewIds !== undefined) {
            payload['previewIds'] = previewIds;
        }

        return axios.post<ApiDataResponse<ImportLearningPromoteResponse>>(`bills/import/v2/learning/${sessionId}/promote`, payload).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    updateImportPreviewItem: ({
        sessionId,
        payload
    }: {
        sessionId: string,
        payload: UpdateImportPreviewItemPayload
    }): ApiResponsePromise<UpdateImportPreviewItemResponse> => {
        return axios.put<{ success?: boolean, data?: { updated?: boolean, previewItem?: Record<string, unknown> } }>(`bills/import/v2/preview/${encodeURIComponent(sessionId)}/update`, payload).then(response => {
            return buildApiResponse(response, {
                updated: !!(response.data?.data?.updated ?? response.data?.success),
                previewItem: response.data?.data?.previewItem
            });
        });
    },
    getImportPreviewPage: ({
        sessionId,
        page,
        pageSize,
        selectedOnly,
        previewIds,
        signal
    }: {
        sessionId: string,
        page?: number,
        pageSize?: number,
        selectedOnly?: boolean,
        previewIds?: number[],
        signal?: string
    }): ApiResponsePromise<Record<string, unknown>> => {
        const params: Record<string, string | number | boolean> = {};
        if (typeof page === 'number') {
            params['page'] = page;
        }
        if (typeof pageSize === 'number') {
            params['page_size'] = pageSize;
        }
        if (typeof selectedOnly === 'boolean') {
            params['selected_only'] = selectedOnly;
        }
        if (previewIds && previewIds.length > 0) {
            params['preview_ids'] = previewIds.join(',');
        }
        if (signal) {
            params['signal'] = signal;
        }

        return axios.get<ApiDataResponse<Record<string, unknown>>>(`bills/import/v2/preview/${encodeURIComponent(sessionId)}`, {
            params
        }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    confirmImportPreview: ({
        sessionId,
        previewUpdates,
        preserveUnpatchedSelection,
        historyRewriteAcknowledgement
    }: {
        sessionId: string,
        previewUpdates?: Record<string, unknown>[],
        preserveUnpatchedSelection?: boolean,
        historyRewriteAcknowledgement?: unknown | null
    }): ApiResponsePromise<Record<string, unknown>> => {
        const payload: Record<string, unknown> = {
            session_id: sessionId,
            preserve_unpatched_selection: !!preserveUnpatchedSelection,
            preview_updates: previewUpdates || []
        };
        if (historyRewriteAcknowledgement) {
            payload['history_rewrite_acknowledgement'] = historyRewriteAcknowledgement;
        }

        return axios.post<ApiDataResponse<Record<string, unknown>>>('bills/import/v2/confirm', payload, {
            timeout: DEFAULT_UPLOAD_API_TIMEOUT
        } as ApiRequestConfig).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    reviewImportTransferDecision: ({
        previewId,
        decision,
        payload
    }: {
        previewId: number,
        decision: 'accept' | 'reject' | 'clear',
        payload?: Record<string, unknown>
    }): ApiResponsePromise<Record<string, unknown>> => {
        return axios.post<ApiDataResponse<Record<string, unknown>>>(`bills/import/v2/preview-item/${encodeURIComponent(String(previewId))}/transfer-decision`, {
            decision,
            ...(payload || {})
        }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    getMatchingSessionCandidates: ({
        sessionId
    }: {
        sessionId: string
    }): ApiResponsePromise<MatchingSessionCandidatesResponse> => {
        return axios.get<ApiDataResponse<MatchingSessionCandidatesResponse>>(`matching/candidates?sessionId=${encodeURIComponent(sessionId)}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    getMatchingBillCandidates: ({
        billId
    }: {
        billId: string | number
    }): ApiResponsePromise<BillMatchingCandidatesResponse> => {
        return axios.get<ApiDataResponse<BillMatchingCandidatesResponse>>(`matching/candidates?billId=${encodeURIComponent(String(billId))}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    getMatchingBillFeedback: ({
        billId
    }: {
        billId: string | number
    }): ApiResponsePromise<BillMatchingFeedbackResponse> => {
        return axios.get<ApiDataResponse<BillMatchingFeedbackResponse>>(`matching/bills/${encodeURIComponent(String(billId))}/feedback`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    acceptMatchingCandidate: ({
        candidateId,
        payload
    }: {
        candidateId: string,
        payload?: Record<string, unknown>
    }): ApiResponsePromise<MatchingCandidateActionResponse> => {
        return postMatchingCandidateAction('accept', candidateId, payload);
    },
    rejectMatchingCandidate: ({
        candidateId,
        payload
    }: {
        candidateId: string,
        payload?: Record<string, unknown>
    }): ApiResponsePromise<MatchingCandidateActionResponse> => {
        return postMatchingCandidateAction('reject', candidateId, payload);
    },
    clearMatchingCandidate: ({
        candidateId,
        payload
    }: {
        candidateId: string,
        payload?: Record<string, unknown>
    }): ApiResponsePromise<MatchingCandidateActionResponse> => {
        return postMatchingCandidateAction('clear', candidateId, payload);
    },
    deleteMatchingPair: ({
        pairId
    }: {
        pairId: string | number
    }): ApiResponsePromise<MatchingPairOperationResponse> => {
        return axios.delete<ApiDataResponse<MatchingPairOperationResponse>>(`matching/pairs/${encodeURIComponent(String(pairId))}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    getMatchingPairs: ({
        pairType,
        page,
        pageSize
    }: {
        pairType?: string,
        page?: number,
        pageSize?: number
    } = {}): ApiResponsePromise<MatchingPairsResponse> => {
        const params: Record<string, string> = {};
        if (pairType) params['pair_type'] = pairType;
        if (page !== undefined) params['page'] = String(page);
        if (pageSize !== undefined) params['page_size'] = String(pageSize);
        return axios.get<ApiDataResponse<MatchingPairsResponse>>('matching/pairs', { params }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    reconcileMatchingHistory: ({
        billIds,
        families
    }: {
        billIds: number[],
        families?: string[]
    }): ApiResponsePromise<ReconcileHistoryResponse> => {
        const body: Record<string, unknown> = { billIds };
        if (families && families.length > 0) {
            body['families'] = families;
        }
        return axios.post<ApiDataResponse<ReconcileHistoryResponse>>('matching/reconcile-history', body).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    createManualPair: ({
        billId,
        candidateBillId,
        pairType
    }: {
        billId: number,
        candidateBillId: number,
        pairType?: string
    }): ApiResponsePromise<MatchingPairOperationResponse> => {
        return axios.post<ApiDataResponse<MatchingPairOperationResponse>>('matching/manual-pair', {
            billId,
            candidateBillId,
            pairType: pairType || 'transfer'
        }).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    getImportConfigs: ({ fileFormat }: { fileFormat?: string } = {}): ApiResponsePromise<any[]> => {
        return axios.get<ApiResponse<any[]>>('bills/import/configs', {
            params: {
                file_format: fileFormat
            }
        });
    },
    previewImportFile: ({ importFile, fileEncoding, delimiter }: { importFile: File, fileEncoding?: string, delimiter?: string }): ApiResponsePromise<any> => {
        return axios.postForm<ApiResponse<any>>('bills/import/preview', {
            file: importFile,
            fileEncoding: fileEncoding,
            delimiter: delimiter
        }, {
            timeout: DEFAULT_UPLOAD_API_TIMEOUT
        } as ApiRequestConfig);
    },
    previewImportFileFromTemp: ({ tempPath, fileEncoding, delimiter }: { tempPath: string, fileEncoding?: string, delimiter?: string }): ApiResponsePromise<any> => {
        return axios.postForm<ApiResponse<any>>('bills/import/preview', {
            temp_path: tempPath,
            fileEncoding: fileEncoding,
            delimiter: delimiter
        }, {
            timeout: DEFAULT_UPLOAD_API_TIMEOUT
        } as ApiRequestConfig);
    },
    parseGenericIntoSession: ({ sessionId, tempPath, columnMapping, transactionTypeMapping, hasHeaderLine, timeFormat, timezoneFormat, amountDecimalSeparator, amountDigitGroupingSymbol, delimiter }: {
        sessionId: string;
        tempPath: string;
        columnMapping: Record<string, number>;
        transactionTypeMapping?: Record<string, number>;
        hasHeaderLine?: boolean;
        timeFormat?: string;
        timezoneFormat?: string;
        amountDecimalSeparator?: string;
        amountDigitGroupingSymbol?: string;
        delimiter?: string;
    }): ApiResponsePromise<any> => {
        return axios.post<ApiResponse<any>>('bills/import/v2/parse_generic', {
            session_id: sessionId,
            temp_path: tempPath,
            column_mapping: columnMapping,
            transaction_type_mapping: transactionTypeMapping,
            has_header_line: hasHeaderLine,
            time_format: timeFormat,
            timezone_format: timezoneFormat,
            amount_decimal_separator: amountDecimalSeparator,
            amount_digit_grouping_symbol: amountDigitGroupingSymbol,
            delimiter: delimiter
        }, {
            timeout: DEFAULT_UPLOAD_API_TIMEOUT
        } as ApiRequestConfig);
    },
    matchImportConfig: ({ fileFormat, headers }: { fileFormat: string, headers: string[] }): ApiResponsePromise<any | null> => {
        return axios.post<ApiResponse<any | null>>('bills/import/configs/match', {
            fileFormat,
            headers
        });
    },
    suggestImportConfig: ({ fileFormat, headers, sampleRows }: { fileFormat: string, headers: string[], sampleRows?: string[][] }): ApiResponsePromise<any> => {
        return axios.post<ApiResponse<any>>('bills/import/configs/suggest', {
            fileFormat,
            headers,
            sampleRows
        });
    },
    saveImportConfig: (req: any): ApiResponsePromise<{ id: number }> => {
        return axios.post<ApiResponse<{ id: number }>>('bills/import/configs', req);
    },
    deleteImportConfig: ({ id }: { id: number | string }): ApiResponsePromise<boolean> => {
        return axios.delete<ApiResponse<boolean>>(`bills/import/configs/${id}`);
    },
    
};

export default importPreviewServices;
