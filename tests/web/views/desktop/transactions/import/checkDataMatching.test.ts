import { describe, expect, test } from '@jest/globals';

import {
    buildImportPreviewSignalViewModel,
    buildImportPreviewTypeColumnViewModel,
    formatInvestmentSignalReason,
    getImportCheckMatchingContextSummary,
    getImportCheckMatchingDedupLabel,
    getImportCheckMatchingDedupTitle,
    getImportCheckMatchingParserTagsText,
    hasImportCheckMatchingDedupContext,
    hasImportCheckMatchingContext,
    resolveImportPreviewInvestmentDecisionState,
    shouldShowImportCheckMatchingDedupSourceCount
} from '@/views/desktop/transactions/import/checkDataMatching.ts';

describe('checkDataMatching helpers', () => {
    test('builds matching context summaries for parser, dedup, and manual annotation display', () => {
        const summary = getImportCheckMatchingContextSummary({
            parserSource: 'alipay',
            parserTags: ['parser:alipay', 'channel:wallet'],
            dedupType: 'transfer',
            dedupSourceIds: [101, 102],
            isManuallyAnnotated: true
        });

        expect(summary).toStrictEqual({
            parserId: 'alipay',
            parserTags: ['parser:alipay', 'channel:wallet'],
            dedupType: 'transfer',
            dedupSourceIds: [101, 102],
            isManuallyAnnotated: true
        });
        expect(hasImportCheckMatchingContext(summary)).toBe(true);
        expect(getImportCheckMatchingDedupLabel(summary)).toBe('Transfer Match');
        expect(getImportCheckMatchingDedupTitle(summary, {
            matchLabel: '匹配',
            parserLabels: {
                alipay: '支付宝',
                wechat: '微信'
            },
            sourceRows: [
                { id: 101, parserSource: 'wechat' },
                { id: 102, parserSource: 'alipay' }
            ]
        })).toBe('匹配 | 支付宝 | 微信');
        expect(getImportCheckMatchingParserTagsText(summary)).toBe('parser:alipay · channel:wallet');
    });

    test('does not surface remaining dedup rows as matching context by themselves', () => {
        const summary = getImportCheckMatchingContextSummary({
            parserSource: '',
            parserTags: [],
            dedupType: 'remaining',
            dedupSourceIds: [201],
            isManuallyAnnotated: false
        });

        expect(hasImportCheckMatchingContext(summary)).toBe(false);
    });

    test('resolves transfer dedup parser labels from a prebuilt lookup keyed by row index or preview id', () => {
        const summary = getImportCheckMatchingContextSummary({
            parserSource: 'bank',
            dedupType: 'transfer',
            dedupSourceIds: [12, 'preview:9']
        });

        expect(getImportCheckMatchingDedupTitle(summary, {
            matchLabel: '匹配',
            parserLabels: {
                bank: '银行卡',
                wechat: '微信',
                alipay: '支付宝'
            },
            sourceRowLookup: new Map([
                ['12', 'wechat'],
                ['preview:9', 'alipay']
            ])
        })).toBe('匹配 | 银行卡 | 微信 | 支付宝');
    });

    test('resolves transfer dedup parser labels from source-row parser tags', () => {
        const summary = getImportCheckMatchingContextSummary({
            parserSource: 'bank',
            dedupType: 'transfer',
            dedupSourceIds: [12, 'preview:9']
        });

        expect(getImportCheckMatchingDedupTitle(summary, {
            matchLabel: '匹配',
            parserLabels: {
                bank: '银行卡',
                wechat: '微信',
                cmbc: '民生银行',
                alipay: '支付宝',
                abc: '农业银行'
            },
            sourceRowLookup: new Map([
                ['12', { parserSource: 'wechat', parserTags: ['parser:wechat', 'parser:cmbc'] }],
                ['preview:9', { parserSource: 'alipay', parserTags: ['parser:alipay', 'parser:abc'] }]
            ])
        })).toBe('匹配 | 银行卡 | 微信 | 民生银行 | 支付宝 | 农业银行');
    });

    test('resolves transfer dedup parser labels from combined parser tags', () => {
        const summary = getImportCheckMatchingContextSummary({
            parserSource: 'cmbc',
            parserTags: ['parser:cmbc', 'parser:alipay'],
            dedupType: 'transfer',
            dedupSourceIds: [701, 702]
        });

        expect(getImportCheckMatchingDedupTitle(summary, {
            matchLabel: '匹配',
            parserLabels: {
                cmbc: '民生银行',
                alipay: '支付宝'
            }
        })).toBe('匹配 | 民生银行 | 支付宝');
    });

    test('surfaces non-transfer dedup rows with stable labels', () => {
        const summary = getImportCheckMatchingContextSummary({
            dedupType: 'platform_bank',
            dedupSourceIds: [301, 302]
        });

        expect(hasImportCheckMatchingDedupContext(summary)).toBe(true);
        expect(hasImportCheckMatchingContext(summary)).toBe(true);
        expect(getImportCheckMatchingDedupLabel(summary)).toBe('Platform Duplicate');
        const title = getImportCheckMatchingDedupTitle(summary, {
            dedupLabels: {
                'Platform Duplicate': '平台重复'
            }
        });
        expect(title).toBe('平台重复');
        expect(shouldShowImportCheckMatchingDedupSourceCount(summary.dedupType)).toBe(false);
        expect(title).not.toContain('platform_bank');
        expect(title).not.toContain('301');
        expect(title).not.toContain('302');
    });

    test('maps split dedup aliases to the split-merge label', () => {
        const summary = getImportCheckMatchingContextSummary({
            dedupType: 'split',
            dedupSourceIds: [401]
        });

        expect(hasImportCheckMatchingDedupContext(summary)).toBe(true);
        expect(getImportCheckMatchingDedupLabel(summary)).toBe('Split-Merge Duplicate');
    });

    test('builds compact signal cell model without exposing raw parser tags in body labels', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            parserSource: 'alipay',
            parserTags: ['parser:alipay', 'channel:wallet'],
            dedupType: 'transfer',
            dedupSourceIds: [9],
            isManuallyAnnotated: true,
            investmentStatus: 'pending',
            investmentTitle: 'investment candidate'
        }, {
            matchLabel: '匹配',
            parserLabels: {
                alipay: '支付宝',
                wechat: '微信'
            },
            parserColors: {
                alipay: 'blue'
            },
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            },
            sourceRows: [
                { id: 9, parserSource: 'wechat' }
            ]
        });

        expect(viewModel.parser).toStrictEqual({
            parserId: 'alipay',
            label: '支付宝',
            color: 'blue',
            title: '来源：支付宝',
            detailLines: ['来源：支付宝']
        });
        expect(viewModel.dedup?.labelKey).toBe('Transfer Match');
        expect(viewModel.dedup?.title).toBe('匹配 | 支付宝 | 微信');
        expect(viewModel.isManuallyAnnotated).toBe(true);
        expect(viewModel.investment?.labelKey).toBe('Investment Signal');
        expect(viewModel.investment?.actions).toStrictEqual([]);
    });

    test('keeps transfer and learning review actions while investment stays signal-only', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            transferStatus: 'pending',
            investmentStatus: 'pending',
            learningStatus: 'pending'
        });

        expect(viewModel.transferSuggestion?.actions.map(action => action.labelKey)).toStrictEqual([
            'Apply Suggestion',
            'Reject Transfer Suggestion'
        ]);
        expect(viewModel.learning?.actions.map(action => action.labelKey)).toStrictEqual([
            'Apply Suggestion',
            'Reject Learning Suggestion'
        ]);
        expect(viewModel.investment?.actions).toStrictEqual([]);
    });

    test('formats transfer details from structured source metadata and hides redundant parser chip', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            parserSource: 'cmbc',
            parserTags: ['parser:cmbc', 'parser:wechat'],
            parserSourceChain: [
                {
                    position: 0,
                    role: 'outgoing',
                    parser_id: 'cmbc',
                    parser_label: '民生银行',
                    label: '民生银行卡'
                },
                {
                    position: 1,
                    role: 'incoming',
                    parser_id: 'wechat',
                    parser_label: '微信',
                    label: '微信'
                }
            ],
            transferStatus: 'pending',
            transferPairOrder: 'outgoing_first',
            transferSourceChain: [
                {
                    position: 1,
                    role: 'incoming',
                    parser_id: 'wechat',
                    parser_label: '微信',
                    label: '微信'
                },
                {
                    position: 0,
                    role: 'outgoing',
                    parser_id: 'cmbc',
                    parser_label: '民生银行',
                    label: '民生银行卡'
                }
            ]
        }, {
            sourceRoleLabels: {
                outgoing: '转出',
                incoming: '转入'
            }
        });

        expect(viewModel.parser).toBeNull();
        expect(viewModel.transferSuggestion?.detailLines).toStrictEqual([
            '转出：民生银行卡',
            '转入：微信'
        ]);
        expect(viewModel.transferSuggestion?.title).toBe('转出：民生银行卡 | 转入：微信');
    });

    test('formats platform duplicate label and localized duplicate-source detail from structured metadata', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            dedupType: 'platform_bank',
            dedupSourceIds: [301, 302],
            dedupSourceCount: 2,
            dedupSourceLabels: ['支付宝', '民生银行'],
            dedupSources: [
                {
                    position: 0,
                    role: 'kept',
                    parser_id: 'alipay',
                    parser_label: '支付宝',
                    label: '支付宝'
                },
                {
                    position: 1,
                    role: 'duplicate',
                    parser_id: 'cmbc',
                    parser_label: '民生银行',
                    label: '民生银行'
                }
            ]
        }, {
            dedupLabels: {
                'Platform Duplicate': '平台重复'
            },
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            }
        });

        expect(viewModel.dedup?.label).toBe('平台重复·2');
        expect(viewModel.dedup?.detailLines).toStrictEqual([
            '重复来源：支付宝 · 民生银行'
        ]);
    });

    test('keeps investment profile in hover title and maps reason keys for display', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            investmentStatus: 'pending',
            investmentTitle: 'platform:蚂蚁财富, product:黄金ETF',
            investmentProfileText: '蚂蚁财富 黄金ETF'
        });

        expect(viewModel.investment?.title).toBe('Platform: 蚂蚁财富, Product: 黄金ETF | 蚂蚁财富 黄金ETF');
        expect(viewModel.investment?.profileText).toBe('蚂蚁财富 黄金ETF');
    });

    test('formats learning detail lines with the user-facing recommended-category summary', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            learningStatus: 'pending',
            learningTitle: 'parser_id:exact',
            learningSummary: '支出 | 餐饮/咖啡 | 招商银行卡 → 支付宝'
        }, {
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            }
        });

        expect(viewModel.learning?.detailLines).toStrictEqual([
            '推荐分类：支出 / 餐饮/咖啡',
            '账户链路：招商银行卡 → 支付宝'
        ]);
        expect(viewModel.learning?.title).toBe('推荐分类：支出 / 餐饮/咖啡 | 账户链路：招商银行卡 → 支付宝');
    });

    test('removes learning clear actions after review states', () => {
        const acceptedViewModel = buildImportPreviewSignalViewModel({
            learningStatus: 'accepted',
            learningSummary: '支出 | 餐饮/咖啡'
        }, {
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            }
        });

        expect(acceptedViewModel.learning?.actions).toStrictEqual([]);
        expect(acceptedViewModel.learning?.detailLines).toStrictEqual([
            '推荐分类：支出 / 餐饮/咖啡'
        ]);
    });

    test('formats investment reason keys with caller-provided labels', () => {
        expect(formatInvestmentSignalReason('platform:蚂蚁财富, product:黄金ETF', {
            platform: 'Platform',
            product: 'Product'
        })).toBe('Platform: 蚂蚁财富, Product: 黄金ETF');
    });

    test('resolves investment decision state from minimal action payloads', () => {
        expect(resolveImportPreviewInvestmentDecisionState('accept', {
            reviewStatus: 'accepted',
            suppressed: false
        })).toStrictEqual({
            reviewStatus: 'accepted',
            suppressed: false
        });
        expect(resolveImportPreviewInvestmentDecisionState('reject', {
            reviewStatus: 'rejected',
            suppressed: true
        })).toStrictEqual({
            reviewStatus: 'rejected',
            suppressed: true
        });
        expect(resolveImportPreviewInvestmentDecisionState('clear', {
            reviewStatus: 'pending',
            suppressed: false
        })).toStrictEqual({
            reviewStatus: 'pending',
            suppressed: false
        });
        expect(resolveImportPreviewInvestmentDecisionState('reject')).toStrictEqual({
            reviewStatus: 'rejected',
            suppressed: true
        });
    });

    test('keeps manual annotation out of visible signals when it is the only context', () => {
        const summary = getImportCheckMatchingContextSummary({
            isManuallyAnnotated: true
        });
        const viewModel = buildImportPreviewSignalViewModel({
            isManuallyAnnotated: true
        });

        expect(summary.isManuallyAnnotated).toBe(true);
        expect(hasImportCheckMatchingContext(summary)).toBe(false);
        expect(viewModel.isManuallyAnnotated).toBe(true);
        expect(viewModel.hasAnySignal).toBe(false);
    });

    test('keeps parser and investment signals out of the type column model', () => {
        const typeColumn = buildImportPreviewTypeColumnViewModel(5);

        expect(typeColumn).toStrictEqual({
            type: 5,
            signalKeys: []
        });
    });

    test('returns false when parser, dedup, and manual annotation context are all empty', () => {
        const summary = getImportCheckMatchingContextSummary({});

        expect(summary).toStrictEqual({
            parserId: '',
            parserTags: [],
            dedupType: '',
            dedupSourceIds: [],
            isManuallyAnnotated: false
        });
        expect(hasImportCheckMatchingContext(summary)).toBe(false);
        expect(getImportCheckMatchingDedupTitle(summary)).toBe('');
        expect(getImportCheckMatchingParserTagsText(summary)).toBe('');
    });
});
