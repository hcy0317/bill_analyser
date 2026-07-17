import { request, type APIRequestContext } from '@playwright/test';

import { E2EApiClient } from '../helpers/apiClient';
import { clearAllUserData } from '../helpers/dataLifecycle';
import { assertBackendHealthy, assertLocalE2EEnvironment, getE2EEnvironment } from '../helpers/env';
import { loginE2EAccount } from '../helpers/testAccount';

export default async function globalTeardown(): Promise<void> {
    const env = getE2EEnvironment();
    const strictCleanup = readStrictCleanup();

    try {
        assertLocalE2EEnvironment(env);
    } catch (error) {
        return handleCleanupFailure(
            strictCleanup,
            'E2E end cleanup rejected a non-local or unsafe environment.',
            error
        );
    }

    const requestContext = await request.newContext();
    try {
        try {
            await assertBackendHealthy(requestContext, env);
        } catch (error) {
            return handleCleanupFailure(
                strictCleanup,
                'E2E end cleanup requires a healthy backend.',
                error
            );
        }

        const client = new E2EApiClient(requestContext, env);
        const auth = await loginE2EAccount(client, env);
        client.setToken(auth.token);

        const cleared = await clearAllUserData(client, env);
        if (!cleared) {
            handleCleanupFailure(
                strictCleanup,
                'E2E end cleanup failed to clear the dedicated test account.',
                null
            );
        }
        if (strictCleanup) {
            await cleanupWeaviateCollections(
                requestContext,
                env.expectedWeaviatePrefix,
                true,
                env.allowCIServiceHosts
            );
        }
    } catch (error) {
        handleCleanupFailure(
            strictCleanup,
            'E2E end cleanup could not complete.',
            error
        );
    } finally {
        await requestContext.dispose();
    }
}

interface WeaviateSchema {
    readonly classes?: Array<{ readonly class?: string }>;
}

const LOCAL_HOSTS = new Set(['127.0.0.1', 'localhost', '::1']);
const CONTROLLED_CI_WEAVIATE_HOSTS = new Set(['weaviate']);

function readStrictCleanup(): boolean {
    return process.env['E2E_STRICT_CLEANUP']?.trim().toLowerCase() === 'true';
}

export function handleCleanupFailure(strict: boolean, message: string, cause: unknown): void {
    if (strict) {
        throw new Error(message, { cause });
    }
    console.warn(message, cause);
}

export async function cleanupWeaviateCollections(
    requestContext: APIRequestContext,
    expectedPrefix: string,
    strict: boolean,
    allowCIServiceHosts = false
): Promise<void> {
    const endpoint = process.env['BILL_ANALYSER_WEAVIATE_ENDPOINT']?.trim();
    if (!endpoint) {
        handleCleanupFailure(strict, 'E2E Weaviate cleanup requires BILL_ANALYSER_WEAVIATE_ENDPOINT.', null);
        return;
    }

    let parsed: URL;
    try {
        parsed = new URL(endpoint);
    } catch (error) {
        handleCleanupFailure(strict, `E2E Weaviate endpoint is invalid: ${endpoint}.`, error);
        return;
    }
    const hostAllowed = LOCAL_HOSTS.has(parsed.hostname)
        || (allowCIServiceHosts && CONTROLLED_CI_WEAVIATE_HOSTS.has(parsed.hostname));
    if (!['http:', 'https:'].includes(parsed.protocol) || !hostAllowed) {
        handleCleanupFailure(
            strict,
            `E2E Weaviate cleanup refuses non-local or uncontrolled endpoint ${endpoint}.`,
            null
        );
        return;
    }
    if (!expectedPrefix.startsWith('BillAnalyserE2E') || expectedPrefix === 'BillAnalyserE2E') {
        handleCleanupFailure(strict, `E2E Weaviate prefix is not run-scoped: ${expectedPrefix}.`, null);
        return;
    }

    const schemaUrl = `${endpoint.replace(/\/+$/u, '')}/v1/schema`;
    const response = await requestContext.get(schemaUrl);
    if (!response.ok()) {
        handleCleanupFailure(strict, `GET ${schemaUrl} returned HTTP ${response.status()}.`, null);
        return;
    }
    const schema = await response.json() as WeaviateSchema;
    const classNames = (schema.classes ?? [])
        .map(item => item.class?.trim())
        .filter((name): name is string => Boolean(name?.startsWith(expectedPrefix)));

    for (const className of classNames) {
        const deleteUrl = `${schemaUrl}/${encodeURIComponent(className)}`;
        const deleted = await requestContext.delete(deleteUrl);
        if (!deleted.ok()) {
            handleCleanupFailure(strict, `DELETE ${deleteUrl} returned HTTP ${deleted.status()}.`, null);
        }
    }

    if (strict && classNames.length > 0) {
        const verification = await requestContext.get(schemaUrl);
        if (!verification.ok()) {
            throw new Error(`GET ${schemaUrl} verification returned HTTP ${verification.status()}.`);
        }
        const verifiedSchema = await verification.json() as WeaviateSchema;
        const remaining = (verifiedSchema.classes ?? [])
            .map(item => item.class?.trim())
            .filter((name): name is string => Boolean(name?.startsWith(expectedPrefix)));
        if (remaining.length > 0) {
            throw new Error(`E2E Weaviate cleanup left run-scoped classes: ${remaining.join(', ')}`);
        }
    }
}
