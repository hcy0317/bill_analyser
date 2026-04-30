import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockAxiosGet = jest.fn<(...args: Array<unknown>) => Promise<any>>();

jest.mock('axios', () => ({
    __esModule: true,
    default: {
        get: mockAxiosGet,
        post: jest.fn(),
        put: jest.fn(),
        delete: jest.fn(),
        defaults: {
            baseURL: '',
            timeout: 0,
            headers: {
                common: {}
            }
        },
        interceptors: {
            request: {
                use: jest.fn()
            },
            response: {
                use: jest.fn()
            }
        }
    }
}));

jest.mock('@/lib/web.ts', () => ({
    __esModule: true,
    getBasePath: () => '/desktop'
}));

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: {
        debug: jest.fn(),
        info: jest.fn(),
        warn: jest.fn(),
        error: jest.fn()
    }
}));

import services from '@/lib/services.ts';

describe('services llm memory wrapper', () => {
    beforeEach(() => {
        mockAxiosGet.mockReset();
    });

    test('preserves both filtered events and total from /api/llm/memory responses', async () => {
        mockAxiosGet.mockResolvedValue({
            data: {
                success: true,
                data: [
                    {
                        preview_id: 9,
                        event_type: 'feedback',
                        decision: 'accept'
                    }
                ],
                total: 3
            }
        });

        const response = await services.getLLMMemoryEvents({
            session_id: 'session-a',
            event_type: 'feedback',
            limit: 50,
            offset: 10
        });

        expect(mockAxiosGet).toHaveBeenCalledWith('llm/memory', {
            params: {
                session_id: 'session-a',
                event_type: 'feedback',
                limit: 50,
                offset: 10
            }
        });
        expect(response.data.success).toBe(true);
        expect(response.data.result).toStrictEqual({
            events: [
                {
                    preview_id: 9,
                    event_type: 'feedback',
                    decision: 'accept'
                }
            ],
            total: 3
        });
    });
});
