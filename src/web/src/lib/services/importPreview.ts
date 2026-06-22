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

/**
 * 中文说明：统一提交匹配候选 accept/reject/clear 动作，并保持返回值仍映射成旧 ApiResponse.result 形态。
 */
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

// 中文说明：导入预览域服务 facade，集中保留旧 endpoint、timeout 和 payload 字段名，供根 services.ts 继续透出。
const importPreviewServices = {
    // 中文说明：提交通用导入文件解析请求，按旧表单字段发送列映射、类型映射和文件格式参数。
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
    // 中文说明：按 session 获取导入学习建议；有局部预览更新时使用 POST，否则保持 GET 兼容旧调用路径。
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
    // 中文说明：把当前预览更新提升为导入学习规则，payload 字段名保持后端 learning lifecycle 契约。
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
    // 中文说明：更新单条预览草稿并把后端 success/data.updated 归一成前端使用的 updated 布尔值。
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
    // 中文说明：读取导入预览分页数据，显式透传 page、selected_only、preview_ids 和 signal 筛选条件。
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
    // 中文说明：确认导入预览，保留未 patch 选择状态和历史重写确认字段，避免确认链路丢失用户选择。
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
    // 中文说明：提交导入预览转账建议的人工审核决定，兼容 accept/reject/clear 三种状态。
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
    // 中文说明：按导入 session 读取匹配候选，供导入预览页面展示同批候选摘要。
    getMatchingSessionCandidates: ({
        sessionId
    }: {
        sessionId: string
    }): ApiResponsePromise<MatchingSessionCandidatesResponse> => {
        return axios.get<ApiDataResponse<MatchingSessionCandidatesResponse>>(`matching/candidates?sessionId=${encodeURIComponent(sessionId)}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    // 中文说明：按账单 id 读取匹配候选，供账单详情或匹配中心复用同一候选接口。
    getMatchingBillCandidates: ({
        billId
    }: {
        billId: string | number
    }): ApiResponsePromise<BillMatchingCandidatesResponse> => {
        return axios.get<ApiDataResponse<BillMatchingCandidatesResponse>>(`matching/candidates?billId=${encodeURIComponent(String(billId))}`).then(response => {
            return buildApiResponse(response, response.data?.data);
        });
    },
    // 中文说明：读取账单匹配反馈，维持 matching feedback 与导入预览反馈共用的响应适配。
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
    // 中文说明：读取匹配 pair 列表并保留 pair_type/page/page_size 查询字段，供历史匹配中心分页展示。
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
    // 中文说明：触发历史匹配重算，按账单 id 和可选 family 限定 reconciliation 范围。
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
    // 中文说明：创建人工匹配 pair，默认 pairType 为 transfer 以兼容导入预览转账确认入口。
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
    // 中文说明：把临时预览文件按用户列映射解析进既有导入 session，保持 v2 parse_generic payload 字段名。
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
