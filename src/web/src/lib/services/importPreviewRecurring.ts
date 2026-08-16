import axios from 'axios';

import type { ImportPreviewRecord } from '@/models/import_preview.ts';

import {
    buildApiResponse,
    type ApiDataResponse,
    type ApiResponsePromise
} from './http.ts';

interface ImportPreviewRecurringDecisionResponse {
    previewId?: number;
    sessionId?: string;
    recurringId?: number | null;
    previewItem?: ImportPreviewRecord;
    preview?: ImportPreviewRecord[];
}

const importPreviewRecurringServices = {
    updateImportPreviewRecurringMatch: ({
        previewId,
        recurringId,
        expectedState
    }: {
        previewId: number;
        recurringId: number | null;
        expectedState: Record<string, string | number | null>;
    }): ApiResponsePromise<ImportPreviewRecurringDecisionResponse> => {
        const endpoint = `bills/import/v2/preview-item/${encodeURIComponent(previewId)}/recurring-match`;
        const payload: Record<string, unknown> = {
            expectedState,
            responseMode: 'preview-item'
        };
        if (recurringId === null) {
            return axios.delete<ApiDataResponse<ImportPreviewRecurringDecisionResponse>>(
                endpoint,
                { data: payload }
            ).then(response => buildApiResponse(response, response.data?.data));
        }
        return axios.put<ApiDataResponse<ImportPreviewRecurringDecisionResponse>>(
            endpoint,
            { ...payload, recurringId }
        ).then(response => buildApiResponse(response, response.data?.data));
    }
};

export default importPreviewRecurringServices;
