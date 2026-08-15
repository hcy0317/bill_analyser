import { isCanonicalTruthy as isImportPreviewCanonicalTruthy } from '@/models/imported_transaction/matching.ts';

import type {
    ImportPreviewSignalDecision,
    ImportPreviewSignalState,
    ImportPreviewSignalStatus
} from './types.ts';

const TRANSFER_LEARNING_LEVELS = new Set(['yellow', 'green', 'blue']);

export interface TransferLearningMembershipProjection {
    transferLearningLevel: string;
    transferOwnsLearningMembership: boolean;
    learningStatus: ImportPreviewSignalStatus | null;
}

export function resolveTransferLearningMembership(
    state: ImportPreviewSignalState,
    transferStatus: ImportPreviewSignalStatus | null,
    independentLearningStatus: ImportPreviewSignalStatus | null
): TransferLearningMembershipProjection {
    const transferLearningLevel = (state.transferLearningLevel || '').trim().toLowerCase();
    const transferOwnsLearningMembership = !!(state.transferCandidateType || '').trim()
        && TRANSFER_LEARNING_LEVELS.has(transferLearningLevel)
        && !isImportPreviewCanonicalTruthy(state.transferSuppressed)
        && (transferStatus === 'pending' || transferStatus === 'accepted');
    const learningTransferProtected = !!(state.transferCandidateType || '').trim()
        && independentLearningStatus === 'skipped'
        && (state.learningTitle || '').trim().toLowerCase().includes('transfer preview is protected');

    return {
        transferLearningLevel,
        transferOwnsLearningMembership,
        learningStatus: transferOwnsLearningMembership
            ? transferStatus
            : (learningTransferProtected ? null : independentLearningStatus)
    };
}

export interface TransferActionProjection {
    pendingActions: ImportPreviewSignalDecision[];
    reviewedActions: ImportPreviewSignalDecision[];
}

export function buildTransferActionProjection(learningLevel: string): TransferActionProjection {
    const pendingActions: ImportPreviewSignalDecision[] = learningLevel === 'yellow'
        ? []
        : [
            { decision: 'accept', labelKey: 'Accept', color: 'warning' },
            { decision: 'reject', labelKey: 'Reject', color: 'error' }
        ];
    const reviewedActions: ImportPreviewSignalDecision[] = learningLevel === 'blue'
        ? [
            { decision: 'clear', labelKey: 'Clear', color: 'warning' },
            { decision: 'reject', labelKey: 'Reject', color: 'error' }
        ]
        : [{ decision: 'clear', labelKey: 'Clear', color: 'warning' }];

    return { pendingActions, reviewedActions };
}

export interface LearningActionProjection {
    isGreenLearning: boolean;
    pendingActions: ImportPreviewSignalDecision[];
    reviewedActions: ImportPreviewSignalDecision[];
}

export function buildLearningActionProjection(
    state: ImportPreviewSignalState,
    transferOwnsLearningMembership: boolean
): LearningActionProjection {
    const normalizedMode = (state.learningMode || '').trim().toLowerCase();
    const normalizedSignalState = (state.learningSignalState || '').trim().toLowerCase();
    const isGreenLearning = !transferOwnsLearningMembership && (
        normalizedSignalState === 'green'
        || normalizedSignalState === 'auto_applied'
        || normalizedMode === 'green'
        || isImportPreviewCanonicalTruthy(state.learningAutoApplied)
    );
    const pendingActions: ImportPreviewSignalDecision[] = transferOwnsLearningMembership
        ? []
        : [
            { decision: 'accept', labelKey: 'Apply Suggestion', color: isGreenLearning ? 'success' : 'secondary' },
            { decision: 'reject', labelKey: 'Reject Learning Suggestion', color: 'error' }
        ];
    const reviewedActions: ImportPreviewSignalDecision[] = isGreenLearning
        ? [{ decision: 'reject', labelKey: 'Reject Learning Suggestion', color: 'error' }]
        : [];

    return { isGreenLearning, pendingActions, reviewedActions };
}
