import { expect, test } from '@playwright/test';

import { getE2EEnvironment } from '../helpers/env';
import { mobileRoute } from '../helpers/routes';
import { cleanupE2ESession, createCleanE2ESession } from '../helpers/session';

interface Entity {
    readonly id: string | number;
    readonly name: string;
}

const MOBILE_VISUAL_VIEWPORTS = [
    { width: 360, height: 800 },
    { width: 390, height: 844 },
    { width: 430, height: 932 }
] as const;

test.describe('mobile transaction composer', () => {
    test.describe.configure({ retries: 0 });

    test('creates a cents-based expense and shows it in the mobile list', async ({ page, request }, testInfo) => {
        const env = getE2EEnvironment();
        const pageErrors: Error[] = [];
        page.on('pageerror', error => pageErrors.push(error));
        await page.emulateMedia({ colorScheme: 'light' });
        const session = await createCleanE2ESession(request, env);
        const suffix = env.runId;
        const marker = `E2E mobile transaction ${suffix}`;

        try {
            await page.goto(mobileRoute('/transaction/add?type=3&noTransactionDraft=true', env), {
                waitUntil: 'domcontentloaded'
            });
            const emptyEditor = page.locator('.page-current[data-testid="mobile.transactions.edit.page"]');
            const emptyAccountState = emptyEditor.getByTestId('mobile.transactions.edit.account-empty');
            await expect(emptyAccountState).toBeVisible();
            const addAccount = emptyEditor.getByTestId('mobile.transactions.edit.action.add-account');
            for (const viewport of MOBILE_VISUAL_VIEWPORTS) {
                await page.setViewportSize(viewport);
                const addAccountBox = await addAccount.boundingBox();
                expect(addAccountBox).not.toBeNull();
                expect(addAccountBox!.height).toBeGreaterThanOrEqual(44);
            }

            const account = await session.client.post<Entity>('accounts', {
                name: `移动端现金账户回归测试 ${suffix}`,
                category: 1,
                type: 1,
                icon: '1',
                color: '#c67e48',
                currency: 'USD',
                balanceCents: 0,
                balanceTime: 0,
                comment: marker,
                clientSessionId: suffix
            });
            const primaryCategory = await session.client.post<Entity>('categories', {
                name: `移动端支出分类 ${suffix}`,
                type: 3,
                parentId: '0',
                icon: '1',
                color: '#c67e48',
                comment: marker,
                displayOrder: 0,
                ruleExpression: '',
                clientSessionId: suffix
            });
            const category = await session.client.post<Entity>('categories', {
                name: `移动端支出明细分类回归测试 ${suffix}`,
                type: 3,
                parentId: String(primaryCategory.id),
                icon: '1',
                color: '#c67e48',
                comment: marker,
                displayOrder: 0,
                ruleExpression: '',
                clientSessionId: suffix
            });
            const listPath = [
                '/transaction/list?type=3',
                `categoryIds=${encodeURIComponent(String(category.id))}`,
                `accountIds=${encodeURIComponent(String(account.id))}`,
                `keyword=${encodeURIComponent(marker)}`
            ].join('&');

            await page.goto(mobileRoute(listPath, env), { waitUntil: 'domcontentloaded' });
            await expect(page.getByTestId('mobile.transactions.page')).toBeVisible();
            await page.getByTestId('mobile.transactions.action.add').click();

            const editor = page.locator('.page-current[data-testid="mobile.transactions.edit.page"]');
            await expect(editor).toBeVisible();
            await expect(editor.getByTestId('mobile.transactions.edit.source-amount')).toBeVisible();
            await expect(editor.getByTestId('mobile.transactions.edit.category')).toContainText(category.name);
            const save = editor.getByTestId('mobile.transactions.edit.action.save');
            const typeButtons = page.locator('.transaction-type-selector:visible .button');
            await expect(typeButtons).toHaveCount(4);

            for (const viewport of MOBILE_VISUAL_VIEWPORTS) {
                await page.setViewportSize(viewport);
                await expect(editor).toBeVisible();
                const typeButtonBoxes = await typeButtons.evaluateAll(buttons => buttons.map(button => {
                    const box = button.getBoundingClientRect();
                    return { top: Math.round(box.top), height: box.height };
                }));
                expect(new Set(typeButtonBoxes.map(box => box.top)).size).toBe(1);
                expect(typeButtonBoxes.every(box => box.height >= 44)).toBe(true);
                const saveBox = await save.boundingBox();
                expect(saveBox).not.toBeNull();
                expect(saveBox!.height).toBeGreaterThanOrEqual(44);
                expect(saveBox!.y).toBeGreaterThanOrEqual(0);
                expect(saveBox!.y + saveBox!.height).toBeLessThanOrEqual(viewport.height);
                expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
                await page.screenshot({
                    path: testInfo.outputPath(`composer-light-${viewport.width}x${viewport.height}.png`),
                    animations: 'disabled'
                });
            }

            await page.setViewportSize({ width: 390, height: 844 });
            await page.emulateMedia({ colorScheme: 'dark' });
            await expect(page.locator('html')).toHaveClass(/dark/u);
            await page.screenshot({
                path: testInfo.outputPath('composer-dark-390x844.png'),
                animations: 'disabled'
            });
            await page.emulateMedia({ colorScheme: 'light' });
            await expect(page.locator('html')).not.toHaveClass(/dark/u);
            await editor.getByTestId('mobile.transactions.edit.source-amount').click();

            const numberPad = page.locator('.numpad-sheet.modal-in');
            await expect(numberPad).toBeVisible();
            for (const digit of ['1', '2', '3', '.', '4', '5']) {
                await numberPad.locator('.numpad-button-num').filter({ hasText: new RegExp(`^${digit.replace('.', '\\.')}$`, 'u') }).first().click();
            }
            await numberPad.locator('.numpad-button-confirm').click();
            await expect(numberPad).toBeHidden();
            await editor.locator('textarea[name="transaction-comment"]').fill(marker);

            await expect(editor.getByTestId('mobile.transactions.edit.validation')).toHaveCount(0);
            await expect(save).not.toHaveClass(/disabled/u);
            const saveBox = await save.boundingBox();
            const viewport = page.viewportSize();
            expect(saveBox).not.toBeNull();
            expect(viewport).not.toBeNull();
            expect(saveBox!.y).toBeGreaterThanOrEqual(0);
            expect(saveBox!.y + saveBox!.height).toBeLessThanOrEqual(viewport!.height);
            const createRequest = page.waitForRequest(request => (
                new URL(request.url()).pathname === '/api/bills'
                && request.method() === 'POST'
            ));
            const createResponse = page.waitForResponse(response => (
                new URL(response.url()).pathname === '/api/bills'
                && response.request().method() === 'POST'
            ));
            await save.click();
            const requestPayload = (await createRequest).postDataJSON() as { sourceAmountCents?: number };
            expect(requestPayload.sourceAmountCents).toBe(12_345);
            const response = await createResponse;
            expect(response.ok()).toBe(true);
            const responsePayload = await response.json() as {
                result?: { sourceAmountCents?: number };
            };
            expect(responsePayload.result?.sourceAmountCents).toBe(12_345);

            await expect(page.getByTestId('mobile.transactions.page')).toBeVisible();
            await expect(page.getByText(marker, { exact: true })).toBeVisible();
            expect(pageErrors).toEqual([]);
        } finally {
            await cleanupE2ESession(session);
        }
    });
});
