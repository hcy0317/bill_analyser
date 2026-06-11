import { beforeEach, expect, jest, test } from '@jest/globals';

import { assertBackendHealthy, type E2EEnvironment } from '../../../src/web/e2e/helpers/env';

function buildEnvironment(overrides: Partial<E2EEnvironment> = {}): E2EEnvironment {
    return {
        baseURL: 'http://127.0.0.1:8081',
        apiBaseURL: 'http://127.0.0.1:8081/api',
        account: {
            username: 'bill-analyser-e2e',
            email: 'bill-analyser-e2e@example.test',
            nickname: 'Bill Analyser E2E',
            password: 'BillAnalyserE2E!2026'
        },
        runId: 'e2e-test',
        expectedWeaviatePrefix: 'BillAnalyserE2E',
        allowSharedLocalDatabase: false,
        healthTimeoutMs: 10_000,
        ...overrides
    };
}

function buildHealthRequest(postgresURL: string): never {
    return {
        get: jest.fn(async () => ({
            ok: () => true,
            json: async () => ({
                status: 'ok',
                details: {
                    postgres_url_redacted: postgresURL,
                    weaviate_collection_prefix: 'BillAnalyserE2E'
                }
            })
        }))
    } as never;
}

beforeEach(() => {
    delete process.env['BILL_ANALYSER_POSTGRES_URL'];
});

test('E2E health guard accepts local dedicated PostgreSQL databases', async () => {
    await expect(
        assertBackendHealthy(
            buildHealthRequest('postgres://bill_analyser:***@127.0.0.1:5432/bill_analyser_e2e'),
            buildEnvironment()
        )
    ).resolves.toEqual(expect.objectContaining({ status: 'ok' }));
});

test('E2E health guard rejects the default local development PostgreSQL database', async () => {
    await expect(
        assertBackendHealthy(
            buildHealthRequest('postgres://bill_analyser:***@127.0.0.1:5432/bill_analyser'),
            buildEnvironment()
        )
    ).rejects.toThrow('health details.postgres_url_redacted database name must include "e2e" or "test"');
});

test('E2E health guard rejects remote PostgreSQL even when shared local DB mode is enabled', async () => {
    await expect(
        assertBackendHealthy(
            buildHealthRequest('postgres://bill_analyser:***@db.example.test:5432/bill_analyser_e2e'),
            buildEnvironment({ allowSharedLocalDatabase: true })
        )
    ).rejects.toThrow('health details.postgres_url_redacted must point to a local PostgreSQL host');
});
