import type { E2EEnvironment } from './env';
import { E2EApiClient } from './apiClient';

export interface CleanupResult {
    readonly ok: boolean;
    readonly error?: unknown;
}

export async function runStartCleanup(client: E2EApiClient, env: E2EEnvironment): Promise<void> {
    await clearAllUserData(client, env);
}

export async function runEndCleanup(client: E2EApiClient, env: E2EEnvironment): Promise<CleanupResult> {
    try {
        await clearAllUserData(client, env);
        return { ok: true };
    } catch (error) {
        return { ok: false, error };
    }
}

export async function clearAllUserData(client: E2EApiClient, env: E2EEnvironment): Promise<boolean> {
    return client.post<boolean>('data/clear/all', {
        password: env.account.password
    });
}
