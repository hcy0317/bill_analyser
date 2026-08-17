import axios from 'axios';

import type {
    ImportPreviewExpectedState,
    ImportPreviewRecord
} from '@/models/import_preview.ts';

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

interface ImportPreviewRecurringDecisionPayload {
    expectedState: ImportPreviewExpectedState;
    responseMode: 'preview-item';
    recurringId?: number;
}

const importPreviewRecurringServices = {
    updateImportPreviewRecurringMatch: ({
        previewId,
        recurringId,
        expectedState
    }: {
        previewId: number;
        recurringId: number | null;
        expectedState: ImportPreviewExpectedState;
    }): ApiResponsePromise<ImportPreviewRecurringDecisionResponse> => {
        const endpoint = `bills/import/v2/preview-item/${encodeURIComponent(previewId)}/recurring-match`;
        const payload: ImportPreviewRecurringDecisionPayload = {
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
