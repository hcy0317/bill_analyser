import type { ImportTransaction } from '@/models/imported_transaction.ts';

import type { ImportPreviewSignalStatus } from './checkDataMatching.ts';

export function getImportPreviewTransferSignalStatus(item: ImportTransaction): ImportPreviewSignalStatus | null {
    const reviewStatus = item.getTransferSuggestionReviewStatus();
    if (reviewStatus === 'pending') {
        return item.matching?.transfer?.suppressed ? null : 'pending';
    }
    if (reviewStatus === 'accepted' || reviewStatus === 'rejected' || reviewStatus === 'skipped') {
        return reviewStatus;
    }

    if (item.hasTransferSuggestion()) {
        return 'pending';
    }

    if (item.isTransferSuggestionAccepted()) {
        return 'accepted';
    }

    if (item.isTransferSuggestionRejected()) {
        return 'rejected';
    }

    return null;
}

export function getImportPreviewTransferSignalTitle(item: ImportTransaction): string {
    return item.transferSuggestionReason || item.matching?.transfer.reason || '';
}
