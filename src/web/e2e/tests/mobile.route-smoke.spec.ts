import { test } from '@playwright/test';

import { expectNoLoginRedirect, expectPageAnchor } from '../helpers/assertions';
import { getE2EEnvironment } from '../helpers/env';
import { mobileRoute, mobileSmokeRoutes } from '../helpers/routes';

test.describe('mobile route smoke', () => {
    const env = getE2EEnvironment();

    for (const target of mobileSmokeRoutes) {
        test(`mobile route renders ${target.name}`, async ({ page }) => {
            await page.goto(mobileRoute(target.path, env), { waitUntil: 'domcontentloaded' });

            await expectNoLoginRedirect(page, 'mobile.auth.login.page');
            await expectPageAnchor(page, target.testId);
        });
    }
});
