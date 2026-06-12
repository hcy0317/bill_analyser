import { normalizeStrictAbsoluteCents } from './strictCents.ts';

export interface ImportCheckLearningPreviewTextSyncState {
    previewId?: number | null;
    counterparty?: string;
    paymentMethod?: string;
    comment?: string;
    selected?: boolean;
}

export interface ImportCheckLearningPreviewTextSyncPayload {
    id: number;
    counterparty: string;
    paymentMethod: string;
    description: string;
    isSelected: boolean;
}

export interface ImportCheckLearningDecisionBaseline {
    inputFingerprint: string;
    type: number;
    categoryId: string;
    recurringTemplateId: string;
    sourceAccountId: string;
    destinationAccountId: string;
}

function normalizePreviewId(previewId?: number | null): number | null {
    if (typeof previewId !== 'number' || !Number.isInteger(previewId) || previewId <= 0) {
        return null;
    }

    return previewId;
}

export function buildImportCheckLearningPreviewTextSyncPayload(
    state: ImportCheckLearningPreviewTextSyncState
): ImportCheckLearningPreviewTextSyncPayload | null {
    const previewId = normalizePreviewId(state.previewId);
    if (previewId === null) {
        return null;
    }

    return {
        id: previewId,
        counterparty: state.counterparty || '',
        paymentMethod: state.paymentMethod || '',
        description: state.comment || '',
        isSelected: !!state.selected
    };
}

export function hasImportCheckLearningTextDrift(
    baseline: ImportCheckLearningDecisionBaseline,
    current: ImportCheckLearningDecisionBaseline
): boolean {
    return baseline.inputFingerprint !== current.inputFingerprint;
}

export function hasImportCheckLearningExpectedStateDrift(
    baseline: ImportCheckLearningDecisionBaseline,
    current: ImportCheckLearningDecisionBaseline
): boolean {
    return baseline.type !== current.type
        || baseline.categoryId !== current.categoryId
        || baseline.recurringTemplateId !== current.recurringTemplateId
        || baseline.sourceAccountId !== current.sourceAccountId
        || baseline.destinationAccountId !== current.destinationAccountId;
}

export function normalizeImportPreviewAmountCents(amountInCents: unknown, fallbackInCents: number): number {
    return normalizeStrictAbsoluteCents(amountInCents, fallbackInCents);
}
