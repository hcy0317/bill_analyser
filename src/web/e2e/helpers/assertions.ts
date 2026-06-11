import { expect, type Page } from '@playwright/test';

export async function expectPageAnchor(page: Page, testId: string): Promise<void> {
    await expect(page.getByTestId(testId), `page anchor ${testId}`).toBeVisible({
        timeout: 15_000
    });
}

export async function expectNoLoginRedirect(page: Page, loginTestId: string): Promise<void> {
    await expect(page.getByTestId(loginTestId), 'authenticated route should not show login page').toHaveCount(0);
}
