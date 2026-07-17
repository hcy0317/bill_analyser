import { expect, type Page } from '@playwright/test';

import type { E2EEnvironment } from './env';
import { E2EApiClient } from './apiClient';

interface Entity {
    readonly id: string | number;
    readonly name: string;
}

export interface DomainCrudFixture {
    readonly account: Entity;
    readonly category: Entity;
    readonly tag: Entity;
    readonly budget: Entity;
    readonly template: Entity;
    readonly schedule: Entity;
    readonly categoryRule: Entity;
    readonly accountRule: Entity;
}

export async function createDomainCrudFixture(client: E2EApiClient, env: E2EEnvironment): Promise<DomainCrudFixture> {
    const suffix = env.runId;
    const budgetPeriod = currentMonthDateRange();
    const account = await client.post<Entity>('accounts', {
        name: `E2E Cash ${suffix}`,
        category: 1,
        type: 1,
        icon: '1',
        color: '#4c6ef5',
        currency: 'USD',
        balanceCents: 0,
        balanceTime: 0,
        comment: `created by ${suffix}`,
        clientSessionId: suffix
    });
    const category = await client.post<Entity>('categories', {
        name: `E2E Category ${suffix}`,
        type: 3,
        parentId: '0',
        icon: '1',
        color: '#e03131',
        comment: `created by ${suffix}`,
        displayOrder: 0,
        ruleExpression: '',
        clientSessionId: suffix
    });
    const tag = await client.post<Entity>('tags', {
        name: `E2E Tag ${suffix}`
    });
    const budgetName = `E2E Budget ${suffix}`;
    const budget = entityFromResponse(await client.post<Record<string, unknown>>('budgets/', {
        name: budgetName,
        category: category.name,
        sub_category: '',
        period_type: 'monthly',
        amountCents: 12345,
        start_date: budgetPeriod.startDate,
        end_date: budgetPeriod.endDate,
        alert_threshold: 80,
        enabled: true
    }), budgetName);
    const template = await client.post<Entity>('templates', buildTemplatePayload({
        name: `E2E Template ${suffix}`,
        templateType: 1,
        accountId: String(account.id),
        categoryId: String(category.id),
        tagId: String(tag.id),
        clientSessionId: suffix
    }));
    const schedule = await client.post<Entity>('templates', {
        ...buildTemplatePayload({
            name: `E2E Schedule ${suffix}`,
            templateType: 2,
            accountId: String(account.id),
            categoryId: String(category.id),
            tagId: String(tag.id),
            clientSessionId: suffix
        }),
        scheduledFrequencyType: 2,
        scheduledFrequency: '1',
        scheduledStartDate: '2026-01-01',
        scheduledEndDate: '2026-12-31',
        utcOffset: 480
    });
    const categoryRule = await client.post<Entity>('category-rules/', {
        category_id: Number(category.id),
        name: `E2E Category Rule ${suffix}`,
        priority: 10,
        rule_expression: `keyword:${suffix}`,
        regex_enabled: false,
        enabled: true
    });
    const accountRule = await client.post<Entity>('account-rules/', {
        account_id: Number(account.id),
        name: `E2E Account Rule ${suffix}`,
        priority: 10,
        rule_expression: `account:${suffix}`,
        regex_enabled: false,
        enabled: true
    });

    return {
        account,
        category,
        tag,
        budget,
        template,
        schedule,
        categoryRule,
        accountRule
    };
}

export async function updateDomainCrudFixture(client: E2EApiClient, fixture: DomainCrudFixture, env: E2EEnvironment): Promise<DomainCrudFixture> {
    const suffix = `${env.runId}-updated`;
    const budgetPeriod = currentMonthDateRange();
    const account = await client.put<Entity>(`accounts/${fixture.account.id}`, {
        id: String(fixture.account.id),
        name: `E2E Cash ${suffix}`,
        category: 1,
        icon: '1',
        color: '#2f9e44',
        comment: `updated by ${suffix}`,
        hidden: false,
        clientSessionId: suffix
    });
    const category = await client.put<Entity>(`categories/${fixture.category.id}`, {
        id: String(fixture.category.id),
        name: `E2E Category ${suffix}`,
        type: 3,
        parentId: '0',
        icon: '1',
        color: '#f08c00',
        comment: `updated by ${suffix}`,
        displayOrder: 0,
        ruleExpression: '',
        hidden: false
    });
    const tag = await client.put<Entity>(`tags/${fixture.tag.id}`, {
        id: String(fixture.tag.id),
        name: `E2E Tag ${suffix}`
    });
    const budgetName = `E2E Budget ${suffix}`;
    await client.put<Record<string, unknown>>(`budgets/${fixture.budget.id}`, {
        name: budgetName,
        category: category.name,
        sub_category: '',
        period_type: 'monthly',
        amountCents: 23456,
        start_date: budgetPeriod.startDate,
        end_date: budgetPeriod.endDate,
        alert_threshold: 75,
        enabled: true
    });
    const budget = { id: fixture.budget.id, name: budgetName };
    const template = await client.put<Entity>(`templates/${fixture.template.id}?templateType=1`, buildTemplatePayload({
        id: String(fixture.template.id),
        name: `E2E Template ${suffix}`,
        templateType: 1,
        accountId: String(account.id),
        categoryId: String(category.id),
        tagId: String(tag.id),
        clientSessionId: suffix
    }));
    const schedule = await client.put<Entity>(`templates/${fixture.schedule.id}?templateType=2`, {
        ...buildTemplatePayload({
            id: String(fixture.schedule.id),
            name: `E2E Schedule ${suffix}`,
            templateType: 2,
            accountId: String(account.id),
            categoryId: String(category.id),
            tagId: String(tag.id),
            clientSessionId: suffix
        }),
        scheduledFrequencyType: 2,
        scheduledFrequency: '1',
        scheduledStartDate: '2026-01-01',
        scheduledEndDate: '2026-12-31',
        utcOffset: 480
    });
    const categoryRule = await client.put<Entity>(`category-rules/${fixture.categoryRule.id}`, {
        name: `E2E Category Rule ${suffix}`,
        priority: 11,
        rule_expression: `keyword:${suffix}`,
        regex_enabled: false,
        enabled: true
    });
    const accountRule = await client.put<Entity>(`account-rules/${fixture.accountRule.id}`, {
        name: `E2E Account Rule ${suffix}`,
        priority: 11,
        rule_expression: `account:${suffix}`,
        regex_enabled: false,
        enabled: true
    });

    return {
        account,
        category,
        tag,
        budget,
        template,
        schedule,
        categoryRule,
        accountRule
    };
}

export async function deleteDomainCrudFixture(client: E2EApiClient, fixture: DomainCrudFixture): Promise<void> {
    await bestEffortDelete(() => client.delete(`account-rules/${fixture.accountRule.id}`));
    await bestEffortDelete(() => client.delete(`category-rules/${fixture.categoryRule.id}`));
    await bestEffortDelete(() => client.delete(`templates/${fixture.schedule.id}?templateType=2`));
    await bestEffortDelete(() => client.delete(`templates/${fixture.template.id}?templateType=1`));
    await bestEffortDelete(() => client.delete(`budgets/${fixture.budget.id}`));
    await bestEffortDelete(() => client.delete(`tags/${fixture.tag.id}`));
    await bestEffortDelete(() => client.delete(`categories/${fixture.category.id}`));
    await bestEffortDelete(() => client.delete(`accounts/${fixture.account.id}`));
}

export async function expectEntityVisibleOnPage(page: Page, name: string): Promise<void> {
    const matches = page.getByText(name, { exact: false });
    await expect(async () => {
        const count = await matches.count();
        for (let index = 0; index < count; index += 1) {
            if (await matches.nth(index).isVisible()) {
                return;
            }
        }
        throw new Error(`entity ${name} should be visible`);
    }).toPass({ timeout: 15_000 });
}

export async function expectDomainRulesVisibleInApi(client: E2EApiClient, fixture: DomainCrudFixture): Promise<void> {
    const categoryRules = await client.get<unknown>('category-rules/');
    const accountRules = await client.get<unknown>('account-rules/');

    expect(JSON.stringify(categoryRules), `category rule ${fixture.categoryRule.name} should be listed by API`)
        .toContain(fixture.categoryRule.name);
    expect(JSON.stringify(accountRules), `account rule ${fixture.accountRule.name} should be listed by API`)
        .toContain(fixture.accountRule.name);
}

function currentMonthDateRange(now: Date = new Date()): { startDate: string; endDate: string } {
    const year = now.getFullYear();
    const monthIndex = now.getMonth();
    const month = String(monthIndex + 1).padStart(2, '0');
    const lastDay = String(new Date(year, monthIndex + 1, 0).getDate()).padStart(2, '0');
    return {
        startDate: `${year}-${month}-01`,
        endDate: `${year}-${month}-${lastDay}`
    };
}

function buildTemplatePayload(input: {
    readonly id?: string;
    readonly name: string;
    readonly templateType: number;
    readonly accountId: string;
    readonly categoryId: string;
    readonly tagId: string;
    readonly clientSessionId: string;
}): Record<string, unknown> {
    return {
        ...(input.id ? { id: input.id } : {}),
        templateType: input.templateType,
        name: input.name,
        type: 3,
        categoryId: input.categoryId,
        sourceAccountId: input.accountId,
        destinationAccountId: '0',
        sourceAmountCents: 1234,
        destinationAmountCents: 0,
        hideAmount: false,
        tagIds: [input.tagId],
        comment: `created by ${input.clientSessionId}`,
        clientSessionId: input.clientSessionId
    };
}

function entityFromResponse(response: Record<string, unknown>, fallbackName: string): Entity {
    const id = response['id'] ?? response['budget_id'] ?? response['budgetId'];
    if (typeof id !== 'string' && typeof id !== 'number') {
        throw new Error(`Entity response is missing id: ${JSON.stringify(response)}`);
    }
    return {
        id,
        name: typeof response['name'] === 'string' ? response['name'] : fallbackName
    };
}

async function bestEffortDelete(action: () => Promise<unknown>): Promise<void> {
    try {
        await action();
    } catch (error) {
        console.warn('E2E resource cleanup failed; final whole-account cleanup remains authoritative.', error);
    }
}
