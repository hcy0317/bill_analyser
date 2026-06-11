import { expect, test } from '@playwright/test';

import { expectPageAnchor } from '../helpers/assertions';
import { getE2EEnvironment } from '../helpers/env';
import {
    ALIPAY_SAMPLE_SOURCE_TOTALS,
    januaryStatisticsRoute,
    januaryTransactionListRoute,
    runAlipayImportFlow,
    summarizeObservedAmounts
} from '../helpers/importFlow';
import { cleanupE2ESession, createCleanE2ESession } from '../helpers/session';

test.describe('import preview confirm statistics linkage', () => {
    test.describe.configure({ retries: 0 });

    test('Alipay fixture import is visible in transaction list and January statistics', async ({ page, request }) => {
        const env = getE2EEnvironment();
        const session = await createCleanE2ESession(request, env);

        try {
            const result = await runAlipayImportFlow(session.client);

            test.info().annotations.push({
                type: 'alipay-source-totals',
                description: summarizeObservedAmounts(result)
            });

            await page.goto(januaryTransactionListRoute(env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'desktop.transactions.page');
            await expect(page.getByText(ALIPAY_SAMPLE_SOURCE_TOTALS.merchant, { exact: false }).first()).toBeVisible({
                timeout: 15_000
            });

            await page.goto(januaryStatisticsRoute(env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'desktop.statistics.page');
        } finally {
            await cleanupE2ESession(session);
        }
    });
});
