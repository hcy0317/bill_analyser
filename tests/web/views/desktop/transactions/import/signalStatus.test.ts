import { describe, expect, test } from '@jest/globals';

import {
    IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES,
    IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES,
    importPreviewMatchingHasUnknownSignalStatus,
    importPreviewSignalSectionHasUnknownStatus,
    importPreviewSignalStatusIsUnknown,
    resolveImportPreviewSignalStatus
} from '@/views/desktop/transactions/import/check-data-matching/signalStatus.ts';
import { buildImportPreviewSignalViewModel } from '@/views/desktop/transactions/import/checkDataMatching.ts';

describe('import preview signal status contract', () => {
    test('resolves the first non-empty authoritative status field', () => {
        expect(resolveImportPreviewSignalStatus({
            review_status: '  ',
            status: ' Accepted ',
            lifecycle_status: '__ignored__'
        })).toBe('accepted');
        expect(resolveImportPreviewSignalStatus({
            review_status: 'pending',
            status: '__ignored__'
        })).toBe('pending');
        expect(resolveImportPreviewSignalStatus(null)).toBe('');
    });

    test.each([
        'review_status',
        'status',
        'lifecycle_status',
        'signal_state'
    ])('fails closed for unknown %s', statusField => {
        expect(importPreviewSignalSectionHasUnknownStatus('transfer', {
            [statusField]: '__future_unknown__'
        })).toBe(true);
    });

    test('keeps family-specific canonical statuses explicit', () => {
        expect(IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES).toStrictEqual([
            'pending',
            'none',
            'suppressed',
            'accepted',
            'rejected',
            'skipped',
            'auto_applied',
            'auto-applied'
        ]);
        expect(IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES).toStrictEqual([
            ...IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES,
            'needs_review'
        ]);
        expect(importPreviewSignalStatusIsUnknown('learning', 'needs_review')).toBe(false);
        expect(importPreviewSignalStatusIsUnknown('transfer', 'needs_review')).toBe(true);
        expect(importPreviewSignalStatusIsUnknown('history', 'needs_review')).toBe(true);
        expect(importPreviewSignalStatusIsUnknown('llm', 'auto-applied')).toBe(false);
        expect(importPreviewSignalStatusIsUnknown('llm', '')).toBe(false);
    });

    test('never downgrades an unknown non-empty status into a pending action', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            learningStatus: '__future_unknown__',
            learningTitle: 'learning evidence',
            llmStatus: '__future_unknown__',
            llmTitle: 'LLM evidence'
        });

        expect(viewModel.learning).toBeNull();
        expect(viewModel.llm).toBeNull();
    });

    test('detects any unknown signal family without rewriting evidence', () => {
        const matching = {
            transfer: { review_status: 'pending' },
            learning: { lifecycle_status: '__future_unknown__', reason: 'retained evidence' },
            llm: { review_status: 'accepted' }
        };

        expect(importPreviewMatchingHasUnknownSignalStatus(matching)).toBe(true);
        expect(matching.learning.reason).toBe('retained evidence');
        expect(importPreviewMatchingHasUnknownSignalStatus({
            learning: { review_status: 'needs_review' }
        })).toBe(false);
        expect(importPreviewMatchingHasUnknownSignalStatus({
            reconciliation: { status: '__future_unknown__', planned_operation: 'update_history' }
        })).toBe(true);
    });
});
