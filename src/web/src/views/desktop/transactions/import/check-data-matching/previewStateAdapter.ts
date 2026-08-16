import type {
    ImportPreviewFamilyEvidence,
    ImportPreviewStateSignalFamily,
    ImportPreviewStateSignalStatus,
    ImportPreviewStateSnapshot,
} from '@/models/import_preview_state.ts';
import { isImportPreviewStateSnapshot } from '@/models/import_preview_state.ts';

import { buildImportPreviewSignalViewModel } from './signalViewModel.ts';
import type {
    ImportPreviewSignalReviewView,
    ImportPreviewSignalState,
    ImportPreviewSignalStatus,
    ImportPreviewSignalViewModel,
    ImportPreviewSignalViewModelOptions,
} from './types.ts';

function toUiStatus(status: ImportPreviewStateSignalStatus): ImportPreviewSignalStatus | null {
    if (status === 'pending' || status === 'needs_review') {
        return 'pending';
    }
    if (status === 'accepted' || status === 'auto_applied') {
        return 'accepted';
    }
    if (status === 'rejected' || status === 'skipped') {
        return status;
    }
    return null;
}

function reviewFallback(
    evidence: ImportPreviewFamilyEvidence,
    labelKey: string,
): ImportPreviewSignalReviewView {
    const status = toUiStatus(evidence.status) ?? 'pending';
    return {
        status,
        labelKey,
        title: '',
        color: status === 'accepted' ? 'success' : (status === 'rejected' ? 'error' : 'warning'),
        actions: [],
        detailLines: [],
    };
}

function applyDecisionStatus(
    current: ImportPreviewSignalReviewView | null,
    evidence: ImportPreviewFamilyEvidence,
    labelKey: string,
): ImportPreviewSignalReviewView {
    const fallback = reviewFallback(evidence, labelKey);
    return current
        ? { ...current, status: fallback.status, color: fallback.color }
        : fallback;
}

function hasFamily(
    families: ReadonlySet<ImportPreviewStateSignalFamily>,
    family: ImportPreviewStateSignalFamily,
): boolean {
    return families.has(family);
}

// Legacy matching JSON supplies labels and evidence details only. Membership and
// decision status come exclusively from the versioned backend snapshot.
export function buildImportPreviewSignalViewModelFromSnapshot(
    state: ImportPreviewSignalState,
    snapshot: ImportPreviewStateSnapshot | null | undefined,
    options: ImportPreviewSignalViewModelOptions = {},
): ImportPreviewSignalViewModel {
    const snapshotIsSupported = isImportPreviewStateSnapshot(snapshot);
    const signalFamilies = snapshotIsSupported ? snapshot.signals : [];
    const families = new Set<ImportPreviewStateSignalFamily>(signalFamilies);
    const decisions = snapshotIsSupported ? snapshot.decisions : null;
    const transferStatus = decisions && hasFamily(families, 'transfer')
        ? toUiStatus(decisions.transfer.status)
        : null;
    const learningStatus = decisions && hasFamily(families, 'learning')
        ? toUiStatus(decisions.learning.status)
        : null;
    const llmStatus = decisions && hasFamily(families, 'llm')
        ? toUiStatus(decisions.llm.status)
        : null;

    const legacyView = buildImportPreviewSignalViewModel({
        ...state,
        parserId: hasFamily(families, 'parser') ? state.parserId : '',
        parserTags: hasFamily(families, 'parser') ? state.parserTags : [],
        dedupType: hasFamily(families, 'platform_duplicate') ? state.dedupType : '',
        dedupSourceIds: hasFamily(families, 'platform_duplicate') ? state.dedupSourceIds : [],
        transferStatus,
        learningStatus,
        learningStatusAuthoritative: true,
        llmStatus,
        llmStatusAuthoritative: true,
        reconciliationStatus: hasFamily(families, 'history')
            ? (toUiStatus(decisions?.history.status ?? 'absent') ?? '')
            : '',
        reconciliationTitle: hasFamily(families, 'history') ? state.reconciliationTitle : '',
        reconciliationPlannedOperation: hasFamily(families, 'history')
            ? state.reconciliationPlannedOperation
            : '',
        reconciliationDestructiveAckRequired: hasFamily(families, 'history')
            ? state.reconciliationDestructiveAckRequired
            : false,
    }, options);

    const parser = hasFamily(families, 'parser')
        ? (legacyView.parser ?? {
            parserId: state.parserId || 'unknown',
            label: state.parserId || 'Parser',
            color: 'grey',
            title: '',
            detailLines: [],
        })
        : null;
    const dedup = hasFamily(families, 'platform_duplicate')
        ? (legacyView.dedup ?? {
            dedupType: 'platform_bank',
            labelKey: 'platform_bank',
            label: 'Platform Duplicate',
            title: '',
            color: 'secondary',
            sourceCount: 0,
            detailLines: [],
        })
        : null;
    const transferSuggestion = decisions && hasFamily(families, 'transfer')
        ? applyDecisionStatus(legacyView.transferSuggestion, decisions.transfer, 'Likely Transfer')
        : null;
    const historyRewrite = decisions && hasFamily(families, 'history')
        ? applyDecisionStatus(legacyView.historyRewrite, decisions.history, 'History Rewrite')
        : null;
    const learningEvidence = decisions
        && decisions.learning.status === 'skipped'
        && hasFamily(families, 'transfer')
        ? decisions.transfer
        : decisions?.learning;
    const learning = decisions && hasFamily(families, 'learning')
        ? applyDecisionStatus(
            legacyView.learning,
            learningEvidence ?? decisions.learning,
            'Learning Suggestion',
        )
        : null;
    const llm = decisions && hasFamily(families, 'llm')
        ? applyDecisionStatus(legacyView.llm, decisions.llm, 'LLM Suggestion')
        : null;

    return {
        ...legacyView,
        parser,
        dedup,
        historyRewrite,
        transferSuggestion,
        learning,
        llm,
        signalFamilies: [...signalFamilies],
        hasAnySignal: signalFamilies.length > 0 || !!legacyView.recurring,
    };
}
