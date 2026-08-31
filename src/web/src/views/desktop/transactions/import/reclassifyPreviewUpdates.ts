import services from '@/lib/services.ts';

import { getPreviewUpdateId, type ImportPreviewUpdatePayload } from './importPreviewUpdates.ts';

/** 中文说明：为整会话重新分类合并已浏览草稿，并分页补齐未浏览行的并发版本令牌。 */
export async function buildReclassifyPreviewUpdates(
    sessionId: string,
    serverPaged: boolean,
    totalPreviewCount: number,
    trackedUpdates: ImportPreviewUpdatePayload[]
): Promise<ImportPreviewUpdatePayload[]> {
    if (!serverPaged) {
        return trackedUpdates;
    }

    const updatesById = new Map<number, ImportPreviewUpdatePayload>();
    for (const update of trackedUpdates) {
        const previewId = getPreviewUpdateId(update);
        if (previewId !== null) {
            updatesById.set(previewId, update);
        }
    }
    if (updatesById.size >= totalPreviewCount) {
        return Array.from(updatesById.values());
    }

    const pageSize = 200;
    const totalPages = Math.ceil(totalPreviewCount / pageSize);
    for (let page = 1; page <= totalPages; page += 1) {
        const response = await services.getImportPreviewPage({ sessionId, page, pageSize });
        for (const preview of response.data.result?.preview || []) {
            if (!updatesById.has(preview.id)) {
                updatesById.set(preview.id, {
                    id: preview.id,
                    expected_row_version: preview.row_version
                });
            }
        }
    }
    return Array.from(updatesById.values());
}
