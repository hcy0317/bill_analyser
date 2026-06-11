import type { APIRequestContext } from '@playwright/test';

import { E2EApiClient } from './apiClient';
import { runEndCleanup, runStartCleanup } from './dataLifecycle';
import { assertBackendHealthy, getE2EEnvironment, type E2EEnvironment } from './env';
import { loginE2EAccount } from './testAccount';

export interface CleanE2ESession {
    readonly env: E2EEnvironment;
    readonly client: E2EApiClient;
}

export async function createCleanE2ESession(
    request: APIRequestContext,
    env: E2EEnvironment = getE2EEnvironment()
): Promise<CleanE2ESession> {
    await assertBackendHealthy(request, env);

    const client = new E2EApiClient(request, env);
    const auth = await loginE2EAccount(client, env);
    client.setToken(auth.token);
    await runStartCleanup(client, env);

    return { env, client };
}

export async function cleanupE2ESession(session: CleanE2ESession): Promise<void> {
    const cleanup = await runEndCleanup(session.client, session.env);
    if (!cleanup.ok) {
        console.warn('E2E per-test cleanup failed; global teardown remains the final safety net.', cleanup.error);
    }
}
