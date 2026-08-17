import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockAxiosPost = jest.fn<(...args: Array<unknown>) => Promise<any>>();

jest.mock('axios', () => ({
    __esModule: true,
    default: {
        get: jest.fn(),
        post: mockAxiosPost,
        put: jest.fn(),
        delete: jest.fn(),
        defaults: { baseURL: '', timeout: 0, headers: { common: {} } },
        interceptors: {
            request: { use: jest.fn() },
            response: { use: jest.fn() }
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
import type { ImportPreviewCandidateActionPayload } from '@/models/import_preview.ts';

describe('import learning matching candidate service contract', () => {
    beforeEach(() => {
        mockAxiosPost.mockReset();
    });

    test('uses the existing matching action route and returns the refreshed preview item', async () => {
        mockAxiosPost.mockResolvedValue({
            data: {
                success: true,
                data: {
                    candidateId: 'preview:44:learning',
                    sessionId: 'session-1',
                    reviewStatus: 'accepted',
                    previewItem: {
                        id: 44,
                        matching: {
                            learning: {
                                review_status: 'accepted',
                                suppressed: false
                            }
                        }
                    }
                }
            }
        });
        const payload: ImportPreviewCandidateActionPayload = {
            expectedState: {
                sessionId: 'session-1',
                reviewStatus: 'pending'
            },
            responseMode: 'preview-item'
        };

        const response = await services.acceptMatchingCandidate({
            candidateId: 'preview:44:learning',
            payload
        });

        expect(mockAxiosPost).toHaveBeenCalledWith(
            'matching/candidates/preview%3A44%3Alearning/accept',
            payload
        );
        expect(response.data.result).toMatchObject({
            sessionId: 'session-1',
            reviewStatus: 'accepted',
            previewItem: {
                id: 44,
                matching: {
                    learning: { review_status: 'accepted' }
                }
            }
        });
    });
});
