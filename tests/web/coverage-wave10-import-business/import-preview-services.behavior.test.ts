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

function dataEnvelope<T>(data: T, success = true): Record<string, unknown> {
    return { status: 200, data: { success, data } };
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
        const scoped = await services.getImportLearningSuggestions({
            sessionId: 'session/2',
            previewUpdates: [{ id: 8 }],
            previewIds: [8, 9],
            actionScope: { selection_hash: 'hash' }
        });
        expect(axiosPost).toHaveBeenCalledWith('bills/import/v2/learning/session/2/suggestions', {
            preview_updates: [{ id: 8 }],
            previewIds: [8, 9],
            action_scope: { selection_hash: 'hash' }
        });
        expect(scoped.data.result).toEqual({ suggestions: [2] });
    });

    test('promotes the exact preview scope and preserves empty arrays as explicit user choices', async () => {
        axiosPost.mockResolvedValueOnce(dataEnvelope({ promoted: 0 }));
        const response = await services.promoteImportLearning({
            sessionId: 'learning-session',
            previewUpdates: [],
            previewIds: [],
            actionScope: { mode: 'selected' }
        });

        expect(axiosPost).toHaveBeenCalledWith('bills/import/v2/learning/learning-session/promote', {
            preview_updates: [],
            previewIds: [],
            action_scope: { mode: 'selected' }
        });
        expect(response.data.result).toEqual({ promoted: 0 });
    });
});

describe('import preview lifecycle service behavior', () => {
    test('normalizes update success from either the data field or the legacy success flag', async () => {
        axiosPut
            .mockResolvedValueOnce({ data: { success: true, data: { updated: false, previewItem: { id: 7 } } } })
            .mockResolvedValueOnce({ data: { success: true } });

        const explicit = await services.updateImportPreviewItem({
            sessionId: 'session/a',
            payload: { id: 7, amountCents: 1234 }
        });
        const fallback = await services.updateImportPreviewItem({
            sessionId: 'session/b',
            payload: { id: 8 }
        });

        expect(axiosPut).toHaveBeenNthCalledWith(1, 'bills/import/v2/preview/session%2Fa/update', {
            id: 7,
            amountCents: 1234
        });
        expect(explicit.data.result).toEqual({ updated: false, previewItem: { id: 7 } });
        expect(fallback.data.result).toEqual({ updated: true, previewItem: undefined });
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
        await services.confirmImportPreview({ sessionId: 's1' });
        expect(axiosPost).toHaveBeenNthCalledWith(1, 'bills/import/v2/confirm', {
            session_id: 's1',
            preserve_unpatched_selection: false,
            preview_updates: []
        }, expect.objectContaining({ timeout: expect.any(Number) }));

        const acknowledgement = { token: 'ack-token' };
        await services.confirmImportPreview({
            sessionId: 's2',
            previewUpdates: [{ id: 4 }],
            preserveUnpatchedSelection: true,
            historyRewriteAcknowledgement: acknowledgement
        });
        expect(axiosPost).toHaveBeenNthCalledWith(2, 'bills/import/v2/confirm', {
            session_id: 's2',
            preserve_unpatched_selection: true,
            preview_updates: [{ id: 4 }],
            history_rewrite_acknowledgement: acknowledgement
        }, expect.any(Object));
    });

    test('submits transfer review decisions with encoded ids and optional decision details', async () => {
        await services.reviewImportTransferDecision({
            previewId: 12,
            decision: 'accept',
            payload: { destination_account_id: 9 }
        });
        await services.reviewImportTransferDecision({ previewId: 13, decision: 'clear' });

        expect(axiosPost).toHaveBeenNthCalledWith(1, 'bills/import/v2/preview-item/12/transfer-decision', {
            decision: 'accept',
            destination_account_id: 9
        });
        expect(axiosPost).toHaveBeenNthCalledWith(2, 'bills/import/v2/preview-item/13/transfer-decision', {
            decision: 'clear'
        });
    });
});

describe('matching and import configuration service behavior', () => {
    test('uses encoded candidate and bill identities across read and review actions', async () => {
        await services.getMatchingSessionCandidates({ sessionId: 'session/a b' });
        await services.getMatchingBillCandidates({ billId: 'bill/7' });
        await services.getMatchingBillFeedback({ billId: 'bill/7' });
        await services.acceptMatchingCandidate({ candidateId: 'candidate/a', payload: { merge: true } });
        await services.rejectMatchingCandidate({ candidateId: 'candidate/b' });
        await services.clearMatchingCandidate({ candidateId: 'candidate/c' });
        await services.deleteMatchingPair({ pairId: 'pair/9' });

        expect(axiosGet).toHaveBeenNthCalledWith(1, 'matching/candidates?sessionId=session%2Fa%20b');
        expect(axiosGet).toHaveBeenNthCalledWith(2, 'matching/candidates?billId=bill%2F7');
        expect(axiosGet).toHaveBeenNthCalledWith(3, 'matching/bills/bill%2F7/feedback');
        expect(axiosPost).toHaveBeenNthCalledWith(1, 'matching/candidates/candidate%2Fa/accept', { merge: true });
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
        await services.saveImportConfig({ name: 'bank csv' });
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
        expect(axiosPost).toHaveBeenNthCalledWith(4, 'bills/import/configs', { name: 'bank csv' });
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
        const saved = await services.saveImportConfig({ name: 'CSV' });
        const deleted = await services.deleteImportConfig({ id: 7 });

        expect(listed.data.result).toEqual([{ id: 7, name: 'CSV' }]);
        expect(matched.data.result).toEqual({ id: 7, name: 'CSV' });
        expect(suggested.data.result).toEqual({ columnMapping: { 1: 0, 8: 1 } });
        expect(saved.data.result).toEqual({ id: 7 });
        expect(deleted.data.result).toEqual({ id: 7, deleted: true });
    });
});
