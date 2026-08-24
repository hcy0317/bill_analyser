import axios from 'axios';

import type {
    RecurringAcceptResponse,
    RecurringDetectResponse,
    RecurringSuggestionsResponse
} from '@/models/recurring_suggestion.ts';
import {
    normalizeDetectResponse,
    normalizeSuggestionsResponse
} from '@/models/recurring_suggestion.ts';
import {
    normalizeActionCenterAnomalyResponse,
    type ActionCenterAnomalyResponse
} from '@/views/base/action-center/actionCenterModel.ts';
import type { AnomalyListRequest, PagedStatusRequest } from './contracts.ts';
import {
    buildApiResponse,
    type ApiDataResponse,
    type ApiResponsePromise
} from './http.ts';

const actionCenterServices = {
    getRecurringSuggestions: (
        { status, limit, offset }: PagedStatusRequest = {}
    ): ApiResponsePromise<RecurringSuggestionsResponse> => {
        return axios.get<ApiDataResponse<RecurringSuggestionsResponse>>('recurring/suggestions', {
            params: { status, limit, offset }
        }).then(response => buildApiResponse(response, normalizeSuggestionsResponse(response.data?.data)));
    },

    detectRecurringPatterns: (): ApiResponsePromise<RecurringDetectResponse> => {
        return axios.post<ApiDataResponse<RecurringDetectResponse>>('recurring/suggestions/detect')
            .then(response => buildApiResponse(response, normalizeDetectResponse(response.data?.data)));
    },

    acceptRecurringSuggestion: (
        { suggestionId }: { suggestionId: number }
    ): ApiResponsePromise<RecurringAcceptResponse> => {
        return axios.post<ApiDataResponse<RecurringAcceptResponse>>(`recurring/suggestions/${suggestionId}/accept`)
            .then(response => buildApiResponse(response, response.data?.data));
    },

    rejectRecurringSuggestion: (
        { suggestionId }: { suggestionId: number }
    ): ApiResponsePromise<unknown> => {
        return axios.post(`recurring/suggestions/${suggestionId}/reject`)
            .then(response => buildApiResponse(response, response.data?.data));
    },

    getAnomalies: (
        { months }: AnomalyListRequest = {}
    ): ApiResponsePromise<ActionCenterAnomalyResponse> => {
        return axios.get<ApiDataResponse<unknown>>('insights/anomalies', { params: { months } })
            .then(response => buildApiResponse(
                response,
                normalizeActionCenterAnomalyResponse(response.data?.data)
            ));
    }
};

export default actionCenterServices;
