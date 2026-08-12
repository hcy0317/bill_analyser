import type { ImportTransaction } from '@/models/imported_transaction.ts';

import type { ImportPreviewRecord } from './importPreview';

export interface DecisionPreviewReplacement {
    removedPreviewIds: number[];
    upsertedPreviewItems: ImportPreviewRecord[];
}

export function resolveDecisionPreviewReplacement(payload: unknown): DecisionPreviewReplacement | null {
    if (!payload || typeof payload !== 'object') return null;
    const data = payload as Record<string, unknown>;
    const removedPreviewIds = Array.isArray(data['removedPreviewIds'])
        ? data['removedPreviewIds'].map(Number).filter(Number.isFinite)
        : [];
    const upsertedPreviewItems = Array.isArray(data['upsertedPreviewItems'])
        ? data['upsertedPreviewItems'].filter(item => item && typeof item === 'object') as ImportPreviewRecord[]
        : [];
    return removedPreviewIds.length > 0 || upsertedPreviewItems.length > 0
        ? { removedPreviewIds, upsertedPreviewItems }
        : null;
}

export function applyServerPagedReclassification(
    serverPaged: boolean,
    refreshCurrentPage: () => void,
): boolean {
    if (!serverPaged) return false;
    refreshCurrentPage();
    return true;
}

export function applyNonServerPagedReplacement(
    currentRows: ImportTransaction[],
    removedPreviewIds: number[],
    upsertedRows: ImportTransaction[],
    getPreviewId: (row: ImportTransaction) => number | null,
): ImportTransaction[] {
    const removed = new Set(removedPreviewIds);
    const upsertedByPreviewId = new Map<number, ImportTransaction>();
    for (const row of upsertedRows) {
        const previewId = getPreviewId(row);
        if (previewId !== null) {
            upsertedByPreviewId.set(previewId, row);
        }
    }
    const consumedPreviewIds = new Set<number>();
    const remainingRows = currentRows.flatMap(row => {
        const previewId = getPreviewId(row);
        if (previewId === null || !removed.has(previewId)) {
            return [row];
        }
        const replacement = upsertedByPreviewId.get(previewId);
        if (!replacement) {
            return [];
        }
        consumedPreviewIds.add(previewId);
        return [replacement];
    });
    const appendedRows = upsertedRows.filter(row => {
        const previewId = getPreviewId(row);
        return previewId === null || !consumedPreviewIds.has(previewId);
    });
    return [...remainingRows, ...appendedRows];
}
