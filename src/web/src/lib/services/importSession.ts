import axios from 'axios';

import { DEFAULT_IMPORT_API_TIMEOUT } from '@/consts/api.ts';
import type {
    ImportPreviewConfirmCommand,
    ImportPreviewConfirmPayload,
    ImportConfirmResult,
    ImportSessionSummary,
    ImportSessionVersionConflict
} from '@/models/import_preview.ts';

import {
    buildApiResponse,
    type ApiDataResponse,
    type ApiRequestConfig,
    type ApiResponsePromise
} from './http.ts';

interface ImportSessionVersionConflictEnvelope {
    code?: unknown;
    data?: unknown;
}

interface ImportSessionVersionConflictPayload {
    expected_session_version?: unknown;
    actual_session_version?: unknown;
    session?: unknown;
}

function getImportSessionVersionConflict(error: unknown): ImportSessionVersionConflict | null {
    const response = (error as {
        response?: { status?: number; data?: ImportSessionVersionConflictEnvelope };
    })?.response;
    if (response?.status !== 409 || response.data?.code !== 'IMPORT_SESSION_VERSION_CONFLICT') {
        return null;
    }
    const data = response.data.data;
    if (!data || typeof data !== 'object') {
        return null;
    }
    const conflict = data as ImportSessionVersionConflictPayload;
    const expected = conflict.expected_session_version;
    const actual = conflict.actual_session_version;
    const session = conflict.session;
    if (
        !Number.isInteger(expected)
        || !Number.isInteger(actual)
        || !session
        || typeof session !== 'object'
    ) {
        return null;
    }
    return {
        expected_session_version: expected as number,
        actual_session_version: actual as number,
        session: session as ImportSessionSummary
    };
}

const importSessionServices = {
    getImportSessionVersionConflict,
    getImportSession: ({ sessionId }: { sessionId: string }): ApiResponsePromise<ImportSessionSummary> => (
        axios.get<ApiDataResponse<ImportSessionSummary>>(
            `bills/import/v2/session/${encodeURIComponent(sessionId)}`
        ).then(response => buildApiResponse(response, response.data?.data))
    ),
    // 中文说明：confirm 的 session token 独立于 preview row/selection token；不传 token 时保留旧客户端兼容。
    confirmImportPreview: ({
        sessionId,
        previewUpdates,
        preserveUnpatchedSelection,
        expectedSessionVersion,
        historyRewriteAcknowledgement
    }: ImportPreviewConfirmCommand): ApiResponsePromise<ImportConfirmResult> => {
        const payload: ImportPreviewConfirmPayload = {
            session_id: sessionId,
            preserve_unpatched_selection: !!preserveUnpatchedSelection,
            preview_updates: previewUpdates || []
        };
        if (Number.isSafeInteger(expectedSessionVersion) && Number(expectedSessionVersion) > 0) {
            payload['expected_session_version'] = expectedSessionVersion;
        }
        if (historyRewriteAcknowledgement) {
            payload['history_rewrite_acknowledgement'] = historyRewriteAcknowledgement;
        }

        return axios.post<ApiDataResponse<ImportConfirmResult>>('bills/import/v2/confirm', payload, {
            timeout: DEFAULT_IMPORT_API_TIMEOUT
        } as ApiRequestConfig).then(response => buildApiResponse(response, response.data?.data));
    }
};

export default importSessionServices;
