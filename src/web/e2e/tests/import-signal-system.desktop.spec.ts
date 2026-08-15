import { expect, test } from '@playwright/test';

import { E2EApiError } from '../helpers/apiClient';
import { cleanupE2ESession, createCleanE2ESession } from '../helpers/session';
import {
    createImportSignalFixtureContext,
    EXPECTED_SIGNAL_COUNTS,
    IMPORT_AUXILIARY_SIGNAL_LABELS,
    IMPORT_SIGNAL_FAMILIES,
    IMPORT_SIGNAL_LABELS,
    IMPORT_SIGNAL_MARKERS,
    IMPORT_SIGNAL_ROW_COUNT,
    openImportSignalDialog,
    openSignalFilterMenu,
    patchImportSignalPreview,
    patchImportSignalPreviewMissingCategory,
    previewRowsFromEnvelope,
    refreshPreviewThroughFilter,
    rowIncludesMarker,
    signalCountsFromEnvelope,
    type ImportSignalDedupResponse,
    type ImportSignalPreviewPage
} from '../helpers/importSignalSystem';

test.describe('desktop import signal system', () => {
    test.describe.configure({ mode: 'serial', retries: 0 });

    test('shows exactly six filters with SQL-backed rows, counts, full LLM payload, and stable paging', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        try {
            const fixtureContext = await createImportSignalFixtureContext(session.client, session.env);
            await openImportSignalDialog(page, session.env);

            const dedupResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().includes('/api/bills/import/v2/dedup')
            ));
            await page.getByTestId('desktop.import.action.next').click();
            const dedupResponse = await dedupResponsePromise;
            expect(dedupResponse.ok(), 'browser upload/dedup request').toBe(true);
            const dedupEnvelope = await dedupResponse.json() as Record<string, unknown>;
            const dedup = unwrapData<ImportSignalDedupResponse>(dedupEnvelope);
            expect(dedup.after_dedup, 'all deterministic fixture rows survive dedup').toBe(IMPORT_SIGNAL_ROW_COUNT);
            expect(dedup.session_id, 'dedup returns a session id').toBeTruthy();
            await expect(page.getByTestId('desktop.import.preview.table')).toBeVisible();

            patchImportSignalPreview(dedup.session_id, fixtureContext);

            const allPage = await session.client.get<ImportSignalPreviewPage>(
                `bills/import/v2/preview/${encodeURIComponent(dedup.session_id)}?page=1&page_size=50&sort_by=time&sort_direction=asc`
            );
            expect(allPage.total).toBe(IMPORT_SIGNAL_ROW_COUNT);
            expect(allPage.metadata?.counts?.signals).toEqual(EXPECTED_SIGNAL_COUNTS);
            expect(Object.keys(allPage.metadata?.counts?.signals || {}).sort()).toEqual([...IMPORT_SIGNAL_FAMILIES].sort());

            const fullPreview = await session.client.post<ImportSignalDedupResponse>('bills/import/v2/dedup', {
                session_id: dedup.session_id,
                include_preview: true
            });
            const llmFullRow = (fullPreview.preview || []).find(row => rowIncludesMarker(row, IMPORT_SIGNAL_MARKERS.llm));
            expect(llmFullRow, 'full preview includes the LLM action row').toBeTruthy();
            expect(llmFullRow?.['matching']).toMatchObject({
                llm: {
                    review_status: 'pending',
                    reason: 'deterministic LLM action suggestion'
                }
            });

            const signalMenu = await openSignalFilterMenu(page);
            const signalsGroup = signalMenu.getByRole('group', { name: 'Signals' });
            const visibleSignalItems = (await signalsGroup
                .getByRole('listitem')
                .allTextContents())
                .map(value => value.trim())
                .filter(Boolean);
            expect(visibleSignalItems).toEqual(['All', ...IMPORT_SIGNAL_LABELS]);
            for (const auxiliary of IMPORT_AUXILIARY_SIGNAL_LABELS) {
                await expect(signalMenu.getByText(auxiliary, { exact: true })).toHaveCount(0);
            }

            const expectedMarkers: Record<typeof IMPORT_SIGNAL_FAMILIES[number], readonly string[]> = {
                parser: [
                    IMPORT_SIGNAL_MARKERS.parser,
                    'E2E-SIG-PAGINATION-A',
                    'E2E-SIG-PAGINATION-B',
                    'E2E-SIG-PAGINATION-C',
                    'E2E-SIG-PAGINATION-D'
                ],
                platform_duplicate: [IMPORT_SIGNAL_MARKERS.platform_duplicate],
                transfer: [
                    IMPORT_SIGNAL_MARKERS.transfer,
                    IMPORT_SIGNAL_MARKERS.transfer_terminal
                ],
                history: [IMPORT_SIGNAL_MARKERS.history],
                learning: [IMPORT_SIGNAL_MARKERS.learning],
                llm: [IMPORT_SIGNAL_MARKERS.llm, IMPORT_SIGNAL_MARKERS.llm_edit]
            };
            for (let index = 0; index < IMPORT_SIGNAL_FAMILIES.length; index += 1) {
                const family = IMPORT_SIGNAL_FAMILIES[index];
                const label = IMPORT_SIGNAL_LABELS[index];
                if (!family || !label) {
                    throw new Error(`Missing signal family/label at index ${index}.`);
                }
                const envelope = await refreshPreviewThroughFilter(page, label);
                const rows = previewRowsFromEnvelope(envelope);
                const markers = expectedMarkers[family];
                expect(rows, `${family} SQL filter row count`).toHaveLength(markers.length);
                for (const marker of markers) {
                    expect(rows.some(row => rowIncludesMarker(row, marker)), `${family} includes ${marker}`).toBe(true);
                }
                const counts = signalCountsFromEnvelope(envelope);
                expect(Object.keys(counts).sort(), `${family} response count keys`).toEqual([...IMPORT_SIGNAL_FAMILIES].sort());
                const primaryMarker = markers[0];
                if (!primaryMarker) {
                    throw new Error(`No expected marker configured for ${family}.`);
                }
                await expect(page.getByTestId('desktop.import.preview.table')).toContainText(primaryMarker);
            }

            const transferEnvelope = await refreshPreviewThroughFilter(page, 'Transfer Match');
            const transferRows = previewRowsFromEnvelope(transferEnvelope);
            expect(transferRows.some(row => rowIncludesMarker(row, IMPORT_SIGNAL_MARKERS.transfer_terminal))).toBe(true);

            const platformEnvelope = await refreshPreviewThroughFilter(page, 'Platform Duplicate');
            const platformRow = previewRowsFromEnvelope(platformEnvelope)[0];
            expect(platformRow?.['dedup_type']).toBe('platform_bank');
            expect(platformRow?.['preview_matching_feedback']).toEqual({});

            const pageOne = await previewPage(session.client, dedup.session_id, 1, 5);
            const pageTwo = await previewPage(session.client, dedup.session_id, 2, 5);
            const pageThree = await previewPage(session.client, dedup.session_id, 3, 5);
            const pagedRows = [
                ...(pageOne.preview || []),
                ...(pageTwo.preview || []),
                ...(pageThree.preview || [])
            ];
            expect(pagedRows).toHaveLength(IMPORT_SIGNAL_ROW_COUNT);
            const pagedIds = pagedRows.map(row => Number(row['id']));
            expect(new Set(pagedIds).size, 'pagination has no duplicate ids').toBe(IMPORT_SIGNAL_ROW_COUNT);
            const pagedTimes = pagedRows.map(row => String(row['preview_date']));
            expect(pagedTimes).toEqual([...pagedTimes].sort());
        } finally {
            await cleanupE2ESession(session);
        }
    });

    test('rejects a pending learning candidate and persists the terminal state', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        try {
            const { sessionId } = await stageDesktopSignalFixture(page, session);
            const learningEnvelope = await refreshPreviewThroughFilter(page, 'Learning Suggestion');
            const learningRow = previewRowsFromEnvelope(learningEnvelope)
                .find(row => rowIncludesMarker(row, IMPORT_SIGNAL_MARKERS.learning));
            const learningPreviewId = Number(learningRow?.['id']);
            expect(learningPreviewId).toBeGreaterThan(0);
            expect(learningRow?.['matching'], 'paginated learning projection reaches the browser response').toMatchObject({
                learning: {
                    review_status: 'pending',
                    lifecycle_status: 'pending',
                    signal_state: 'pending',
                    score: 0.91
                }
            });
            const table = page.getByTestId('desktop.import.preview.table');
            const visibleLearningRow = table.getByRole('row').filter({ hasText: IMPORT_SIGNAL_MARKERS.learning });
            await expect(visibleLearningRow, 'frontend reconciles paginated learning payload').toContainText('Learning Suggestion', {
                timeout: 5_000
            });

            const rejectResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().includes('/api/matching/candidates/')
                && response.url().endsWith('/reject')
            ));
            await visibleLearningRow.getByRole('button', { name: 'Reject', exact: true }).click();
            expect((await rejectResponsePromise).ok(), 'existing matching-candidate reject route').toBe(true);
            await expect(visibleLearningRow.getByText('Rejected', { exact: true })).toBeVisible();

            const persisted = await previewPage(session.client, sessionId, 1, 50);
            const persistedLearning = (persisted.preview || [])
                .find(row => rowIncludesMarker(row, IMPORT_SIGNAL_MARKERS.learning));
            expect(persistedLearning?.['matching']).toMatchObject({
                learning: { review_status: 'rejected' }
            });
        } finally {
            await cleanupE2ESession(session);
        }
    });

    test('clears pending learning and LLM candidates without reopening terminal decisions', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        try {
            const { sessionId } = await stageDesktopSignalFixture(page, session);
            const learningEnvelope = await refreshPreviewThroughFilter(page, 'Learning Suggestion');
            const learningRow = previewRowsFromEnvelope(learningEnvelope)
                .find(row => rowIncludesMarker(row, IMPORT_SIGNAL_MARKERS.learning));
            const learningPreviewId = Number(learningRow?.['id']);
            expect(learningPreviewId).toBeGreaterThan(0);

            await session.client.post<Record<string, unknown>>(
                `matching/candidates/${encodeURIComponent(`preview:${learningPreviewId}:learning`)}/clear`,
                { sessionId, responseMode: 'preview-item' }
            );

            const llmEnvelope = await refreshPreviewThroughFilter(page, 'LLM Suggestion');
            const llmRows = previewRowsFromEnvelope(llmEnvelope);
            expect(llmRows).toHaveLength(2);
            for (const row of llmRows) {
                const previewId = Number(row['id']);
                expect(previewId).toBeGreaterThan(0);
                await session.client.post<Record<string, unknown>>(
                    `matching/candidates/${encodeURIComponent(`preview:${previewId}:llm`)}/clear`,
                    { sessionId, responseMode: 'preview-item' }
                );
            }

            const persisted = await previewPage(session.client, sessionId, 1, 50);
            const clearedLearning = (persisted.preview || [])
                .find(row => rowIncludesMarker(row, IMPORT_SIGNAL_MARKERS.learning));
            expect(clearedLearning?.['preview_matching_feedback']).not.toHaveProperty('learning');
            for (const marker of [IMPORT_SIGNAL_MARKERS.llm, IMPORT_SIGNAL_MARKERS.llm_edit]) {
                const clearedLlm = (persisted.preview || []).find(row => rowIncludesMarker(row, marker));
                expect(clearedLlm?.['preview_matching_feedback']).not.toHaveProperty('llm');
            }
            const filtered = await session.client.get<ImportSignalPreviewPage>(
                `bills/import/v2/preview/${encodeURIComponent(sessionId)}?page=1&page_size=50&signal=learning`
            );
            expect(filtered.total).toBe(0);
            const llmFiltered = await session.client.get<ImportSignalPreviewPage>(
                `bills/import/v2/preview/${encodeURIComponent(sessionId)}?page=1&page_size=50&signal=llm`
            );
            expect(llmFiltered.total).toBe(0);

            await refreshPreviewThroughFilter(page, 'Parser');
            const refreshedLlmEnvelope = await refreshPreviewThroughFilter(page, 'LLM Suggestion');
            expect(previewRowsFromEnvelope(refreshedLlmEnvelope)).toHaveLength(0);
            const table = page.getByTestId('desktop.import.preview.table');
            for (const marker of [IMPORT_SIGNAL_MARKERS.llm, IMPORT_SIGNAL_MARKERS.llm_edit]) {
                await expect(table.getByRole('row').filter({ hasText: marker })).toHaveCount(0);
            }
            await expect(table.getByText('LLM Suggestion', { exact: true })).toHaveCount(0);

            const refreshedLearningEnvelope = await refreshPreviewThroughFilter(page, 'Learning Suggestion');
            expect(previewRowsFromEnvelope(refreshedLearningEnvelope)).toHaveLength(0);
            await expect(table.getByRole('row').filter({ hasText: IMPORT_SIGNAL_MARKERS.learning })).toHaveCount(0);
            await expect(table.getByText('Learning Suggestion', { exact: true })).toHaveCount(0);
        } finally {
            await cleanupE2ESession(session);
        }
    });

    test('accepts and rejects independent LLM suggestions through visible row actions', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        try {
            const { sessionId } = await stageDesktopSignalFixture(page, session);
            const llmEnvelope = await refreshPreviewThroughFilter(page, 'LLM Suggestion');
            expect(previewRowsFromEnvelope(llmEnvelope)).toHaveLength(2);
            const table = page.getByTestId('desktop.import.preview.table');
            const acceptedRow = table.getByRole('row').filter({ hasText: IMPORT_SIGNAL_MARKERS.llm });
            const rejectedRow = table.getByRole('row').filter({ hasText: IMPORT_SIGNAL_MARKERS.llm_edit });

            const acceptResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().endsWith('/api/llm/preview-recommend/accept')
            ));
            await acceptedRow.getByRole('button', { name: 'Accept', exact: true }).click();
            expect((await acceptResponsePromise).ok(), 'LLM accept route').toBe(true);
            await expect(acceptedRow.getByText('Accepted', { exact: true })).toBeVisible();

            const rejectResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().endsWith('/api/llm/preview-recommend/reject')
            ));
            await rejectedRow.getByRole('button', { name: 'Reject', exact: true }).click();
            expect((await rejectResponsePromise).ok(), 'LLM reject route').toBe(true);
            await expect(rejectedRow.getByText('Rejected', { exact: true })).toBeVisible();

            const persisted = await previewPage(session.client, sessionId, 1, 50);
            const accepted = (persisted.preview || [])
                .find(row => rowIncludesMarker(row, IMPORT_SIGNAL_MARKERS.llm));
            const rejected = (persisted.preview || [])
                .find(row => rowIncludesMarker(row, IMPORT_SIGNAL_MARKERS.llm_edit));
            expect(accepted?.['matching']).toMatchObject({ llm: { review_status: 'accepted' } });
            expect(rejected?.['matching']).toMatchObject({ llm: { review_status: 'rejected' } });
        } finally {
            await cleanupE2ESession(session);
        }
    });

    test('keeps server LLM tombstones authoritative after a real page reload', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        try {
            const { sessionId } = await stageDesktopSignalFixture(page, session);
            const initial = await previewPage(session.client, sessionId, 1, 50);
            const noOpSource = (initial.preview || [])
                .find(row => rowIncludesMarker(row, IMPORT_SIGNAL_MARKERS.llm_edit));
            const reclassifySource = (initial.preview || [])
                .find(row => rowIncludesMarker(row, IMPORT_SIGNAL_MARKERS.llm));
            const noOpId = Number(noOpSource?.['id']);
            const reclassifyId = Number(reclassifySource?.['id']);
            expect(noOpId).toBeGreaterThan(0);
            expect(reclassifyId).toBeGreaterThan(0);

            await refreshPreviewThroughFilter(page, 'LLM Suggestion');
            const table = page.getByTestId('desktop.import.preview.table');
            const acceptedRow = table.getByRole('row').filter({ hasText: IMPORT_SIGNAL_MARKERS.llm_edit });
            const rejectedRow = table.getByRole('row').filter({ hasText: IMPORT_SIGNAL_MARKERS.llm });

            const acceptResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().endsWith('/api/llm/preview-recommend/accept')
            ));
            await acceptedRow.getByRole('button', { name: 'Accept', exact: true }).click();
            expect((await acceptResponsePromise).ok(), 'LLM accept creates terminal memory').toBe(true);
            await expect(acceptedRow.getByText('Accepted', { exact: true })).toBeVisible();

            const rejectResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().endsWith('/api/llm/preview-recommend/reject')
            ));
            await rejectedRow.getByRole('button', { name: 'Reject', exact: true }).click();
            expect((await rejectResponsePromise).ok(), 'LLM reject creates terminal memory').toBe(true);
            await expect(rejectedRow.getByText('Rejected', { exact: true })).toBeVisible();

            const noOp = await session.client.put<{ previewItem?: Record<string, unknown> }>(
                `bills/import/v2/preview/${encodeURIComponent(sessionId)}/update`,
                {
                    id: noOpId,
                    counterparty: noOpSource?.['preview_counterparty'],
                    responseMode: 'preview-item'
                }
            );
            expect(noOp.previewItem?.['matching']).toMatchObject({
                llm: { review_status: 'accepted' }
            });

            const material = await session.client.put<{ previewItem?: Record<string, unknown> }>(
                `bills/import/v2/preview/${encodeURIComponent(sessionId)}/update`,
                {
                    id: noOpId,
                    counterparty: `${String(noOpSource?.['preview_counterparty'] || IMPORT_SIGNAL_MARKERS.llm_edit)} material edit`,
                    clear_llm_decision: true,
                    clear_actionable_suggestions: ['llm'],
                    responseMode: 'preview-item'
                }
            );
            expect(material.previewItem?.['preview_matching_feedback']).not.toHaveProperty('llm');

            const reclassified = await session.client.post<{ preview?: Array<Record<string, unknown>> }>(
                `bills/import/v2/reclassify/${encodeURIComponent(sessionId)}`,
                {
                    preview_updates: [{ id: reclassifyId, selected: true }]
                }
            );
            const reclassifiedRow = (reclassified.preview || [])
                .find(row => Number(row['id']) === reclassifyId);
            expect(reclassifiedRow?.['preview_matching_feedback']).not.toHaveProperty('llm');

            const persisted = await previewPage(session.client, sessionId, 1, 50);
            const materialPersisted = (persisted.preview || []).find(row => Number(row['id']) === noOpId);
            const reclassifiedPersisted = (persisted.preview || []).find(row => Number(row['id']) === reclassifyId);
            for (const serverTombstone of [materialPersisted, reclassifiedPersisted]) {
                expect(serverTombstone?.['preview_matching_feedback']).not.toHaveProperty('llm');
                expect(serverTombstone?.['matching']).toMatchObject({
                    llm: {
                        review_status: '',
                        confidence: 0,
                        reason: '',
                        suggested_type: '',
                        suggested_main_category: ''
                    }
                });
            }

            const memoryEvents = await session.client.get<Array<Record<string, unknown>>>(
                `llm/memory?session_id=${encodeURIComponent(sessionId)}&limit=500`
            );
            expect(memoryEvents.some(event => Number(event['preview_id']) === noOpId && event['decision'] === 'accept')).toBe(true);
            expect(memoryEvents.some(event => Number(event['preview_id']) === reclassifyId && event['decision'] === 'reject')).toBe(true);

            const llmFiltered = await session.client.get<ImportSignalPreviewPage>(
                `bills/import/v2/preview/${encodeURIComponent(sessionId)}?page=1&page_size=50&signal=llm`
            );
            expect(llmFiltered.total).toBe(0);

            await reopenDesktopSignalSessionAfterReload(page, session, sessionId);
            const reloadedTable = page.getByTestId('desktop.import.preview.table');
            for (const marker of [IMPORT_SIGNAL_MARKERS.llm, IMPORT_SIGNAL_MARKERS.llm_edit]) {
                const serverRow = reloadedTable.getByRole('row').filter({ hasText: marker });
                await expect(serverRow).toHaveCount(1);
                await expect(serverRow.getByText('Accepted', { exact: true })).toHaveCount(0);
                await expect(serverRow.getByText('Rejected', { exact: true })).toHaveCount(0);
                await expect(serverRow.getByText('LLM Suggestion', { exact: true })).toHaveCount(0);
            }

            const refreshedLlmEnvelope = await refreshPreviewThroughFilter(page, 'LLM Suggestion');
            expect(previewRowsFromEnvelope(refreshedLlmEnvelope)).toHaveLength(0);
            for (const marker of [IMPORT_SIGNAL_MARKERS.llm, IMPORT_SIGNAL_MARKERS.llm_edit]) {
                await expect(reloadedTable.getByRole('row').filter({ hasText: marker })).toHaveCount(0);
            }
            await expect(reloadedTable.getByText('Accepted', { exact: true })).toHaveCount(0);
            await expect(reloadedTable.getByText('Rejected', { exact: true })).toHaveCount(0);
            await expect(reloadedTable.getByText('LLM Suggestion', { exact: true })).toHaveCount(0);
        } finally {
            await cleanupE2ESession(session);
        }
    });

    test('keeps a resolved missing category cleared across acknowledgement, paging, reload, and re-entry', async ({ page, request }) => {
        test.setTimeout(60_000);
        const session = await createCleanE2ESession(request);
        try {
            const { sessionId, context } = await stageDesktopSignalFixture(page, session);
            patchImportSignalPreviewMissingCategory(sessionId);

            const parserEnvelope = await refreshPreviewThroughFilter(page, 'Parser');
            const missingCategoryRow = previewRowsFromEnvelope(parserEnvelope)
                .find(row => rowIncludesMarker(row, IMPORT_SIGNAL_MARKERS.parser));
            const previewId = Number(missingCategoryRow?.['id']);
            expect(previewId).toBeGreaterThan(0);

            const table = page.getByTestId('desktop.import.preview.table');
            const row = table.getByRole('row').filter({ hasText: IMPORT_SIGNAL_MARKERS.parser });
            await expect(row.getByTitle('Missing Category')).toBeVisible();

            const editButton = row.getByRole('cell').nth(1).getByRole('button');
            await editButton.click();
            const editingRow = table.getByRole('row').filter({ has: page.getByRole('combobox') });
            await expect(editingRow).toHaveCount(1);
            const categorySelect = editingRow.getByRole('cell').nth(5).getByRole('combobox').first();
            await categorySelect.click();
            const categoryMenu = page.locator('.two-column-select-menu');
            await categoryMenu.getByText(context.expensePrimaryCategory.name, { exact: true }).click();
            await categoryMenu.getByText(context.expenseCategory.name, { exact: true }).click();

            await expect(editingRow.getByTitle('Missing Category')).toHaveCount(0);
            await editingRow.getByRole('cell').nth(1).getByRole('button').click();

            const acknowledged = await session.client.put<{ previewItem?: Record<string, unknown> }>(
                `bills/import/v2/preview/${encodeURIComponent(sessionId)}/update`,
                {
                    id: previewId,
                    categoryId: Number(context.expenseCategory.id),
                    responseMode: 'preview-item'
                }
            );
            expect(Number(acknowledged.previewItem?.['category_id'])).toBe(Number(context.expenseCategory.id));
            expect(JSON.stringify(acknowledged.previewItem?.['preview_matching_feedback'] || {}))
                .not.toContain('missing_category');

            const signalMenu = await openSignalFilterMenu(page);
            const allResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'GET'
                && response.url().includes(`/api/bills/import/v2/preview/${encodeURIComponent(sessionId)}`)
                && !response.url().includes('signal=')
            ));
            await signalMenu.getByRole('group', { name: 'Signals' }).getByText('All', { exact: true }).click();
            expect((await allResponsePromise).ok(), 'all rows reload after category acknowledgement').toBe(true);
            await expect(row).toContainText(context.expenseCategory.name);
            await expect(row.getByTitle('Missing Category')).toHaveCount(0);

            const nextPageResponse = page.waitForResponse(response => (
                response.request().method() === 'GET'
                && response.url().includes(`/api/bills/import/v2/preview/${encodeURIComponent(sessionId)}`)
                && response.url().includes('page=2')
            ));
            await table.getByRole('button', { name: '2', exact: true }).click();
            expect((await nextPageResponse).ok(), 'server-paged preview reaches page two').toBe(true);
            const previousPageResponse = page.waitForResponse(response => (
                response.request().method() === 'GET'
                && response.url().includes(`/api/bills/import/v2/preview/${encodeURIComponent(sessionId)}`)
                && response.url().includes('page=1')
            ));
            await table.getByRole('button', { name: '1', exact: true }).click();
            expect((await previousPageResponse).ok(), 'server-paged preview returns to page one').toBe(true);
            await expect(row).toContainText(context.expenseCategory.name);
            await expect(row.getByTitle('Missing Category')).toHaveCount(0);

            await reopenDesktopSignalSessionAfterReload(page, session, sessionId);
            const reloadedRow = page.getByTestId('desktop.import.preview.table')
                .getByRole('row')
                .filter({ hasText: IMPORT_SIGNAL_MARKERS.parser });
            await expect(reloadedRow).toContainText(context.expenseCategory.name);
            await expect(reloadedRow.getByTitle('Missing Category')).toHaveCount(0);
        } finally {
            await cleanupE2ESession(session);
        }
    });

    test('accepts a learning candidate, then confirms with replay and conflict outcomes', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        try {
            const { sessionId } = await stageDesktopSignalFixture(page, session);
            await refreshPreviewThroughFilter(page, 'Learning Suggestion');
            const signalMenu = await openSignalFilterMenu(page);
            const signalsGroup = signalMenu.getByRole('group', { name: 'Signals' });
            const allResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'GET'
                && response.url().includes(`/api/bills/import/v2/preview/${encodeURIComponent(sessionId)}`)
                && !response.url().includes('signal=')
            ));
            await signalsGroup.getByText('All', { exact: true }).click();
            expect((await allResponsePromise).ok(), 'reset signal filter before lifecycle and confirm').toBe(true);
            const table = page.getByTestId('desktop.import.preview.table');
            const learningRow = table.getByRole('row').filter({ hasText: IMPORT_SIGNAL_MARKERS.learning });

            const acceptResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().includes('/api/matching/candidates/')
                && response.url().endsWith('/accept')
            ));
            await learningRow.getByRole('button', { name: 'Accept', exact: true }).click();
            expect((await acceptResponsePromise).ok(), 'existing matching-candidate accept route').toBe(true);
            await expect(learningRow.getByText('Accepted', { exact: true })).toBeVisible();
            await expect(page.getByTestId('desktop.import.action.confirm')).toBeEnabled();

            const confirmRequestPromise = page.waitForRequest(requestValue => (
                requestValue.method() === 'POST'
                && requestValue.url().includes('/api/bills/import/v2/confirm')
            ));
            const confirmResponsePromise = page.waitForResponse(response => (
                response.request().method() === 'POST'
                && response.url().includes('/api/bills/import/v2/confirm')
            ));
            await page.getByTestId('desktop.import.action.confirm').click();
            const confirmDialog = page.getByRole('dialog').filter({
                hasText: 'Are you sure you want to import'
            });
            await expect(confirmDialog).toBeVisible();
            await confirmDialog.getByRole('button', { name: 'OK', exact: true }).click();
            const confirmRequest = await confirmRequestPromise;
            const confirmResponse = await confirmResponsePromise;
            expect(confirmResponse.ok(), 'first user-visible confirm').toBe(true);
            await expect(page.getByRole('heading', { name: 'Data Import Completed' })).toBeVisible();
            const confirmPayload = confirmRequest.postDataJSON() as Record<string, unknown>;
            const firstConfirmData = unwrapData<Record<string, unknown>>(
                await confirmResponse.json() as Record<string, unknown>
            );

            const replay = await session.client.post<Record<string, unknown>>('bills/import/v2/confirm', confirmPayload);
            expect(replay, 'same fingerprint returns the durable stored success envelope').toEqual(firstConfirmData);

            const conflictingPayload = {
                ...confirmPayload,
                preserve_unpatched_selection: !Boolean(confirmPayload['preserve_unpatched_selection'])
            };
            let conflict: unknown;
            try {
                await session.client.post<Record<string, unknown>>('bills/import/v2/confirm', conflictingPayload);
            } catch (error) {
                conflict = error;
            }
            expect(conflict).toBeInstanceOf(E2EApiError);
            expect((conflict as E2EApiError).status).toBe(409);
        } finally {
            await cleanupE2ESession(session);
        }
    });
});

async function stageDesktopSignalFixture(
    page: Parameters<typeof openImportSignalDialog>[0],
    session: Awaited<ReturnType<typeof createCleanE2ESession>>
): Promise<{ sessionId: string; context: Awaited<ReturnType<typeof createImportSignalFixtureContext>> }> {
    const fixtureContext = await createImportSignalFixtureContext(session.client, session.env);
    await openImportSignalDialog(page, session.env);
    const dedupResponsePromise = page.waitForResponse(response => (
        response.request().method() === 'POST'
        && response.url().includes('/api/bills/import/v2/dedup')
    ));
    await page.getByTestId('desktop.import.action.next').click();
    const dedupResponse = await dedupResponsePromise;
    expect(dedupResponse.ok()).toBe(true);
    const dedup = unwrapData<ImportSignalDedupResponse>(
        await dedupResponse.json() as Record<string, unknown>
    );
    expect(dedup.after_dedup).toBe(IMPORT_SIGNAL_ROW_COUNT);
    await expect(page.getByTestId('desktop.import.preview.table')).toBeVisible();
    patchImportSignalPreview(dedup.session_id, fixtureContext);
    return { sessionId: dedup.session_id, context: fixtureContext };
}

async function previewPage(
    client: Awaited<ReturnType<typeof createCleanE2ESession>>['client'],
    sessionId: string,
    page: number,
    pageSize: number
): Promise<ImportSignalPreviewPage> {
    return client.get<ImportSignalPreviewPage>(
        `bills/import/v2/preview/${encodeURIComponent(sessionId)}?page=${page}&page_size=${pageSize}&sort_by=time&sort_direction=asc`
    );
}

async function reopenDesktopSignalSessionAfterReload(
    page: Parameters<typeof openImportSignalDialog>[0],
    session: Awaited<ReturnType<typeof createCleanE2ESession>>,
    sessionId: string
): Promise<void> {
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.route('**/api/bills/import/v2/parse', route => route.fulfill({
        json: {
            success: true,
            data: {
                session_id: sessionId,
                parsed_count: IMPORT_SIGNAL_ROW_COUNT,
                unmatched_files: []
            }
        }
    }), { times: 1 });
    await page.route('**/api/bills/import/v2/dedup', route => route.fulfill({
        json: {
            success: true,
            data: {
                session_id: sessionId,
                after_dedup: IMPORT_SIGNAL_ROW_COUNT,
                preview_count: IMPORT_SIGNAL_ROW_COUNT,
                dedup_stats: {}
            }
        }
    }), { times: 1 });

    await openImportSignalDialog(page, session.env);
    const previewResponsePromise = page.waitForResponse(response => (
        response.request().method() === 'GET'
        && response.url().includes(`/api/bills/import/v2/preview/${encodeURIComponent(sessionId)}`)
        && !response.url().includes('signal=')
    ));
    const memoryResponsePromise = page.waitForResponse(response => (
        response.request().method() === 'GET'
        && response.url().includes('/api/llm/memory')
        && response.url().includes(`session_id=${encodeURIComponent(sessionId)}`)
    ));
    await page.getByTestId('desktop.import.action.next').click();
    expect((await previewResponsePromise).ok(), 'fresh component mount reloads canonical preview').toBe(true);
    expect((await memoryResponsePromise).ok(), 'fresh component mount refreshes old LLM memory').toBe(true);
    await expect(page.getByTestId('desktop.import.preview.table')).toBeVisible();
}

function unwrapData<T>(envelope: Record<string, unknown>): T {
    const data = envelope['data'];
    if (!data || typeof data !== 'object') {
        throw new Error('Expected an API data envelope.');
    }
    return data as T;
}
