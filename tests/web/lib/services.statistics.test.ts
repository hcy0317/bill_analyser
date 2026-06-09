import { beforeEach, describe, expect, jest, test } from '@jest/globals';

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
    postForm: jest.fn()
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

(globalThis as unknown as { window: { location: { pathname: string; origin: string } } }).window = {
    location: { pathname: '/', origin: 'http://localhost' }
};

let services: typeof import('@/lib/services.ts').default;

beforeEach(async () => {
    axiosMock.get.mockReset();
    axiosMock.post.mockReset();
    axiosMock.put.mockReset();
    axiosMock.delete.mockReset();
    axiosMock.postForm.mockReset();
    axiosMock.get.mockImplementation(() => Promise.resolve({ data: { success: true, result: [] } }));
    jest.resetModules();
    services = (await import('@/lib/services.ts')).default;
});

describe('services statistics adapters', () => {
    test('builds category statistics URL with current snake-case query contract', async () => {
        await services.getTransactionStatistics({
            startTime: 0,
            endTime: 1_775_087_999,
            tagIds: 'tag-a,tag/b',
            tagFilterType: 2,
            keyword: 'coffee & milk',
            useTransactionTimezone: true
        });

        expect(axiosMock.get).toHaveBeenCalledWith(
            'statistics/category-statistics?use_transaction_timezone=true'
            + '&start_time=0&end_time=1775087999&tag_ids=tag-a,tag/b'
            + '&tag_filter_type=2&keyword=coffee%20%26%20milk'
        );
    });

    test('builds trend and asset statistics URLs with current range sentinels', async () => {
        await services.getTransactionStatisticsTrends({
            startYearMonth: '197001',
            endYearMonth: '197001',
            tagIds: 'tag-1',
            tagFilterType: 1,
            keyword: '餐饮',
            useTransactionTimezone: false
        });
        expect(axiosMock.get).toHaveBeenLastCalledWith(
            'statistics/category-statistics/trends?use_transaction_timezone=false'
            + '&start_year_month=197001&end_year_month=197001'
            + '&tag_ids=tag-1&tag_filter_type=1&keyword=%E9%A4%90%E9%A5%AE'
        );

        await services.getTransactionStatisticsAssetTrends({
            startTime: 1_772_409_600,
            endTime: 1_772_496_000
        });
        expect(axiosMock.get).toHaveBeenLastCalledWith(
            'statistics/asset-trends?start_time=1772409600&end_time=1772496000'
        );
    });

    test('builds amounts and exchange-rate requests without normalizing ignored params', async () => {
        await services.getTransactionAmounts(
            {
                useTransactionTimezone: false,
                today: { startTime: 1, endTime: 2 },
                thisMonth: { startTime: 3, endTime: 4 }
            },
            ['acc-1', 'acc-2'],
            ['cat/1']
        );
        expect(axiosMock.get).toHaveBeenLastCalledWith(
            'statistics/amounts?use_transaction_timezone=false'
            + '&query=today_1_2|thisMonth_3_4'
            + '&exclude_account_ids=acc-1,acc-2&exclude_category_ids=cat/1'
        );

        await services.getLatestExchangeRates({ ignoreError: true, provider: 'boc_cn' });
        expect(axiosMock.get).toHaveBeenLastCalledWith(
            'statistics/exchange-rates',
            expect.objectContaining({
                params: { provider: 'boc_cn' },
                ignoreError: true,
                timeout: expect.any(Number)
            })
        );
    });
});
