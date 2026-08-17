import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const axiosGet = jest.fn<(...args: any[]) => Promise<any>>();
const axiosPost = jest.fn<(...args: any[]) => Promise<any>>();
const axiosPut = jest.fn<(...args: any[]) => Promise<any>>();
const axiosDelete = jest.fn<(...args: any[]) => Promise<any>>();
const axiosPostForm = jest.fn<(...args: any[]) => Promise<any>>();

jest.mock('axios', () => ({
    __esModule: true,
    default: {
        get: axiosGet,
        post: axiosPost,
        put: axiosPut,
        delete: axiosDelete,
        postForm: axiosPostForm
    }
}));

import { TransactionType } from '@/core/transaction.ts';
import services from '@/lib/services/importPreview.ts';
import type { ImportConfigSaveRequest } from '@/models/import_config.ts';
import type {
    ImportPreviewActionScope,
    ImportPreviewHistoryRewriteAcknowledgement
} from '@/models/import_preview.ts';

function dataEnvelope<T>(data: T, success = true): Record<string, unknown> {
    return { status: 200, data: { success, data } };
}

function buildImportConfigSaveRequest(name: string): ImportConfigSaveRequest {
    return {
        name,
        fileFormat: 'csv',
        description: '',
        fieldMappings: {},
        sampleHeaders: ['time', 'amount'],
        dateFormat: 'YYYY-MM-DD',
        delimiter: ',',
        encoding: 'utf-8',
        skipRows: 0,
        hasHeader: true,
        customRules: {},
        isDefault: false
    };
}

beforeEach(() => {
    for (const mock of [axiosGet, axiosPost, axiosPut, axiosDelete, axiosPostForm]) {
        mock.mockReset().mockResolvedValue(dataEnvelope({}));
    }
});

describe('import preview upload and learning service behavior', () => {
    test('serializes generic import mappings and header choices into multipart fields', async () => {
        const file = new File(['time,amount'], 'bills.csv', { type: 'text/csv' });
        await services.parseImportTransaction({
            fileType: 'csv',
            fileEncoding: 'utf-8',
            importFile: file,
            columnMapping: { 0: 1, 1: 2 },
            transactionTypeMapping: { expense: TransactionType.Expense },
            hasHeaderLine: false,
            timeFormat: 'YYYY-MM-DD',
            timezoneFormat: 'UTC+8',
            amountDecimalSeparator: '.',
            amountDigitGroupingSymbol: ',',
            geoSeparator: ',',
            geoOrder: 'lat_lng',
            tagSeparator: ';',
            delimiter: ','
        });

        expect(axiosPostForm).toHaveBeenCalledWith(
            'bills/parse_import',
            expect.objectContaining({
                fileType: 'csv',
                file,
                columnMapping: JSON.stringify({ 0: 1, 1: 2 }),
                transactionTypeMapping: JSON.stringify({ expense: TransactionType.Expense }),
                hasHeaderLine: 'false',
                delimiter: ','
            }),
            expect.objectContaining({ timeout: expect.any(Number) })
        );

        await services.parseImportTransaction({ fileType: 'csv', importFile: file });
        expect(axiosPostForm).toHaveBeenLastCalledWith(
            'bills/parse_import',
            expect.objectContaining({
                columnMapping: undefined,
                transactionTypeMapping: undefined,
                hasHeaderLine: undefined
            }),
            expect.any(Object)
        );
    });

    test('uses GET for an empty learning query and POST when scoped updates are present', async () => {
        axiosGet.mockResolvedValueOnce(dataEnvelope({ suggestions: [1] }));
        const empty = await services.getImportLearningSuggestions({ sessionId: 'session/1' });
        expect(axiosGet).toHaveBeenCalledWith('bills/import/v2/learning/session/1/suggestions');
        expect(empty.data.result).toEqual({ suggestions: [1] });

        axiosPost.mockResolvedValueOnce(dataEnvelope({ suggestions: [2] }));
        const selectedScope: ImportPreviewActionScope = {
            kind: 'selected',
            selection_hash: 'hash'
        };
        const invalidTopLevelIds = {
            sessionId: 'session/2',
            actionScope: selectedScope,
            // @ts-expect-error preview ids belong inside an explicit_selected action scope.
            previewIds: [8, 9],
        } satisfies Parameters<typeof services.getImportLearningSuggestions>[0];
        expect(invalidTopLevelIds.previewIds).toEqual([8, 9]);

        const scoped = await services.getImportLearningSuggestions({
            sessionId: 'session/2',
            previewUpdates: [{ id: 8 }],
            actionScope: selectedScope
        });
        expect(axiosPost).toHaveBeenCalledWith('bills/import/v2/learning/session/2/suggestions', {
            preview_updates: [{ id: 8 }],
            action_scope: selectedScope
        });
        expect(scoped.data.result).toEqual({ suggestions: [2] });
    });

    test('promotes the exact preview scope and preserves empty arrays as explicit user choices', async () => {
        axiosPost.mockResolvedValueOnce(dataEnvelope({ promoted: 0 }));
        const response = await services.promoteImportLearning({
            sessionId: 'learning-session',
            previewUpdates: [],
            actionScope: { kind: 'explicit_selected', preview_ids: [8, 9] }
        });

        expect(axiosPost).toHaveBeenCalledWith('bills/import/v2/learning/learning-session/promote', {
            preview_updates: [],
            action_scope: { kind: 'explicit_selected', preview_ids: [8, 9] }
        });
        expect(response.data.result).toEqual({ promoted: 0 });
    });
});

describe('import preview lifecycle service behavior', () => {
    test('submits recurring decisions through the typed preview service for PUT and DELETE', async () => {
        axiosPut.mockResolvedValueOnce(dataEnvelope({
            sessionId: 'recurring/session',
            previewItem: { id: 7, row_version: 9, preview_recurring_id: 17 }
        }));
        axiosDelete.mockResolvedValueOnce(dataEnvelope({
            sessionId: 'recurring/session',
            previewItem: { id: 7, row_version: 10, preview_recurring_id: null }
        }));
        const expectedState = { sessionId: 'recurring/session', rowVersion: 8 };

        const accepted = await services.updateImportPreviewRecurringMatch({
            previewId: 7,
            recurringId: 17,
            expectedState
        });
        const cleared = await services.updateImportPreviewRecurringMatch({
            previewId: 7,
            recurringId: null,
            expectedState: { ...expectedState, rowVersion: 9 }
        });

        expect(axiosPut).toHaveBeenCalledWith(
            'bills/import/v2/preview-item/7/recurring-match',
            {
                recurringId: 17,
                expectedState,
                responseMode: 'preview-item'
            }
        );
        expect(axiosDelete).toHaveBeenCalledWith(
            'bills/import/v2/preview-item/7/recurring-match',
            {
                data: {
                    expectedState: { ...expectedState, rowVersion: 9 },
                    responseMode: 'preview-item'
                }
            }
        );
        expect(accepted.data.result.previewItem?.row_version).toBe(9);
        expect(cleared.data.result.previewItem?.row_version).toBe(10);
    });

    test('normalizes update success from either the data field or the legacy success flag', async () => {
        axiosPut
            .mockResolvedValueOnce({ data: { success: true, data: { updated: false, previewItem: { id: 7, row_version: 9 } } } })
            .mockResolvedValueOnce({ data: { success: true } });

        const explicit = await services.updateImportPreviewItem({
            sessionId: 'session/a',
            payload: { id: 7, amountCents: 1234, expectedRowVersion: 8 }
        });
        const fallback = await services.updateImportPreviewItem({
            sessionId: 'session/b',
            payload: { id: 8 }
        });

        expect(axiosPut).toHaveBeenNthCalledWith(1, 'bills/import/v2/preview/session%2Fa/update', {
            id: 7,
            amountCents: 1234,
            expected_row_version: 8
        });
        expect(explicit.data.result).toEqual({ updated: false, previewItem: { id: 7, row_version: 9 } });
        expect(fallback.data.result).toEqual({ updated: true, previewItem: undefined });
    });

    test('recognizes only the typed preview row version conflict envelope', () => {
        const previewItem = { id: 7, row_version: 9 };
        expect(services.getImportPreviewRowVersionConflict({
            response: {
                status: 409,
                data: {
                    success: false,
                    code: 'PREVIEW_ROW_VERSION_CONFLICT',
                    error: 'Preview row changed, please refresh',
                    data: {
                        expected_row_version: 8,
                        actual_row_version: 9,
                        previewItem
                    }
                }
            }
        })).toEqual({
            expected_row_version: 8,
            actual_row_version: 9,
            previewItem
        });
        expect(services.getImportPreviewRowVersionConflict({
            response: { status: 409, data: { code: 'OTHER_CONFLICT' } }
        })).toBeNull();
        expect(services.getImportPreviewRowVersionConflict(new Error('network'))).toBeNull();
    });

    test('recognizes only a complete typed import session version conflict', () => {
        const session = {
            session_id: 'session-conflict',
            session_version: 9,
            status: 'previewing',
            created_at: '2026-08-17T00:00:00Z',
            parsed_count: 2,
            preview_count: 1,
            file_paths: []
        };
        expect(services.getImportSessionVersionConflict({
            response: {
                status: 409,
                data: {
                    code: 'IMPORT_SESSION_VERSION_CONFLICT',
                    data: {
                        expected_session_version: 8,
                        actual_session_version: 9,
                        session
                    }
                }
            }
        })).toEqual({
            expected_session_version: 8,
            actual_session_version: 9,
            session
        });
        expect(services.getImportSessionVersionConflict({
            response: { status: 409, data: { code: 'OTHER_CONFLICT' } }
        })).toBeNull();
        expect(services.getImportSessionVersionConflict({
            response: {
                status: 409,
                data: { code: 'IMPORT_SESSION_VERSION_CONFLICT', data: null }
            }
        })).toBeNull();
        expect(services.getImportSessionVersionConflict({
            response: {
                status: 409,
                data: {
                    code: 'IMPORT_SESSION_VERSION_CONFLICT',
                    data: {
                        expected_session_version: '8',
                        actual_session_version: 9,
                        session
                    }
                }
            }
        })).toBeNull();
    });

    test('patches selection with the collection token and recognizes only its typed conflict', async () => {
        axiosPut.mockResolvedValueOnce(dataEnvelope({
            updated: 2,
            selectionAction: 'patch',
            metadata: { selection_hash: 'fnv1a32:22222222' }
        }));

        const response = await services.patchImportPreviewSelection({
            sessionId: 'session/selection',
            expectedSelectionHash: 'fnv1a32:11111111',
            selectedIds: [7],
            deselectedIds: [8]
        });

        expect(axiosPut).toHaveBeenCalledWith(
            'bills/import/v2/preview/session%2Fselection/selection',
            {
                selectionAction: 'patch',
                expected_selection_hash: 'fnv1a32:11111111',
                selected_ids: [7],
                deselected_ids: [8]
            }
        );
        expect(response.data.result.metadata.selection_hash).toBe('fnv1a32:22222222');

        axiosPut.mockResolvedValueOnce(dataEnvelope({
            updated: 1,
            selectionAction: 'patch',
            metadata: { selection_hash: 'fnv1a32:33333333' }
        }));
        await services.patchImportPreviewSelection({
            sessionId: 'legacy-selection',
            selectedIds: [9],
            deselectedIds: []
        });
        expect(axiosPut).toHaveBeenNthCalledWith(
            2,
            'bills/import/v2/preview/legacy-selection/selection',
            {
                selectionAction: 'patch',
                selected_ids: [9],
                deselected_ids: []
            }
        );

        const conflict = services.getImportPreviewSelectionConflict({
            response: {
                status: 409,
                data: {
                    code: 'PREVIEW_SELECTION_CONFLICT',
                    data: {
                        expected_selection_hash: 'fnv1a32:11111111',
                        actual_selection_hash: 'fnv1a32:22222222',
                        metadata: { selection_hash: 'fnv1a32:22222222' },
                        previewItems: [{ id: 7, preview_selected: false }]
                    }
                }
            }
        });
        expect(conflict).toEqual({
            expected_selection_hash: 'fnv1a32:11111111',
            actual_selection_hash: 'fnv1a32:22222222',
            metadata: { selection_hash: 'fnv1a32:22222222' },
            previewItems: [{ id: 7, preview_selected: false }]
        });
        expect(services.getImportPreviewSelectionConflict({
            response: { status: 409, data: { code: 'PREVIEW_ROW_VERSION_CONFLICT' } }
        })).toBeNull();
        for (const malformed of [
            { response: { status: 409, data: { code: 'PREVIEW_SELECTION_CONFLICT' } } },
            {
                response: {
                    status: 409,
                    data: {
                        code: 'PREVIEW_SELECTION_CONFLICT',
                        data: {
                            expected_selection_hash: '',
                            actual_selection_hash: 'fnv1a32:22222222',
                            metadata: {},
                            previewItems: []
                        }
                    }
                }
            },
            {
                response: {
                    status: 409,
                    data: {
                        code: 'PREVIEW_SELECTION_CONFLICT',
                        data: {
                            expected_selection_hash: 'fnv1a32:11111111',
                            actual_selection_hash: 'fnv1a32:22222222',
                            metadata: {},
                            previewItems: [null]
                        }
                    }
                }
            }
        ]) {
            expect(services.getImportPreviewSelectionConflict(malformed)).toBeNull();
        }
    });

    test('applies a conditional selection through the typed service with both CAS tokens', async () => {
        axiosPut.mockResolvedValueOnce(dataEnvelope({
            updated: 1,
            applied_preview_updates: 1,
            selectionAction: 'select_valid',
            metadata: { selection_hash: 'fnv1a32:22222222' },
            previewItems: [{ id: 7, row_version: 4, preview_selected: true }]
        }));

        const response = await services.applyImportPreviewSelectionAction({
            sessionId: 'session/conditional',
            selectionAction: 'select_valid',
            filters: { category: '8' },
            expectedSelectionHash: 'fnv1a32:11111111',
            previewUpdates: [{
                id: 7,
                expected_row_version: 3,
                category_id: 8
            }]
        });

        expect(axiosPut).toHaveBeenCalledWith(
            'bills/import/v2/preview/session%2Fconditional/selection',
            {
                selectionAction: 'select_valid',
                filters: { category: '8' },
                expected_selection_hash: 'fnv1a32:11111111',
                preview_updates: [{
                    id: 7,
                    expected_row_version: 3,
                    category_id: 8
                }]
            }
        );
        expect(response.data.result.previewItems[0]?.row_version).toBe(4);
    });

    test('omits absent preview filters and includes false/zero values when they are explicit', async () => {
        await services.getImportPreviewPage({ sessionId: 'plain session' });
        expect(axiosGet).toHaveBeenNthCalledWith(1, 'bills/import/v2/preview/plain%20session', { params: {} });

        await services.getImportPreviewPage({
            sessionId: 'filtered/session',
            page: 0,
            pageSize: 25,
            selectedOnly: false,
            previewIds: [3, 5],
            signal: 'learning'
        });
        expect(axiosGet).toHaveBeenNthCalledWith(2, 'bills/import/v2/preview/filtered%2Fsession', {
            params: {
                page: 0,
                page_size: 25,
                selected_only: false,
                preview_ids: '3,5',
                signal: 'learning'
            }
        });
    });

    test('confirms with stable defaults and attaches history acknowledgement only when provided', async () => {
        axiosGet.mockResolvedValueOnce(dataEnvelope({
            session_id: 's2',
            session_version: 7,
            status: 'previewing',
            created_at: '2026-08-17T00:00:00Z',
            parsed_count: 2,
            preview_count: 2,
            file_paths: []
        }));
        const session = await services.getImportSession({ sessionId: 's2' });
        expect(axiosGet).toHaveBeenCalledWith('bills/import/v2/session/s2');
        expect(session.data.result.session_version).toBe(7);

        await services.confirmImportPreview({ sessionId: 's1' });
        expect(axiosPost).toHaveBeenNthCalledWith(1, 'bills/import/v2/confirm', {
            session_id: 's1',
            preserve_unpatched_selection: false,
            preview_updates: []
        }, expect.objectContaining({ timeout: 1_800_000 }));

        const acknowledgement: ImportPreviewHistoryRewriteAcknowledgement = {
            acknowledged: true,
            selected_preview_ids: [4],
            operations: [{
                preview_id: 4,
                operation_id: 'rewrite-4',
                planned_operation: 'replace_history_bill',
                history_bill_id: 40,
                history_bill_version: 3,
                acknowledgement_token: 'ack-token'
            }],
            selection_scope: {
                mode: 'visible-preview',
                selected_count: 1,
                history_rewrite_count: 1
            }
        };
        await services.confirmImportPreview({
            sessionId: 's2',
            previewUpdates: [{ id: 4 }],
            preserveUnpatchedSelection: true,
            expectedSessionVersion: 7,
            historyRewriteAcknowledgement: acknowledgement
        });
        expect(axiosPost).toHaveBeenNthCalledWith(2, 'bills/import/v2/confirm', {
            session_id: 's2',
            preserve_unpatched_selection: true,
            preview_updates: [{ id: 4 }],
            expected_session_version: 7,
            history_rewrite_acknowledgement: acknowledgement
        }, expect.objectContaining({ timeout: 1_800_000 }));
    });

    test('submits transfer review decisions with encoded ids and typed expected state', async () => {
        await services.reviewImportTransferDecision({
            previewId: 12,
            decision: 'accept',
            payload: {
                expectedState: { sessionId: 'session-12', rowVersion: 4 },
                responseMode: 'preview-item'
            }
        });
        await services.reviewImportTransferDecision({
            previewId: 13,
            decision: 'clear',
            payload: {
                expectedState: { sessionId: 'session-13', rowVersion: 5 },
                responseMode: 'preview-item'
            }
        });

        expect(axiosPost).toHaveBeenNthCalledWith(1, 'bills/import/v2/preview-item/12/transfer-decision', {
            decision: 'accept',
            expectedState: { sessionId: 'session-12', rowVersion: 4 },
            responseMode: 'preview-item'
        });
        expect(axiosPost).toHaveBeenNthCalledWith(2, 'bills/import/v2/preview-item/13/transfer-decision', {
            decision: 'clear',
            expectedState: { sessionId: 'session-13', rowVersion: 5 },
            responseMode: 'preview-item'
        });
    });

    test('reclassifies versioned preview updates through the shared import service', async () => {
        axiosPost.mockResolvedValueOnce(dataEnvelope({
            session_id: 'session/reclassify',
            updated: 1,
            preview: [{ id: 21, row_version: 6 }]
        }));

        const response = await services.reclassifyImportPreview({
            sessionId: 'session/reclassify',
            previewUpdates: [{ id: 21, expected_row_version: 5 }]
        });

        expect(axiosPost).toHaveBeenCalledWith(
            'bills/import/v2/reclassify/session%2Freclassify',
            { preview_updates: [{ id: 21, expected_row_version: 5 }] }
        );
        expect(response.data.result).toEqual({
            session_id: 'session/reclassify',
            updated: 1,
            preview: [{ id: 21, row_version: 6 }]
        });
    });
});

describe('matching and import configuration service behavior', () => {
    test('uses encoded candidate and bill identities across read and review actions', async () => {
        await services.getMatchingSessionCandidates({ sessionId: 'session/a b' });
        await services.getMatchingBillCandidates({ billId: 'bill/7' });
        await services.getMatchingBillFeedback({ billId: 'bill/7' });
        await services.acceptMatchingCandidate({
            candidateId: 'candidate/a',
            payload: {
                expectedState: { sessionId: 'session-1', rowVersion: 3 },
                responseMode: 'preview-item'
            }
        });
        await services.rejectMatchingCandidate({ candidateId: 'candidate/b' });
        await services.clearMatchingCandidate({ candidateId: 'candidate/c' });
        await services.deleteMatchingPair({ pairId: 'pair/9' });

        expect(axiosGet).toHaveBeenNthCalledWith(1, 'matching/candidates?sessionId=session%2Fa%20b');
        expect(axiosGet).toHaveBeenNthCalledWith(2, 'matching/candidates?billId=bill%2F7');
        expect(axiosGet).toHaveBeenNthCalledWith(3, 'matching/bills/bill%2F7/feedback');
        expect(axiosPost).toHaveBeenNthCalledWith(1, 'matching/candidates/candidate%2Fa/accept', {
            expectedState: { sessionId: 'session-1', rowVersion: 3 },
            responseMode: 'preview-item'
        });
        expect(axiosPost).toHaveBeenNthCalledWith(2, 'matching/candidates/candidate%2Fb/reject', {});
        expect(axiosPost).toHaveBeenNthCalledWith(3, 'matching/candidates/candidate%2Fc/clear', {});
        expect(axiosDelete).toHaveBeenCalledWith('matching/pairs/pair%2F9');
    });

    test('builds matching list, reconciliation and manual-pair requests for default and scoped forms', async () => {
        await services.getMatchingPairs();
        await services.getMatchingPairs({ pairType: 'duplicate', page: 0, pageSize: 50 });
        await services.reconcileMatchingHistory({ billIds: [1, 2] });
        await services.reconcileMatchingHistory({ billIds: [3], families: ['transfer', 'duplicate'] });
        await services.createManualPair({ billId: 1, candidateBillId: 2 });
        await services.createManualPair({ billId: 3, candidateBillId: 4, pairType: 'duplicate' });

        expect(axiosGet).toHaveBeenNthCalledWith(1, 'matching/pairs', { params: {} });
        expect(axiosGet).toHaveBeenNthCalledWith(2, 'matching/pairs', {
            params: { pair_type: 'duplicate', page: '0', page_size: '50' }
        });
        expect(axiosPost).toHaveBeenNthCalledWith(1, 'matching/reconcile-history', { billIds: [1, 2] });
        expect(axiosPost).toHaveBeenNthCalledWith(2, 'matching/reconcile-history', {
            billIds: [3], families: ['transfer', 'duplicate']
        });
        expect(axiosPost).toHaveBeenNthCalledWith(3, 'matching/manual-pair', {
            billId: 1, candidateBillId: 2, pairType: 'transfer'
        });
        expect(axiosPost).toHaveBeenNthCalledWith(4, 'matching/manual-pair', {
            billId: 3, candidateBillId: 4, pairType: 'duplicate'
        });
    });

    test('preserves legacy import-config and temporary-file request envelopes', async () => {
        const file = new File(['a,b'], 'import.csv');
        await services.getImportConfigs();
        await services.getImportConfigs({ fileFormat: 'csv' });
        await services.previewImportFile({ importFile: file, fileEncoding: 'utf-8', delimiter: ',' });
        await services.previewImportFileFromTemp({
            sessionId: 'session-1',
            tempPath: 'tmp/file.csv',
            delimiter: ';'
        });
        await services.parseGenericIntoSession({
            sessionId: 'session-1',
            tempPath: 'tmp/file.csv',
            columnMapping: { time: 0, amount: 1 },
            transactionTypeMapping: { expense: 3 },
            hasHeaderLine: true,
            timeFormat: 'YYYY-MM-DD',
            timezoneFormat: 'UTC+8',
            amountDecimalSeparator: '.',
            amountDigitGroupingSymbol: ',',
            fileEncoding: 'gb18030',
            delimiter: ';'
        });
        await services.matchImportConfig({ fileFormat: 'csv', headers: ['time', 'amount'] });
        await services.suggestImportConfig({
            fileFormat: 'csv', headers: ['time', 'amount'], sampleRows: [['2026-01-01', '1.23']]
        });
        const saveRequest = buildImportConfigSaveRequest('bank csv');
        await services.saveImportConfig(saveRequest);
        await services.deleteImportConfig({ id: 'config/7' });

        expect(axiosGet).toHaveBeenNthCalledWith(1, 'bills/import/configs', { params: { file_format: undefined } });
        expect(axiosGet).toHaveBeenNthCalledWith(2, 'bills/import/configs', { params: { file_format: 'csv' } });
        expect(axiosPostForm).toHaveBeenNthCalledWith(1, 'bills/import/preview', {
            file,
            fileEncoding: 'utf-8',
            delimiter: ','
        }, expect.objectContaining({ timeout: expect.any(Number) }));
        expect(axiosPostForm).toHaveBeenNthCalledWith(2, 'bills/import/preview', {
            session_id: 'session-1',
            temp_path: 'tmp/file.csv',
            fileEncoding: undefined,
            delimiter: ';'
        }, expect.any(Object));
        expect(axiosPost).toHaveBeenNthCalledWith(1, 'bills/import/v2/parse_generic', {
            session_id: 'session-1',
            temp_path: 'tmp/file.csv',
            column_mapping: { time: 0, amount: 1 },
            transaction_type_mapping: { expense: 3 },
            has_header_line: true,
            time_format: 'YYYY-MM-DD',
            timezone_format: 'UTC+8',
            amount_decimal_separator: '.',
            amount_digit_grouping_symbol: ',',
            file_encoding: 'gb18030',
            delimiter: ';'
        }, expect.objectContaining({ timeout: expect.any(Number) }));
        expect(axiosPost).toHaveBeenNthCalledWith(2, 'bills/import/configs/match', {
            fileFormat: 'csv', headers: ['time', 'amount']
        });
        expect(axiosPost).toHaveBeenNthCalledWith(3, 'bills/import/configs/suggest', {
            fileFormat: 'csv', headers: ['time', 'amount'], sampleRows: [['2026-01-01', '1.23']]
        });
        expect(axiosPost).toHaveBeenNthCalledWith(4, 'bills/import/configs', saveRequest);
        expect(axiosDelete).toHaveBeenCalledWith('bills/import/configs/config/7');
    });

    test('adapts Rust import-config data envelopes to the UI result contract', async () => {
        axiosGet.mockResolvedValueOnce(dataEnvelope([{ id: 7, name: 'CSV' }]));
        axiosPost
            .mockResolvedValueOnce(dataEnvelope({ id: 7, name: 'CSV' }))
            .mockResolvedValueOnce(dataEnvelope({ columnMapping: { 1: 0, 8: 1 } }))
            .mockResolvedValueOnce(dataEnvelope({ id: 7 }));
        axiosDelete.mockResolvedValueOnce(dataEnvelope({ id: 7, deleted: true }));

        const listed = await services.getImportConfigs({ fileFormat: 'csv' });
        const matched = await services.matchImportConfig({ fileFormat: 'csv', headers: ['date', 'amount'] });
        const suggested = await services.suggestImportConfig({ fileFormat: 'csv', headers: ['date', 'amount'] });
        const saved = await services.saveImportConfig(buildImportConfigSaveRequest('CSV'));
        const deleted = await services.deleteImportConfig({ id: 7 });

        expect(listed.data.result).toEqual([{ id: 7, name: 'CSV' }]);
        expect(matched.data.result).toEqual({ id: 7, name: 'CSV' });
        expect(suggested.data.result).toEqual({ columnMapping: { 1: 0, 8: 1 } });
        expect(saved.data.result).toEqual({ id: 7 });
        expect(deleted.data.result).toEqual({ id: 7, deleted: true });
    });
});
