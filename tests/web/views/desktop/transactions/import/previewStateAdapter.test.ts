import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, test } from '@jest/globals';

import type {
    ImportPreviewFamilyEvidence,
    ImportPreviewStateSnapshot,
} from '@/models/import_preview_state.ts';
import {
    buildImportPreviewSignalViewModelFromSnapshot,
    matchesImportPreviewSignalFilter,
    type ImportPreviewSignalState,
    type ImportPreviewVisibleSignalFilterValue,
} from '@/views/desktop/transactions/import/checkDataMatching.ts';
import {
    mapImportPreviewIndexResponseItem,
} from '@/views/desktop/transactions/import/importPreviewIndex.ts';
import type { ImportPreviewRecord } from '@/views/desktop/transactions/import/importPreview.ts';
import {
    buildImportTransactionFromPreviewRecord,
} from '@/views/desktop/transactions/import/importPreviewTransaction.ts';

interface PreviewStateFixtureCase {
    name: string;
    input: {
        transfer?: ImportPreviewFamilyEvidence;
        history?: ImportPreviewFamilyEvidence;
        learning?: ImportPreviewFamilyEvidence;
        llm?: ImportPreviewFamilyEvidence;
    };
    legacy_payload: Record<string, unknown>;
    expected_signals: ImportPreviewVisibleSignalFilterValue[];
    expected_issues: ImportPreviewStateSnapshot['issues'];
}

const EMPTY_EVIDENCE: ImportPreviewFamilyEvidence = {
    status: 'absent',
    has_evidence: false,
};
const SIX_FAMILIES: ImportPreviewVisibleSignalFilterValue[] = [
    'parser',
    'platform_duplicate',
    'transfer',
    'history',
    'learning',
    'llm',
];

function loadFixture(): PreviewStateFixtureCase[] {
    const fixturePath = resolve(
        process.cwd(),
        '../../tests/fixtures/import_preview_state_kernel_v1.json',
    );
    return JSON.parse(readFileSync(fixturePath, 'utf8')) as PreviewStateFixtureCase[];
}

function snapshotFor(fixture: PreviewStateFixtureCase): ImportPreviewStateSnapshot {
    return {
        projection_version: 1,
        signals: fixture.expected_signals,
        issues: fixture.expected_issues,
        decisions: {
            transfer: fixture.input.transfer ?? EMPTY_EVIDENCE,
            history: fixture.input.history ?? EMPTY_EVIDENCE,
            learning: fixture.input.learning ?? EMPTY_EVIDENCE,
            llm: fixture.input.llm ?? EMPTY_EVIDENCE,
        },
        effective: {
            category_id: null,
            source_account_id: null,
            destination_account_id: null,
        },
    };
}

function stateFor(payload: Record<string, unknown>): ImportPreviewSignalState {
    const matching = (payload['preview_matching_feedback'] ?? {}) as Record<string, Record<string, unknown>>;
    const transfer = matching['transfer'] ?? {};
    const learning = matching['learning'] ?? {};
    const llm = matching['llm'] ?? {};
    const reconciliation = matching['reconciliation'] ?? {};
    return {
        parserId: String(payload['preview_parser_id'] ?? ''),
        dedupType: String(payload['dedup_type'] ?? ''),
        transferStatus: String(transfer['review_status'] ?? ''),
        transferCandidateType: String(transfer['candidate_type'] ?? ''),
        transferLearningLevel: String(transfer['learning_level'] ?? ''),
        learningStatus: String(learning['review_status'] ?? ''),
        learningStatusAuthoritative: true,
        llmStatus: String(llm['review_status'] ?? ''),
        llmStatusAuthoritative: true,
        reconciliationStatus: String(reconciliation['status'] ?? ''),
        reconciliationPlannedOperation: String(reconciliation['planned_operation'] ?? ''),
        reconciliationDestructiveAckRequired: !!reconciliation['destructive_ack_required'],
    };
}

describe('typed import preview state adapter', () => {
    test.each(loadFixture())('$name exposes exactly the kernel-owned signal memberships', fixture => {
        const viewModel = buildImportPreviewSignalViewModelFromSnapshot(
            stateFor(fixture.legacy_payload),
            snapshotFor(fixture),
        );
        const visibleFamilies = SIX_FAMILIES.filter(family => (
            matchesImportPreviewSignalFilter(viewModel, family)
        ));

        expect(visibleFamilies).toStrictEqual(fixture.expected_signals);
    });

    test('fails closed when a canonical snapshot is absent even if legacy fields claim signals', () => {
        const legacyState: ImportPreviewSignalState = {
            parserId: 'wechat',
            dedupType: 'platform_bank',
            transferStatus: 'pending',
            learningStatus: 'accepted',
            llmStatus: 'pending',
            reconciliationPlannedOperation: 'update_history',
        };
        const viewModel = buildImportPreviewSignalViewModelFromSnapshot(legacyState, undefined);

        expect(SIX_FAMILIES.some(family => matchesImportPreviewSignalFilter(viewModel, family)))
            .toBe(false);
    });

    test('fails closed for a malformed v1 snapshot instead of throwing', () => {
        const malformedSnapshot = {
            ...snapshotFor(loadFixture()[0]!),
            decisions: { transfer: { status: 'pending', has_evidence: true } },
        } as unknown as ImportPreviewStateSnapshot;

        expect(() => buildImportPreviewSignalViewModelFromSnapshot({}, malformedSnapshot)).not.toThrow();
        const viewModel = buildImportPreviewSignalViewModelFromSnapshot({}, malformedSnapshot);
        expect(SIX_FAMILIES.some(family => matchesImportPreviewSignalFilter(viewModel, family)))
            .toBe(false);
    });

    test('uses typed decision state instead of a stale legacy review status', () => {
        const snapshot = snapshotFor(loadFixture()[3]!);
        snapshot.decisions.transfer = { status: 'rejected', has_evidence: true };
        snapshot.signals = ['transfer'];

        const viewModel = buildImportPreviewSignalViewModelFromSnapshot(
            { transferStatus: 'pending', transferTitle: 'legacy pending' },
            snapshot,
        );

        expect(viewModel.transferSuggestion?.status).toBe('rejected');
    });

    test('preserves the snapshot through detail and server-index DTO mappings', () => {
        const snapshot = snapshotFor(loadFixture()[0]!);
        const record: ImportPreviewRecord = {
            id: 7,
            row_version: 5,
            preview_state: snapshot,
        };
        const transaction = buildImportTransactionFromPreviewRecord(record, 0, {
            categoriesById: {},
            timeZone: 'Asia/Shanghai',
        });
        const indexItem = mapImportPreviewIndexResponseItem({
            id: 7,
            preview_state: snapshot,
        });

        expect(transaction.previewState).toBe(snapshot);
        expect(indexItem.previewState).toBe(snapshot);
    });
});
