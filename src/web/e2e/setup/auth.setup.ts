import { test as setup } from '@playwright/test';

import { E2EApiClient } from '../helpers/apiClient';
import { runStartCleanup } from '../helpers/dataLifecycle';
import { assertBackendHealthy, getE2EEnvironment } from '../helpers/env';
import {
    ensureE2EAccount,
    loginE2EAccount,
    saveAuthenticatedStorageState
} from '../helpers/testAccount';

setup('authenticate dedicated E2E account', async ({ browser, request }) => {
    const env = getE2EEnvironment();

    await assertBackendHealthy(request, env);

    const client = new E2EApiClient(request, env);
    const initialAuth = await ensureE2EAccount(client, env);
    client.setToken(initialAuth.token);

    await runStartCleanup(client, env);

    const authAfterCleanup = await loginE2EAccount(client, env);
    client.setToken(authAfterCleanup.token);
    await saveAuthenticatedStorageState(browser, env, authAfterCleanup);
});
