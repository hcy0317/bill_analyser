import { request } from '@playwright/test';

import { E2EApiClient } from '../helpers/apiClient';
import { runEndCleanup } from '../helpers/dataLifecycle';
import { assertBackendHealthy, assertLocalE2EEnvironment, getE2EEnvironment } from '../helpers/env';
import { loginE2EAccount } from '../helpers/testAccount';

export default async function globalTeardown(): Promise<void> {
    const env = getE2EEnvironment();

    try {
        assertLocalE2EEnvironment(env);
    } catch (error) {
        console.warn('E2E end cleanup skipped because the environment is not a local E2E target.', error);
        return;
    }

    const requestContext = await request.newContext();
    try {
        try {
            await assertBackendHealthy(requestContext, env);
        } catch (error) {
            console.warn('E2E end cleanup skipped because backend health is unavailable.', error);
            return;
        }

        const client = new E2EApiClient(requestContext, env);
        const auth = await loginE2EAccount(client, env);
        client.setToken(auth.token);

        const cleanup = await runEndCleanup(client, env);
        if (!cleanup.ok) {
            console.warn('E2E end cleanup failed; inspect Playwright artifacts and the dedicated test account.', cleanup.error);
        }
    } catch (error) {
        console.warn('E2E end cleanup could not complete; inspect Playwright artifacts and the dedicated test account.', error);
    } finally {
        await requestContext.dispose();
    }
}
