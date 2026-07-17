import { test } from '@playwright/test';

import { expectNoLoginRedirect, expectPageAnchor } from '../helpers/assertions';
import { getE2EEnvironment } from '../helpers/env';
import {
    mobileRoute,
    mobileSmokeRoutes,
    ROUTE_SMOKE_PAGE_ANCHOR_TIMEOUT_MS,
    ROUTE_SMOKE_TEST_TIMEOUT_MS
} from '../helpers/routes';

test.describe('mobile route smoke', () => {
    const env = getE2EEnvironment();

    for (const target of mobileSmokeRoutes) {
        test(`mobile route renders ${target.name}`, async ({ page }) => {
            test.setTimeout(ROUTE_SMOKE_TEST_TIMEOUT_MS);
            await page.goto(mobileRoute(target.path, env), { waitUntil: 'commit' });

            await expectNoLoginRedirect(page, 'mobile.auth.login.page');
            await expectPageAnchor(page, target.testId, ROUTE_SMOKE_PAGE_ANCHOR_TIMEOUT_MS);
        });
    }
});
