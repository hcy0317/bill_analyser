import { beforeEach, expect, jest, test } from '@jest/globals';

import {
    assertBackendHealthy,
    getE2EEnvironment,
    type E2EEnvironment
} from '../../../src/web/e2e/helpers/env';

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
        allowCIServiceHosts: false,
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
    delete process.env['E2E_ALLOW_CI_SERVICE_HOSTS'];
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

test('E2E health guard accepts the controlled PostgreSQL service host only under the explicit CI contract', async () => {
    process.env['E2E_ALLOW_CI_SERVICE_HOSTS'] = 'true';
    process.env['BILL_ANALYSER_POSTGRES_URL'] =
        'postgresql://bill_analyser_e2e:secret@postgres:5432/bill_analyser_e2e';

    await expect(
        assertBackendHealthy(
            buildHealthRequest('postgres://bill_analyser_e2e:***@postgres:5432/bill_analyser_e2e'),
            getE2EEnvironment()
        )
    ).resolves.toEqual(expect.objectContaining({ status: 'ok' }));
});

test('E2E health guard rejects the PostgreSQL service host without the explicit CI contract', async () => {
    process.env['BILL_ANALYSER_POSTGRES_URL'] =
        'postgresql://bill_analyser_e2e:secret@postgres:5432/bill_analyser_e2e';

    await expect(
        assertBackendHealthy(
            buildHealthRequest('postgres://bill_analyser_e2e:***@postgres:5432/bill_analyser_e2e'),
            buildEnvironment()
        )
    ).rejects.toThrow('BILL_ANALYSER_POSTGRES_URL must point to a local or explicitly controlled CI PostgreSQL host');
});

test('E2E health guard rejects remote PostgreSQL even when shared local DB mode is enabled', async () => {
    process.env['E2E_ALLOW_CI_SERVICE_HOSTS'] = 'true';
    process.env['BILL_ANALYSER_POSTGRES_URL'] =
        'postgresql://bill_analyser_e2e:secret@db.example.test:5432/bill_analyser_e2e';

    await expect(
        assertBackendHealthy(
            buildHealthRequest('postgres://bill_analyser:***@db.example.test:5432/bill_analyser_e2e'),
            buildEnvironment({ allowSharedLocalDatabase: true, allowCIServiceHosts: true })
        )
    ).rejects.toThrow('BILL_ANALYSER_POSTGRES_URL must point to a local or explicitly controlled CI PostgreSQL host');
});

test('E2E health guard rejects a remote health PostgreSQL target under the explicit CI contract', async () => {
    process.env['E2E_ALLOW_CI_SERVICE_HOSTS'] = 'true';
    process.env['BILL_ANALYSER_POSTGRES_URL'] =
        'postgresql://bill_analyser_e2e:secret@postgres:5432/bill_analyser_e2e';

    await expect(
        assertBackendHealthy(
            buildHealthRequest('postgres://bill_analyser_e2e:***@db.example.test:5432/bill_analyser_e2e'),
            buildEnvironment({ allowCIServiceHosts: true })
        )
    ).rejects.toThrow(
        'health details.postgres_url_redacted must point to a local or explicitly controlled CI PostgreSQL host'
    );
});
