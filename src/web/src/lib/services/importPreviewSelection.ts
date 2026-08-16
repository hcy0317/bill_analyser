import axios from 'axios';

import type {
    ImportPreviewRecord,
    ImportPreviewSelectionConflict,
    ImportPreviewSelectionMetadata,
    ImportPreviewSelectionPatchResponse
} from '@/models/import_preview.ts';

import {
    buildApiResponse,
    type ApiDataResponse,
    type ApiResponsePromise
} from './http.ts';

interface ImportPreviewSelectionConflictEnvelope {
    code?: unknown;
    data?: unknown;
}

interface ImportPreviewSelectionConflictPayload {
    expected_selection_hash?: unknown;
    actual_selection_hash?: unknown;
    metadata?: unknown;
    previewItems?: unknown;
}

function getImportPreviewSelectionConflict(
    error: unknown
): ImportPreviewSelectionConflict | null {
    const response = (error as {
        response?: { status?: number; data?: ImportPreviewSelectionConflictEnvelope };
    })?.response;
    if (response?.status !== 409 || response.data?.code !== 'PREVIEW_SELECTION_CONFLICT') {
        return null;
    }
    const data = response.data.data;
    if (!data || typeof data !== 'object') {
        return null;
    }
    const conflict = data as ImportPreviewSelectionConflictPayload;
    if (
        typeof conflict.expected_selection_hash !== 'string'
        || !conflict.expected_selection_hash
        || typeof conflict.actual_selection_hash !== 'string'
        || !conflict.actual_selection_hash
        || !conflict.metadata
        || typeof conflict.metadata !== 'object'
        || !Array.isArray(conflict.previewItems)
        || conflict.previewItems.some(item => !item || typeof item !== 'object')
    ) {
        return null;
    }
    return {
        expected_selection_hash: conflict.expected_selection_hash,
        actual_selection_hash: conflict.actual_selection_hash,
        metadata: conflict.metadata as ImportPreviewSelectionMetadata,
        previewItems: conflict.previewItems as ImportPreviewRecord[]
    };
}

const importPreviewSelectionServices = {
    getImportPreviewSelectionConflict,
    // 中文说明：使用服务端 selection_hash 对跨页选择增量执行集合级 CAS。
    patchImportPreviewSelection: ({
        sessionId,
        expectedSelectionHash,
        selectedIds,
        deselectedIds
    }: {
        sessionId: string,
        expectedSelectionHash?: string,
        selectedIds: number[],
        deselectedIds: number[]
    }): ApiResponsePromise<ImportPreviewSelectionPatchResponse> => {
        const payload: Record<string, unknown> = {
            selectionAction: 'patch',
            selected_ids: selectedIds,
            deselected_ids: deselectedIds
        };
        if (expectedSelectionHash) {
            payload['expected_selection_hash'] = expectedSelectionHash;
        }
        return axios.put<ApiDataResponse<ImportPreviewSelectionPatchResponse>>(
            `bills/import/v2/preview/${encodeURIComponent(sessionId)}/selection`,
            payload
        ).then(response => buildApiResponse(response, response.data?.data));
    }
};

export default importPreviewSelectionServices;
