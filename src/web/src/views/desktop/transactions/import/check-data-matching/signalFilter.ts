import type { ImportPreviewSignalViewModel, ImportPreviewVisibleSignalFilterValue } from './types.ts';

import { normalizeDedupType } from './shared.ts';

// Signal filters consume only the visible projection so hidden evidence cannot become filter membership.
export function matchesImportPreviewSignalFilter(
    viewModel: ImportPreviewSignalViewModel,
    filter: ImportPreviewVisibleSignalFilterValue | null
): boolean {
    if (filter === null) {
        return true;
    }

    if (viewModel.signalFamilies) {
        return viewModel.signalFamilies.includes(filter);
    }

    const normalizedDedupType = normalizeDedupType(viewModel.dedup?.dedupType);
    if (filter === 'parser') {
        return !!viewModel.parser
            && normalizedDedupType !== 'platform_bank'
            && viewModel.transferSuggestion?.status !== 'pending'
            && !viewModel.historyRewrite
            && !viewModel.learning
            && !viewModel.llm;
    }

    if (filter === 'platform_duplicate') {
        return normalizedDedupType === 'platform_bank';
    }

    if (filter === 'transfer') {
        return viewModel.transferSuggestion?.status === 'pending';
    }

    if (filter === 'history') {
        return !!viewModel.historyRewrite;
    }

    if (filter === 'learning') {
        return !!viewModel.learning;
    }

    if (filter === 'llm') {
        return !!viewModel.llm;
    }

    return false;
}
