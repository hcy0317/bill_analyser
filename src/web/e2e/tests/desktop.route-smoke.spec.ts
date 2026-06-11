import { test } from '@playwright/test';

import { expectNoLoginRedirect, expectPageAnchor } from '../helpers/assertions';
import { getE2EEnvironment } from '../helpers/env';
import { desktopRoute, desktopSmokeRoutes } from '../helpers/routes';

test.describe('desktop route smoke', () => {
    const env = getE2EEnvironment();

    for (const target of desktopSmokeRoutes) {
        test(`desktop route renders ${target.name}`, async ({ page }) => {
            await page.goto(desktopRoute(target.path, env), { waitUntil: 'domcontentloaded' });

            await expectNoLoginRedirect(page, 'desktop.auth.login.page');
            await expectPageAnchor(page, target.testId);
        });
    }
});
