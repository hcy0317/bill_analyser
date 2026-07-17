import { afterEach, expect, jest, test } from '@jest/globals';

import {
    cleanupWeaviateCollections,
    handleCleanupFailure
} from '../../../src/web/e2e/setup/global-teardown';

afterEach(() => {
    delete process.env['BILL_ANALYSER_WEAVIATE_ENDPOINT'];
    jest.restoreAllMocks();
});

test('strict cleanup converts warning paths into blocking errors', () => {
    expect(() => handleCleanupFailure(true, 'strict cleanup failed', new Error('cause')))
        .toThrow('strict cleanup failed');
});

test('strict cleanup deletes and verifies only the run-scoped Weaviate classes', async () => {
    process.env['BILL_ANALYSER_WEAVIATE_ENDPOINT'] = 'http://127.0.0.1:8088';
    const get = jest.fn<(url: string) => Promise<FakeResponse>>()
        .mockResolvedValueOnce(response({
            classes: [
                { class: 'BillAnalyserE2Erun42ImportLearning' },
                { class: 'UnrelatedCollection' }
            ]
        }))
        .mockResolvedValueOnce(response({ classes: [{ class: 'UnrelatedCollection' }] }));
    const deleteRequest = jest.fn<(url: string) => Promise<FakeResponse>>(async () => response({}));

    await cleanupWeaviateCollections(
        { get, delete: deleteRequest } as never,
        'BillAnalyserE2Erun42',
        true
    );

    expect(deleteRequest).toHaveBeenCalledTimes(1);
    expect(deleteRequest).toHaveBeenCalledWith(
        'http://127.0.0.1:8088/v1/schema/BillAnalyserE2Erun42ImportLearning'
    );
    expect(get).toHaveBeenCalledTimes(2);
});

test('strict cleanup accepts the controlled Weaviate service host only under the explicit CI contract', async () => {
    process.env['BILL_ANALYSER_WEAVIATE_ENDPOINT'] = 'http://weaviate:8080';
    const get = jest.fn<(url: string) => Promise<FakeResponse>>()
        .mockResolvedValueOnce(response({ classes: [] }));

    await cleanupWeaviateCollections(
        { get } as never,
        'BillAnalyserE2Erun42',
        true,
        true
    );

    expect(get).toHaveBeenCalledWith('http://weaviate:8080/v1/schema');
});

test('strict cleanup rejects the Weaviate service host without the explicit CI contract', async () => {
    process.env['BILL_ANALYSER_WEAVIATE_ENDPOINT'] = 'http://weaviate:8080';
    const get = jest.fn<(url: string) => Promise<FakeResponse>>();

    await expect(cleanupWeaviateCollections(
        { get } as never,
        'BillAnalyserE2Erun42',
        true,
        false
    )).rejects.toThrow('refuses non-local or uncontrolled endpoint');
    expect(get).not.toHaveBeenCalled();
});

test('strict cleanup rejects remote Weaviate endpoints before issuing requests', async () => {
    process.env['BILL_ANALYSER_WEAVIATE_ENDPOINT'] = 'https://weaviate.example.test';
    const get = jest.fn<(url: string) => Promise<FakeResponse>>();

    await expect(cleanupWeaviateCollections(
        { get } as never,
        'BillAnalyserE2Erun42',
        true,
        true
    )).rejects.toThrow('refuses non-local or uncontrolled endpoint');
    expect(get).not.toHaveBeenCalled();
});

type FakeResponse = {
    readonly ok: () => boolean;
    readonly status: () => number;
    readonly json: () => Promise<unknown>;
};

function response(body: unknown, status = 200): FakeResponse {
    return {
        ok: () => status >= 200 && status < 300,
        status: () => status,
        json: async () => body
    };
}
