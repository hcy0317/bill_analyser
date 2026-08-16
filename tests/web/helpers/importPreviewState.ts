import type {
    ImportPreviewStateSignalFamily,
    ImportPreviewStateSignalStatus,
    ImportPreviewStateSnapshot,
} from '@/models/import_preview_state.ts';

type DecisionFamily = keyof ImportPreviewStateSnapshot['decisions'];

export function previewStateSnapshot(
    signals: ImportPreviewStateSignalFamily[],
    statuses: Partial<Record<DecisionFamily, ImportPreviewStateSignalStatus>> = {},
): ImportPreviewStateSnapshot {
    const evidence = (family: DecisionFamily) => ({
        status: statuses[family] ?? (signals.includes(family) ? 'pending' : 'absent'),
        has_evidence: signals.includes(family),
    });
    return {
        projection_version: 1,
        signals: [...signals],
        issues: [],
        decisions: {
            transfer: evidence('transfer'),
            history: evidence('history'),
            learning: evidence('learning'),
            llm: evidence('llm'),
        },
        effective: {
            category_id: null,
            source_account_id: null,
            destination_account_id: null,
        },
    };
}
