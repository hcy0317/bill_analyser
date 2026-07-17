import { expect, type APIRequestContext } from '@playwright/test';

export interface E2EAccount {
    readonly username: string;
    readonly email: string;
    readonly nickname: string;
    readonly password: string;
}

export interface E2EEnvironment {
    readonly baseURL: string;
    readonly apiBaseURL: string;
    readonly account: E2EAccount;
    readonly runId: string;
    readonly expectedWeaviatePrefix: string;
    readonly allowSharedLocalDatabase: boolean;
    readonly allowCIServiceHosts: boolean;
    readonly healthTimeoutMs: number;
}

interface RuntimeHealth {
    readonly status?: string;
    readonly details?: Record<string, string>;
}

const LOCAL_HOSTS = new Set(['127.0.0.1', 'localhost', '::1']);
const CONTROLLED_CI_POSTGRES_HOSTS = new Set(['postgres']);

function readEnv(name: string, fallback: string): string {
    const value = process.env[name]?.trim();
    return value ? value : fallback;
}

function readBooleanEnv(name: string, fallback: boolean): boolean {
    const value = process.env[name]?.trim().toLowerCase();
    if (!value) {
        return fallback;
    }

    return ['1', 'true', 'yes', 'on'].includes(value);
}

function trimTrailingSlash(value: string): string {
    return value.replace(/\/+$/u, '');
}

function parsePositiveIntegerEnv(name: string, fallback: number): number {
    const raw = process.env[name]?.trim();
    if (!raw) {
        return fallback;
    }

    const value = Number(raw);
    if (!Number.isInteger(value) || value <= 0) {
        throw new Error(`${name} must be a positive integer, got ${raw}.`);
    }

    return value;
}

export function getE2EEnvironment(): E2EEnvironment {
    const baseURL = trimTrailingSlash(readEnv('E2E_BASE_URL', 'http://127.0.0.1:8081'));

    return {
        baseURL,
        apiBaseURL: trimTrailingSlash(readEnv('E2E_API_BASE_URL', `${baseURL}/api`)),
        account: {
            username: readEnv('E2E_ACCOUNT_USERNAME', 'bill-analyser-e2e'),
            email: readEnv('E2E_ACCOUNT_EMAIL', 'bill-analyser-e2e@example.test'),
            nickname: readEnv('E2E_ACCOUNT_NICKNAME', 'Bill Analyser E2E'),
            password: readEnv('E2E_ACCOUNT_PASSWORD', 'BillAnalyserE2E!2026')
        },
        runId: readEnv('E2E_RUN_ID', `e2e-${new Date().toISOString().replace(/[-:.TZ]/gu, '').slice(0, 14)}`),
        expectedWeaviatePrefix: readEnv('E2E_EXPECTED_WEAVIATE_PREFIX', 'BillAnalyserE2E'),
        allowSharedLocalDatabase: readBooleanEnv('E2E_ALLOW_SHARED_LOCAL_DATABASE', false),
        allowCIServiceHosts: readBooleanEnv('E2E_ALLOW_CI_SERVICE_HOSTS', false),
        healthTimeoutMs: parsePositiveIntegerEnv('E2E_HEALTH_TIMEOUT_MS', 10_000)
    };
}

export function apiURL(path: string, env: E2EEnvironment = getE2EEnvironment()): string {
    return `${env.apiBaseURL}/${path.replace(/^\/+/u, '')}`;
}

export function assertLocalE2EEnvironment(env: E2EEnvironment = getE2EEnvironment()): void {
    assertLocalURL(env.baseURL, 'E2E_BASE_URL');
    assertLocalURL(env.apiBaseURL, 'E2E_API_BASE_URL');

    const postgresURL = process.env['BILL_ANALYSER_POSTGRES_URL']?.trim();
    if (postgresURL) {
        assertLocalPostgresURL(postgresURL, env.allowSharedLocalDatabase, env.allowCIServiceHosts);
    }
}

export async function assertBackendHealthy(
    request: APIRequestContext,
    env: E2EEnvironment = getE2EEnvironment()
): Promise<RuntimeHealth> {
    assertLocalE2EEnvironment(env);

    const response = await request.get(apiURL('health', env), {
        timeout: env.healthTimeoutMs
    });
    expect(response.ok(), `GET ${apiURL('health', env)} should return 2xx`).toBe(true);

    const health = await response.json() as RuntimeHealth;
    expect(health.status, 'GET /api/health status').toBe('ok');

    if (env.expectedWeaviatePrefix) {
        expect(
            health.details?.['weaviate_collection_prefix'],
            'health details must expose the dedicated E2E Weaviate collection prefix'
        ).toBe(env.expectedWeaviatePrefix);
    }

    assertHealthPostgresTarget(health, env);

    return health;
}

function assertLocalURL(value: string, label: string): void {
    let parsed: URL;
    try {
        parsed = new URL(value);
    } catch (error) {
        throw new Error(`${label} must be an absolute local URL, got ${value}.`, { cause: error });
    }

    if (!LOCAL_HOSTS.has(parsed.hostname)) {
        throw new Error(`${label} must point to localhost or 127.0.0.1 for destructive E2E cleanup, got ${value}.`);
    }
}

function assertLocalPostgresURL(
    value: string,
    allowSharedLocalDatabase: boolean,
    allowCIServiceHosts: boolean
): void {
    let parsed: URL;
    try {
        parsed = new URL(value);
    } catch (error) {
        throw new Error('BILL_ANALYSER_POSTGRES_URL must be a valid URL when provided to E2E.', { cause: error });
    }

    if (!isAllowedPostgresHost(parsed.hostname, allowCIServiceHosts)) {
        throw new Error(
            'BILL_ANALYSER_POSTGRES_URL must point to a local or explicitly controlled CI PostgreSQL host for E2E cleanup.'
        );
    }

    assertSafePostgresDatabaseName(
        parsed.pathname.replace(/^\/+/u, ''),
        allowSharedLocalDatabase,
        'BILL_ANALYSER_POSTGRES_URL'
    );
}

function assertHealthPostgresTarget(health: RuntimeHealth, env: E2EEnvironment): void {
    const value = health.details?.['postgres_url_redacted']?.trim();
    if (!value || value === 'unconfigured') {
        throw new Error('GET /api/health must expose details.postgres_url_redacted before destructive E2E cleanup.');
    }

    let parsed: URL;
    try {
        parsed = new URL(value);
    } catch (error) {
        throw new Error(`health details.postgres_url_redacted must be a parseable PostgreSQL URL, got ${value}.`, {
            cause: error
        });
    }

    if (!matchesPostgresScheme(parsed.protocol)) {
        throw new Error(`health details.postgres_url_redacted must use postgres/postgresql, got ${value}.`);
    }

    if (!isAllowedPostgresHost(parsed.hostname, env.allowCIServiceHosts)) {
        throw new Error(
            `health details.postgres_url_redacted must point to a local or explicitly controlled CI PostgreSQL host, got ${value}.`
        );
    }

    assertSafePostgresDatabaseName(
        parsed.pathname.replace(/^\/+/u, ''),
        env.allowSharedLocalDatabase,
        'health details.postgres_url_redacted'
    );
}

function matchesPostgresScheme(protocol: string): boolean {
    return protocol === 'postgres:' || protocol === 'postgresql:';
}

function isAllowedPostgresHost(hostname: string, allowCIServiceHosts: boolean): boolean {
    return LOCAL_HOSTS.has(hostname)
        || (allowCIServiceHosts && CONTROLLED_CI_POSTGRES_HOSTS.has(hostname));
}

function assertSafePostgresDatabaseName(databaseName: string, allowSharedLocalDatabase: boolean, label: string): void {
    if (!allowSharedLocalDatabase && !/(e2e|test)/iu.test(databaseName)) {
        throw new Error(
            `${label} database name must include "e2e" or "test", or set E2E_ALLOW_SHARED_LOCAL_DATABASE=true for an explicitly accepted local-only run.`
        );
    }
}
