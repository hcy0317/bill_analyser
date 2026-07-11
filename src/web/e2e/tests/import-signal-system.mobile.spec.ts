import { expect, test } from '@playwright/test';

import {
    createImportSignalFixtureContext,
    IMPORT_AUXILIARY_SIGNAL_LABELS,
    IMPORT_SIGNAL_LABELS,
    IMPORT_SIGNAL_MARKERS,
    IMPORT_SIGNAL_ROW_COUNT,
    patchImportSignalPreview,
    stageImportSignalFixtureViaApi
} from '../helpers/importSignalSystem';
import { mobileRoute } from '../helpers/routes';
import { cleanupE2ESession, createCleanE2ESession } from '../helpers/session';

test.describe('mobile import signal system', () => {
    test.describe.configure({ retries: 0 });

    test('renders the isolated preview with six canonical filters and actionable signals', async ({ page, request }) => {
        const session = await createCleanE2ESession(request);
        try {
            const fixtureContext = await createImportSignalFixtureContext(session.client, session.env);
            const staged = await stageImportSignalFixtureViaApi(session.client);
            patchImportSignalPreview(staged.session_id, fixtureContext);

            await page.goto(mobileRoute(
                `/transaction/import/preview?sessionId=${encodeURIComponent(staged.session_id)}`,
                session.env
            ), { waitUntil: 'domcontentloaded' });

            await expect(page.getByTestId('mobile.import.preview.page')).toBeVisible();
            await expect(page.getByTestId('mobile.import.preview.summary')).toContainText(String(IMPORT_SIGNAL_ROW_COUNT));
            await expect(page.getByText(IMPORT_SIGNAL_MARKERS.learning, { exact: true })).toBeVisible();
            await expect(page.getByText(IMPORT_SIGNAL_MARKERS.llm, { exact: true })).toBeVisible();
            await expect(page.getByText('Learning Suggestion', { exact: true }).first()).toBeVisible();
            await expect(page.getByText('LLM Suggestion', { exact: true }).first()).toBeVisible();
            await expect(page.getByTestId('mobile.import.preview.action.confirm')).not.toHaveClass(/disabled/u);

            await page.locator('.navbar .right a').click();
            const filterSheet = page.locator('.import-preview-filter-sheet');
            await expect(filterSheet).toBeVisible();
            for (const label of ['All', ...IMPORT_SIGNAL_LABELS]) {
                await expect(filterSheet.getByText(label, { exact: true })).toBeVisible();
            }
            for (const auxiliary of IMPORT_AUXILIARY_SIGNAL_LABELS) {
                await expect(filterSheet.getByText(auxiliary, { exact: true })).toHaveCount(0);
            }
        } finally {
            await cleanupE2ESession(session);
        }
    });
});
