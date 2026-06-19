import { describe, expect, test } from '@jest/globals';

import {
    buildLLMSignalMemoryMap,
    parseLLMMemoryEventSignal,
    shouldReplaceLLMSignalMemoryState
} from '@/views/desktop/transactions/import/llmSignalMemory.ts';

describe('llm signal memory helpers', () => {
    test('parses review feedback and falls back to llm_response_raw payload fields', () => {
        const signal = parseLLMMemoryEventSignal({
            preview_id: 101,
            decision: 'reject',
            llm_response_raw: JSON.stringify({
                suggested_main_category: '餐饮',
                suggested_sub_category: '咖啡',
                suggested_source_account: '招商银行',
                suggested_destination_account: '支付宝',
                confidence: 0.87,
                reason: '历史反馈'
            })
        });

        expect(signal).toStrictEqual({
            reviewStatus: 'rejected',
            suppressed: true,
            suggestedMainCategory: '餐饮',
            suggestedSubCategory: '咖啡',
            suggestedSourceAccount: '招商银行',
            suggestedDestinationAccount: '支付宝',
            confidence: 0.87,
            reason: '历史反馈'
        });
    });

    test('returns null for invalid preview ids', () => {
        expect(parseLLMMemoryEventSignal({
            preview_id: 0,
            decision: 'accept'
        })).toBeNull();
    });

    test('preserves suggested transaction type from raw payload', () => {
        const signal = parseLLMMemoryEventSignal({
            preview_id: 102,
            llm_response_raw: JSON.stringify({
                suggested_type: '投资',
                suggested_category_id: 55,
                suggested_main_category: '投资交易',
                suggested_sub_category: '基金买入'
            })
        });

        expect(signal).toMatchObject({
            reviewStatus: 'pending',
            suggestedType: '投资',
            suggestedCategoryId: 55,
            suggestedMainCategory: '投资交易',
            suggestedSubCategory: '基金买入'
        });
    });

    test('falls back to an empty raw suggestion when llm_response_raw is invalid json', () => {
        const signal = parseLLMMemoryEventSignal({
            preview_id: 7,
            llm_response_raw: '{not-json'
        });

        expect(signal).toStrictEqual({
            reviewStatus: 'pending',
            suppressed: false,
            suggestedMainCategory: '',
            suggestedSubCategory: '',
            suggestedSourceAccount: '',
            suggestedDestinationAccount: '',
            confidence: 0,
            reason: ''
        });
    });

    test('prefers reviewed feedback over pending recommendation when the same preview appears twice', () => {
        const memoryMap = buildLLMSignalMemoryMap([
            {
                id: 10,
                preview_id: 9,
                llm_response_raw: JSON.stringify({
                    suggested_main_category: '餐饮',
                    suggested_sub_category: '咖啡',
                    confidence: 0.82,
                    reason: '初始推荐'
                })
            },
            {
                id: 11,
                preview_id: 9,
                decision: 'accept',
                llm_response_raw: JSON.stringify({
                    suggested_main_category: '餐饮',
                    suggested_sub_category: '咖啡',
                    confidence: 0.82,
                    reason: '用户接受'
                })
            }
        ]);

        expect(memoryMap.get(9)).toMatchObject({
            reviewStatus: 'accepted',
            suppressed: false,
            suggestedMainCategory: '餐饮',
            suggestedSubCategory: '咖啡'
        });
    });

    test('keeps the first reviewed state when a later pending recommendation should not downgrade it', () => {
        const current = {
            reviewStatus: 'accepted' as const,
            suppressed: false,
            suggestedMainCategory: '餐饮',
            suggestedSubCategory: '咖啡',
            suggestedSourceAccount: '',
            suggestedDestinationAccount: '',
            confidence: 0.8,
            reason: 'accepted'
        };
        const next = {
            reviewStatus: 'pending' as const,
            suppressed: false,
            suggestedMainCategory: '餐饮',
            suggestedSubCategory: '咖啡',
            suggestedSourceAccount: '',
            suggestedDestinationAccount: '',
            confidence: 0.8,
            reason: 'pending'
        };

        expect(shouldReplaceLLMSignalMemoryState(current, next)).toBe(false);
    });

    test('ignores invalid preview ids during memory-map reduction and treats blank states as lowest priority', () => {
        const memoryMap = buildLLMSignalMemoryMap([
            {
                preview_id: 0,
                decision: 'accept'
            },
            {
                preview_id: 21,
                llm_response_raw: JSON.stringify({
                    suggested_main_category: '餐饮'
                })
            }
        ]);

        expect(memoryMap.has(0)).toBe(false);
        expect(memoryMap.get(21)?.reviewStatus).toBe('pending');
        expect(shouldReplaceLLMSignalMemoryState(
            {
                reviewStatus: 'pending',
                suppressed: false,
                suggestedMainCategory: '',
                suggestedSubCategory: '',
                suggestedSourceAccount: '',
                suggestedDestinationAccount: '',
                confidence: 0,
                reason: ''
            },
            {
                reviewStatus: '',
                suppressed: false,
                suggestedMainCategory: '',
                suggestedSubCategory: '',
                suggestedSourceAccount: '',
                suggestedDestinationAccount: '',
                confidence: 0,
                reason: ''
            }
        )).toBe(false);
    });
});
