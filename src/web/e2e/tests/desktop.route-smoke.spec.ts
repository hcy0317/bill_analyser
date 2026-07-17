import { test } from '@playwright/test';

import { expectNoLoginRedirect, expectPageAnchor } from '../helpers/assertions';
import { getE2EEnvironment } from '../helpers/env';
import {
    desktopRoute,
    desktopSmokeRoutes,
    ROUTE_SMOKE_PAGE_ANCHOR_TIMEOUT_MS,
    ROUTE_SMOKE_TEST_TIMEOUT_MS
} from '../helpers/routes';

test.describe('desktop route smoke', () => {
    const env = getE2EEnvironment();

    for (const target of desktopSmokeRoutes) {
        test(`desktop route renders ${target.name}`, async ({ page }) => {
            test.setTimeout(ROUTE_SMOKE_TEST_TIMEOUT_MS);
            await page.goto(desktopRoute(target.path, env), { waitUntil: 'commit' });

            await expectNoLoginRedirect(page, 'desktop.auth.login.page');
            await expectPageAnchor(page, target.testId, ROUTE_SMOKE_PAGE_ANCHOR_TIMEOUT_MS);
        });
    }
});
