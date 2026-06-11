import { test } from '@playwright/test';

import { expectPageAnchor } from '../helpers/assertions';
import {
    createDomainCrudFixture,
    deleteDomainCrudFixture,
    expectDomainRulesVisibleInApi,
    expectEntityVisibleOnPage,
    type DomainCrudFixture
} from '../helpers/domainCrud';
import { getE2EEnvironment } from '../helpers/env';
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

            await page.goto(mobileRoute('/account/list', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'mobile.accounts.page');
            await expectEntityVisibleOnPage(page, fixture.account.name);

            await page.goto(mobileRoute('/category/list?type=3', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'mobile.categories.page');
            await expectEntityVisibleOnPage(page, fixture.category.name);

            await page.goto(mobileRoute('/tag/list', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'mobile.tags.page');
            await expectEntityVisibleOnPage(page, fixture.tag.name);

            await page.goto(mobileRoute('/budgets', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'mobile.budgets.page');
            await expectEntityVisibleOnPage(page, fixture.budget.name);

            await page.goto(mobileRoute('/template/list', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'mobile.templates.page');
            await expectEntityVisibleOnPage(page, fixture.template.name);

            await page.goto(mobileRoute('/schedule/list', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'mobile.schedules.page');
            await expectEntityVisibleOnPage(page, fixture.schedule.name);

            await page.goto(mobileRoute('/account/rules', env), { waitUntil: 'domcontentloaded' });
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
