import { test, type Page } from '@playwright/test';

import { expectPageAnchor } from '../helpers/assertions';
import {
    createDomainCrudFixture,
    deleteDomainCrudFixture,
    expectDomainRulesVisibleInApi,
    expectEntityVisibleOnPage,
    type DomainCrudFixture
} from '../helpers/domainCrud';
import { getE2EEnvironment, type E2EEnvironment } from '../helpers/env';
import { mobileRoute } from '../helpers/routes';
import { cleanupE2ESession, createCleanE2ESession } from '../helpers/session';

test.describe('mobile domain CRUD linkage', () => {
    test.describe.configure({ retries: 0 });

    test('created account/category/tag/budget/template/schedule/rule data is visible on mobile pages', async ({ page, request }) => {
        const env = getE2EEnvironment();
        const session = await createCleanE2ESession(request, env);
        const client = session.client;
        let fixture: DomainCrudFixture | undefined;

        try {
            fixture = await createDomainCrudFixture(client, env);

            await gotoFreshMobileRoute(page, '/account/list', env);
            await expectPageAnchor(page, 'mobile.accounts.page');
            await expectEntityVisibleOnPage(page, fixture.account.name);

            await gotoFreshMobileRoute(page, '/category/all', env);
            await expectPageAnchor(page, 'mobile.categories-all.page');
            await page.getByRole('link', { name: /Expense/u }).click();
            await expectPageAnchor(page, 'mobile.categories.page');
            await expectEntityVisibleOnPage(page, fixture.category.name);

            await gotoFreshMobileRoute(page, '/tag/list', env);
            await expectPageAnchor(page, 'mobile.tags.page');
            await expectEntityVisibleOnPage(page, fixture.tag.name);

            await gotoFreshMobileRoute(page, '/budgets', env);
            await expectPageAnchor(page, 'mobile.budgets.page');
            await expectEntityVisibleOnPage(page, fixture.category.name);

            await gotoFreshMobileRoute(page, '/template/list', env);
            await expectPageAnchor(page, 'mobile.templates.page');
            await expectEntityVisibleOnPage(page, fixture.template.name);

            await gotoFreshMobileRoute(page, '/schedule/list', env);
            await expectPageAnchor(page, 'mobile.schedules.page');
            await expectEntityVisibleOnPage(page, fixture.schedule.name);

            await gotoFreshMobileRoute(page, '/account/rules', env);
            await expectPageAnchor(page, 'mobile.account-rules.page');
            await expectDomainRulesVisibleInApi(client, fixture);
        } finally {
            if (fixture) {
                await deleteDomainCrudFixture(client, fixture);
            }
            await cleanupE2ESession(session);
        }
    });
});

async function gotoFreshMobileRoute(page: Page, path: string, env: E2EEnvironment): Promise<void> {
    await page.goto('about:blank');
    await page.goto(mobileRoute(path, env), { waitUntil: 'domcontentloaded' });
}
