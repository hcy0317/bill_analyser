import { test } from '@playwright/test';

import { expectPageAnchor } from '../helpers/assertions';
import {
    createDomainCrudFixture,
    deleteDomainCrudFixture,
    expectDomainRulesVisibleInApi,
    expectEntityVisibleOnPage,
    type DomainCrudFixture,
    updateDomainCrudFixture
} from '../helpers/domainCrud';
import { getE2EEnvironment } from '../helpers/env';
import { desktopRoute } from '../helpers/routes';
import { cleanupE2ESession, createCleanE2ESession } from '../helpers/session';

test.describe('desktop domain CRUD linkage', () => {
    test.describe.configure({ retries: 0 });

    test('account/category/tag/budget/template/schedule/rule data is editable and visible on desktop pages', async ({ page, request }) => {
        const env = getE2EEnvironment();
        const session = await createCleanE2ESession(request, env);
        const client = session.client;
        let fixture: DomainCrudFixture | undefined;

        try {
            fixture = await createDomainCrudFixture(client, env);
            fixture = await updateDomainCrudFixture(client, fixture, env);

            await page.goto(desktopRoute('/account/list', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'desktop.accounts.page');
            await expectEntityVisibleOnPage(page, fixture.account.name);

            await page.goto(desktopRoute('/category/list', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'desktop.categories.page');
            await expectEntityVisibleOnPage(page, fixture.category.name);

            await page.goto(desktopRoute('/tag/list', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'desktop.tags.page');
            await expectEntityVisibleOnPage(page, fixture.tag.name);

            await page.goto(desktopRoute('/budget/list', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'desktop.budgets.page');
            await expectEntityVisibleOnPage(page, fixture.budget.name);

            await page.goto(desktopRoute('/template/list', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'desktop.templates.page');
            await expectEntityVisibleOnPage(page, fixture.template.name);

            await page.goto(desktopRoute('/schedule/list', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'desktop.schedules.page');
            await expectEntityVisibleOnPage(page, fixture.schedule.name);

            await page.goto(desktopRoute('/pairing/list?domain=transfer&tab=rules', env), { waitUntil: 'domcontentloaded' });
            await expectPageAnchor(page, 'desktop.rules.page');
            await expectDomainRulesVisibleInApi(client, fixture);
        } finally {
            if (fixture) {
                await deleteDomainCrudFixture(client, fixture);
            }
            await cleanupE2ESession(session);
        }
    });
});
