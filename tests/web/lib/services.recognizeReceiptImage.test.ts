import { afterEach, beforeEach, describe, expect, jest, test } from '@jest/globals';

type PostFormCall = {
    url: string;
    data: Record<string, unknown>;
    config: Record<string, unknown>;
};

const calls: PostFormCall[] = [];

const interceptorStub = {
    use: jest.fn(),
    eject: jest.fn()
};

const axiosMock = {
    defaults: { baseURL: '', timeout: 0, headers: { common: {} as Record<string, string> } },
    interceptors: { request: interceptorStub, response: interceptorStub },
    get: jest.fn(),
    post: jest.fn(),
    put: jest.fn(),
    delete: jest.fn(),
    postForm: jest.fn((url: string, data: Record<string, unknown>, config: Record<string, unknown>) => {
        calls.push({ url, data, config });
        return Promise.resolve({ data: { success: true, result: { amount: 1, trade_time: '2026-01-01T00:00:00Z', description: '', provenance: { provider: 'p', request_id: 'r' }, confidence: 0.9 } } });
    })
};

jest.mock('axios', () => ({
    __esModule: true,
    default: axiosMock,
    AxiosHeaders: class {}
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

// services.ts touches window.location at module load via getBasePath();
// stub a minimal window for the node test environment.
(globalThis as unknown as { window: { location: { pathname: string; origin: string } } }).window = {
    location: { pathname: '/', origin: 'http://localhost' }
};

let services: typeof import('@/lib/services.ts').default;

function installPostFormImpl(): void {
    axiosMock.postForm.mockImplementation((url: string, data: Record<string, unknown>, config: Record<string, unknown>) => {
        calls.push({ url, data, config });
        return Promise.resolve({ data: { success: true, result: { amount: 1, trade_time: '2026-01-01T00:00:00Z', description: '', provenance: { provider: 'p', request_id: 'r' }, confidence: 0.9 } } });
    });
}

beforeEach(async () => {
    calls.length = 0;
    axiosMock.postForm.mockReset();
    installPostFormImpl();
    jest.resetModules();
    services = (await import('@/lib/services.ts')).default;
});

afterEach(() => {
    // intentionally do not call jest.resetAllMocks() — that wipes the postForm impl shared across tests.
});

describe('services.recognizeReceiptImage', () => {
    test('posts to ml/receipt-recognition with multipart image field', async () => {
        const file = new File([new Uint8Array([7])], 'receipt.jpg', { type: 'image/jpeg' });

        await services.recognizeReceiptImage({ imageFile: file });

        expect(axiosMock.postForm).toHaveBeenCalledTimes(1);
        const call = calls[0]!;
        expect(call.url).toBe('ml/receipt-recognition');
        expect(call.data['image']).toBe(file);
    });

    test('forwards cancelableUuid into axios request config', async () => {
        const file = new File([new Uint8Array([8])], 'receipt2.jpg', { type: 'image/jpeg' });

        await services.recognizeReceiptImage({ imageFile: file, cancelableUuid: 'abc-uuid' });

        const call = calls[0]!;
        expect(call.config['cancelableUuid']).toBe('abc-uuid');
        expect(typeof call.config['timeout']).toBe('number');
    });

    test('omits cancelableUuid when not provided (undefined still passed through config)', async () => {
        const file = new File([new Uint8Array([9])], 'receipt3.jpg', { type: 'image/jpeg' });

        await services.recognizeReceiptImage({ imageFile: file });

        const call = calls[0]!;
        expect(call.config['cancelableUuid']).toBeUndefined();
    });
});
