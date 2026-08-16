import services from '@/lib/services.ts';
import { ImportTransaction } from '@/models/imported_transaction.ts';
import type { ImportPreviewRecord } from '../importPreview.ts';
import {
    getPreviewUpdateId,
    type ImportPreviewUpdatePayload
} from '../importPreviewUpdates.ts';
import type { ImportPreviewMetadata } from '../importPreviewIndex.ts';

interface ImportPreviewActionPreflushConflictRebaseOptions {
    isServerPaged: () => boolean;
    getTransaction: (previewId: number) => ImportTransaction | null;
    setSelectionMetadata: (metadata: ImportPreviewMetadata) => void;
    rebaseTransaction: (transaction: ImportTransaction, previewItem: ImportPreviewRecord) => void;
    reconcileDrafts: (previewUpdates: ImportPreviewUpdatePayload[]) => void;
}

/** 为批量 action 创建类型化冲突重基器，并只清理服务端已返回的冲突行草稿。 */
export function createImportPreviewActionPreflushConflictRebaser(
    options: ImportPreviewActionPreflushConflictRebaseOptions
): (error: unknown, previewUpdates: ImportPreviewUpdatePayload[]) => void {
    return (error, previewUpdates) => {
        const selectionConflict = services.getImportPreviewSelectionConflict(error);
        const rowConflict = services.getImportPreviewRowVersionConflict(error);
        if (!selectionConflict && !rowConflict) {
            return;
        }

        if (selectionConflict) {
            options.setSelectionMetadata(selectionConflict.metadata as ImportPreviewMetadata);
        }
        const authoritativeRows = selectionConflict?.previewItems
            || (rowConflict ? [rowConflict.previewItem] : []);
        const rebasedPreviewIds = new Set<number>();
        for (const previewItem of authoritativeRows) {
            const previewId = Number(previewItem.id);
            const transaction = options.getTransaction(previewId);
            if (!transaction) {
                continue;
            }
            options.rebaseTransaction(transaction, previewItem);
            rebasedPreviewIds.add(previewId);
        }
        if (options.isServerPaged() && rebasedPreviewIds.size > 0) {
            options.reconcileDrafts(previewUpdates.filter(update => {
                const previewId = getPreviewUpdateId(update);
                return previewId !== null && rebasedPreviewIds.has(previewId);
            }));
        }
    };
}
