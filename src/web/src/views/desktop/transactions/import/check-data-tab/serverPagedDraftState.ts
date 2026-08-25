import type { ImportTransaction } from '@/models/imported_transaction.ts';

import { cloneImportPreviewDraftTransaction } from '../importPreviewDrafts.ts';
import {
    getPreviewUpdateId,
    type ImportPreviewUpdateBaseline
} from '../importPreviewUpdates.ts';

export const importPreviewEditableDraftKeys = [
    'selected',
    'type',
    'categoryId',
    'sourceAmountCents',
    'destinationAmountCents',
    'sourceAccountId',
    'destinationAccountId',
    'tagIds',
    'counterparty',
    'paymentMethod',
    'comment',
    'isManuallyAnnotated',
    'recurringTemplateId',
    'recurringTemplateName',
    'recurringCandidateCount',
    'recurringMatchScore',
    'recurringMatchReasons',
    'recurringMatchedDate'
] as const;

const importPreviewValidityDraftKeys = [
    'isManuallyAnnotated',
    'type',
    'categoryId',
    'sourceAccountId',
    'destinationAccountId'
] as const satisfies readonly ImportPreviewEditableDraftKey[];

export type ImportPreviewEditableDraftKey = keyof ImportPreviewUpdateBaseline;
export type ImportPreviewEditableDraftValue = string | number | boolean | string[];
export type ImportPreviewEditableDraftState = ImportPreviewUpdateBaseline;
export type ServerPagedSelectionAction = 'select_all'
    | 'select_valid'
    | 'select_invalid'
    | 'select_needs_annotation'
    | 'select_none'
    | 'invert';

type PreviewIdResolver = (transaction: ImportTransaction) => number | null;

export function cloneImportTransaction(transaction: ImportTransaction): ImportTransaction {
    return cloneImportPreviewDraftTransaction(transaction);
}

function cloneImportPreviewEditableDraftValue(
    value: ImportPreviewEditableDraftValue
): ImportPreviewEditableDraftValue {
    return Array.isArray(value) ? [...value] : value;
}

export function captureImportPreviewEditableDraftState(
    transaction: ImportTransaction
): ImportPreviewEditableDraftState {
    return {
        selected: !!transaction.selected,
        type: transaction.type,
        categoryId: String(transaction.categoryId || ''),
        sourceAmountCents: transaction.sourceAmountCents,
        destinationAmountCents: transaction.destinationAmountCents,
        sourceAccountId: String(transaction.sourceAccountId || ''),
        destinationAccountId: String(transaction.destinationAccountId || ''),
        tagIds: [...(transaction.tagIds || [])],
        counterparty: String(transaction.counterparty || ''),
        paymentMethod: String(transaction.paymentMethod || ''),
        comment: String(transaction.comment || ''),
        isManuallyAnnotated: !!transaction.isManuallyAnnotated,
        recurringTemplateId: String(transaction.recurringTemplateId || ''),
        recurringTemplateName: String(transaction.recurringTemplateName || ''),
        recurringCandidateCount: Number(transaction.recurringCandidateCount || 0),
        recurringMatchScore: Number(transaction.recurringMatchScore || 0),
        recurringMatchReasons: String(transaction.recurringMatchReasons || ''),
        recurringMatchedDate: String(transaction.recurringMatchedDate || '')
    };
}

export function isImportPreviewEditableDraftValueEqual(
    left: ImportPreviewEditableDraftValue,
    right: ImportPreviewEditableDraftValue
): boolean {
    if (Array.isArray(left) || Array.isArray(right)) {
        return Array.isArray(left)
            && Array.isArray(right)
            && left.length === right.length
            && left.every((value, index) => value === right[index]);
    }
    return left === right;
}

export function applyImportPreviewEditableDraftDeltas(
    transaction: ImportTransaction,
    draftState: ImportPreviewEditableDraftState,
    baselineState: ImportPreviewEditableDraftState
): void {
    const target = transaction as unknown as Record<
        ImportPreviewEditableDraftKey,
        ImportPreviewEditableDraftValue
    >;
    for (const key of importPreviewEditableDraftKeys) {
        if (!isImportPreviewEditableDraftValueEqual(draftState[key], baselineState[key])) {
            target[key] = cloneImportPreviewEditableDraftValue(draftState[key]);
        }
    }
}

export function applyImportPreviewEditableDraftOverlay(
    transaction: ImportTransaction,
    draftState: ImportPreviewEditableDraftState,
    baselineState: ImportPreviewEditableDraftState,
    reconcileDerivedState: (transaction: ImportTransaction) => void
): void {
    applyImportPreviewEditableDraftDeltas(transaction, draftState, baselineState);
    reconcileDerivedState(transaction);
}

export function mergeImportPreviewEditableDraftBaseline(
    serverState: ImportPreviewEditableDraftState,
    draftState: ImportPreviewEditableDraftState,
    baselineState: ImportPreviewEditableDraftState
): ImportPreviewEditableDraftState {
    const nextBaseline = Object.fromEntries(
        importPreviewEditableDraftKeys.map(key => [
            key,
            cloneImportPreviewEditableDraftValue(serverState[key])
        ])
    ) as unknown as ImportPreviewEditableDraftState;
    for (const key of importPreviewEditableDraftKeys) {
        const draftChanged = !isImportPreviewEditableDraftValueEqual(
            draftState[key], baselineState[key]
        );
        const serverAcknowledgedDraft = isImportPreviewEditableDraftValueEqual(
            serverState[key], draftState[key]
        );
        if (draftChanged && !serverAcknowledgedDraft) {
            Object.assign(nextBaseline, {
                [key]: cloneImportPreviewEditableDraftValue(baselineState[key])
            });
        }
    }
    return nextBaseline;
}

export function hasServerPagedValidityDraftChanges(
    transaction: ImportTransaction,
    baselines: ReadonlyMap<number, ImportPreviewEditableDraftState>,
    getPreviewId: PreviewIdResolver
): boolean {
    const previewId = getPreviewId(transaction);
    if (previewId === null) {
        return false;
    }
    const baselineState = baselines.get(previewId);
    if (!baselineState) {
        return false;
    }
    const draftState = captureImportPreviewEditableDraftState(transaction);
    return importPreviewValidityDraftKeys.some(key => (
        !isImportPreviewEditableDraftValueEqual(draftState[key], baselineState[key])
    ));
}

export function shouldPersistValidityDraftsForSelection(
    action: ServerPagedSelectionAction
): boolean {
    return action === 'select_valid'
        || action === 'select_invalid'
        || action === 'select_needs_annotation';
}

export function cacheServerPagedDraftState(
    enabled: boolean,
    drafts: ReadonlyMap<number, ImportTransaction>,
    transaction: ImportTransaction,
    getPreviewId: PreviewIdResolver
): Map<number, ImportTransaction> {
    const previewId = enabled ? getPreviewId(transaction) : null;
    if (previewId === null) {
        return drafts as Map<number, ImportTransaction>;
    }
    const nextDrafts = new Map(drafts);
    nextDrafts.set(previewId, cloneImportTransaction(transaction));
    return nextDrafts;
}

export function resolveServerPagedEditingTransaction(
    editingTransaction: ImportTransaction | null,
    transactions: ImportTransaction[],
    getPreviewId: PreviewIdResolver
): { transaction: ImportTransaction | null; tags: string[] } | null {
    const editingPreviewId = editingTransaction ? getPreviewId(editingTransaction) : null;
    if (editingPreviewId === null) {
        return null;
    }
    const transaction = transactions.find(item => getPreviewId(item) === editingPreviewId) || null;
    return { transaction, tags: transaction ? [...transaction.tagIds] : [] };
}

export function dropAcknowledgedServerPagedDraftState<TSelectionBaseline>(
    enabled: boolean,
    previewIds: readonly number[],
    drafts: ReadonlyMap<number, ImportTransaction>,
    baselines: ReadonlyMap<number, ImportPreviewEditableDraftState>,
    selectionBaselines: ReadonlyMap<number, TSelectionBaseline>
): [Map<number, ImportTransaction>, Map<number, ImportPreviewEditableDraftState>, Map<number, TSelectionBaseline>] {
    const nextDrafts = new Map(drafts);
    const nextBaselines = new Map(baselines);
    const nextSelectionBaselines = new Map(selectionBaselines);
    if (enabled) {
        for (const previewId of previewIds) {
            nextDrafts.delete(previewId);
            nextBaselines.delete(previewId);
            nextSelectionBaselines.delete(previewId);
        }
    }
    return [nextDrafts, nextBaselines, nextSelectionBaselines];
}

export function reconcileServerPagedDraftStateAfterSelection(options: {
    persistedPreviewUpdates: Record<string, unknown>[];
    drafts: ReadonlyMap<number, ImportTransaction>;
    transactions: ImportTransaction[];
    baselines: ReadonlyMap<number, ImportPreviewEditableDraftState>;
    getPreviewId: PreviewIdResolver;
}): {
    drafts: Map<number, ImportTransaction>;
    baselines: Map<number, ImportPreviewEditableDraftState>;
} {
    const persistedPreviewIds = new Set(
        options.persistedPreviewUpdates
            .map(getPreviewUpdateId)
            .filter((previewId): previewId is number => previewId !== null)
    );
    const drafts = new Map(options.drafts);
    for (const transaction of options.transactions) {
        const previewId = options.getPreviewId(transaction);
        if (previewId !== null) {
            drafts.set(previewId, cloneImportTransaction(transaction));
        }
    }

    const baselines = new Map(options.baselines);
    for (const [previewId, draft] of drafts.entries()) {
        const draftState = captureImportPreviewEditableDraftState(draft);
        const baseline = baselines.get(previewId);
        baselines.set(previewId, !baseline || persistedPreviewIds.has(previewId)
            ? draftState
            : { ...baseline, selected: draftState.selected });
    }
    return { drafts, baselines };
}
