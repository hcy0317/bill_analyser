import { describe, expect, test } from '@jest/globals';

import {
    buildImportPreviewHistoryRewriteAcknowledgement,
    buildImportPreviewHistoryRewriteOperationAcknowledgement,
    buildImportPreviewSignalViewModel,
    buildImportPreviewTypeColumnViewModel,
    getImportCheckMatchingContextSummary,
    getImportCheckMatchingDedupLabel,
    getImportCheckMatchingDedupTitle,
    getImportCheckMatchingParserTagsText,
    hasImportCheckMatchingDedupContext,
    hasImportCheckMatchingContext,
    matchesImportPreviewSignalFilter,
    resolveImportCheckMatchingTransferParserIds,
    shouldShowImportCheckMatchingDedupSourceCount
} from '@/views/desktop/transactions/import/checkDataMatching.ts';

describe('checkDataMatching helpers', () => {
    test('builds matching context summaries for parser, dedup, and manual annotation display', () => {
        const summary = getImportCheckMatchingContextSummary({
            parserId: 'alipay',
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
                { id: 101, parserId: 'wechat' },
                { id: 102, parserId: 'alipay' }
            ]
        })).toBe('匹配 | 支付宝 | 微信');
        expect(getImportCheckMatchingParserTagsText(summary)).toBe('parser:alipay · channel:wallet');
    });

    test('does not surface remaining dedup rows as matching context by themselves', () => {
        const summary = getImportCheckMatchingContextSummary({
            parserId: '',
            parserTags: [],
            dedupType: 'remaining',
            dedupSourceIds: [201],
            isManuallyAnnotated: false
        });

        expect(hasImportCheckMatchingContext(summary)).toBe(false);
    });

    test('resolves transfer dedup parser labels from a prebuilt lookup keyed by row index or preview id', () => {
        const summary = getImportCheckMatchingContextSummary({
            parserId: 'bank',
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
            parserId: 'bank',
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
                ['12', { parserId: 'wechat', parserTags: ['parser:wechat', 'parser:cmbc'] }],
                ['preview:9', { parserId: 'alipay', parserTags: ['parser:alipay', 'parser:abc'] }]
            ])
        })).toBe('匹配 | 银行卡 | 微信 | 民生银行 | 支付宝 | 农业银行');
    });

    test('resolves transfer dedup parser labels from combined parser tags', () => {
        const summary = getImportCheckMatchingContextSummary({
            parserId: 'cmbc',
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
        expect(shouldShowImportCheckMatchingDedupSourceCount('transfer')).toBe(false);
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

    test('humanizes unknown dedup types when no canonical label exists', () => {
        const summary = getImportCheckMatchingContextSummary({
            dedupType: 'manual_merge_case',
            dedupSourceIds: [501]
        });

        expect(getImportCheckMatchingDedupLabel(summary)).toBe('Manual Merge Case');
        expect(getImportCheckMatchingDedupTitle(summary)).toBe('Manual Merge Case');
    });

    test('builds compact signal cell model without exposing raw parser tags in body labels', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            parserId: 'alipay',
            parserTags: ['parser:alipay', 'channel:wallet'],
            dedupType: 'transfer',
            dedupSourceIds: [9],
            isManuallyAnnotated: true
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
                { id: 9, parserId: 'wechat' }
            ]
        });

        expect(viewModel.parser).toBeNull();
        expect(viewModel.dedup?.labelKey).toBe('Transfer Match');
        expect(viewModel.dedup?.title).toBe('匹配 | 支付宝 | 微信');
        expect(viewModel.isManuallyAnnotated).toBe(true);
        expect(viewModel.investment).toBeNull();
    });

    test('keeps transfer and learning review actions without rendering investment signal chips', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            transferStatus: 'pending',
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
        expect(viewModel.learning?.color).toBe('warning');
        expect(viewModel.investment).toBeNull();
    });

    test('builds yellow llm suggestion signals with warning review actions and detail lines', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            llmStatus: 'pending',
            llmTitle: '根据历史记忆推荐',
            llmSummary: '餐饮/咖啡 | 招商银行→支付宝',
            llmConfidence: 0.87,
            llmCategoryPath: '餐饮/咖啡',
            llmSourceAccount: '招商银行',
            llmDestinationAccount: '支付宝'
        }, {
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            }
        });

        expect(viewModel.llm?.labelKey).toBe('LLM Suggestion');
        expect(viewModel.llm?.color).toBe('warning');
        expect(viewModel.llm?.actions.map(action => action.labelKey)).toStrictEqual([
            'Apply Suggestion',
            'Reject LLM Suggestion'
        ]);
        expect(viewModel.llm?.detailLines).toContain('推荐：餐饮-咖啡');
        expect(viewModel.llm?.detailLines).toContain('账户链路：招商银行→支付宝');
        expect(viewModel.llm?.detailLines).toContain('Confidence: 87%');
        expect(viewModel.hasAnySignal).toBe(true);
    });

    test('formats transfer details from structured source metadata and hides redundant parser chip', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            parserId: 'cmbc',
            parserTags: ['parser:cmbc', 'parser:wechat'],
            parserIdChain: [
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

    test('falls back to parser labels and parser ids for transfer source details when explicit labels are missing', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            transferStatus: 'pending',
            transferPairOrder: 'outgoing_first',
            transferSourceChain: [
                {
                    position: 0,
                    role: 'outgoing',
                    parser_id: 'cmbc',
                    parser_label: '民生银行'
                },
                {
                    position: 1,
                    role: 'other',
                    parser_id: 'wechat'
                }
            ]
        }, {
            parserLabels: {
                wechat: '微信'
            },
            sourceRoleLabels: {
                outgoing: '转出'
            }
        });

        expect(viewModel.transferSuggestion?.detailLines).toStrictEqual([
            '转出：民生银行',
            '微信'
        ]);
    });

    test('sorts known transfer source roles ahead of unknown roles', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            transferStatus: 'pending',
            transferPairOrder: 'outgoing_first',
            transferSourceChain: [
                {
                    position: 0,
                    role: 'other',
                    parser_id: 'wechat'
                },
                {
                    position: 1,
                    role: 'outgoing',
                    parser_label: '民生银行'
                }
            ]
        }, {
            parserLabels: {
                wechat: '微信'
            },
            sourceRoleLabels: {
                outgoing: '转出'
            }
        });

        expect(viewModel.transferSuggestion?.detailLines).toStrictEqual([
            '转出：民生银行',
            '微信'
        ]);
    });

    test('hides parser chip when platform duplicate is present', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            parserId: 'alipay',
            parserTags: ['parser:alipay'],
            dedupType: 'platform_bank',
            dedupSourceIds: [301],
            dedupSourceLabels: ['支付宝']
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

        expect(viewModel.parser).toBeNull();
        expect(viewModel.dedup?.label).toBe('平台重复');
    });

    test('hides parser chip when transfer match dedup is present', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            parserId: 'cmbc',
            parserTags: ['parser:cmbc'],
            dedupType: 'transfer',
            dedupSourceIds: [11, 12]
        }, {
            parserLabels: {
                cmbc: '民生银行'
            }
        });

        expect(viewModel.parser).toBeNull();
        expect(viewModel.dedup?.labelKey).toBe('Transfer Match');

        const crossBatchViewModel = buildImportPreviewSignalViewModel({
            parserId: 'cmbc',
            parserTags: ['parser:cmbc'],
            dedupType: 'transfer_cross_batch',
            dedupSourceIds: [21, 22]
        }, {
            parserLabels: {
                cmbc: '民生银行'
            }
        });

        expect(crossBatchViewModel.parser).toBeNull();
        expect(crossBatchViewModel.dedup?.labelKey).toBe('Cross-Batch Transfer');
    });

    test('matches signal filters by the same visible signal families as the signal cell', () => {
        const parserOnly = buildImportPreviewSignalViewModel({
            parserId: 'alipay',
            parserTags: ['parser:alipay']
        });
        const platformDuplicate = buildImportPreviewSignalViewModel({
            parserId: 'alipay',
            parserTags: ['parser:alipay'],
            dedupType: 'platform_bank',
            dedupSourceIds: [301]
        });
        const transferMatch = buildImportPreviewSignalViewModel({
            parserId: 'cmbc',
            parserTags: ['parser:cmbc'],
            dedupType: 'transfer',
            dedupSourceIds: [11, 12]
        });
        const historyRewrite = buildImportPreviewSignalViewModel({
            parserId: 'abc',
            reconciliationPlannedOperation: 'update_history',
            reconciliationHistoryBillId: 9,
            reconciliationOperationId: 'history:9',
            reconciliationAcknowledgementToken: 'ack-9',
            reconciliationDestructiveAckRequired: true
        });
        const learning = buildImportPreviewSignalViewModel({
            parserId: 'wechat',
            learningStatus: 'pending',
            learningSummary: '支出 | 食品饮料/外卖'
        });
        const llm = buildImportPreviewSignalViewModel({
            parserId: 'wechat',
            llmStatus: 'pending',
            llmTitle: 'LLM'
        });

        expect(matchesImportPreviewSignalFilter(parserOnly, 'parser')).toBe(true);
        expect(matchesImportPreviewSignalFilter(platformDuplicate, 'parser')).toBe(false);
        expect(matchesImportPreviewSignalFilter(transferMatch, 'parser')).toBe(false);
        expect(matchesImportPreviewSignalFilter(historyRewrite, 'parser')).toBe(false);
        expect(matchesImportPreviewSignalFilter(learning, 'parser')).toBe(false);
        expect(matchesImportPreviewSignalFilter(llm, 'parser')).toBe(false);

        expect(matchesImportPreviewSignalFilter(platformDuplicate, 'platform_duplicate')).toBe(true);
        expect(matchesImportPreviewSignalFilter(parserOnly, 'platform_duplicate')).toBe(false);
        expect(matchesImportPreviewSignalFilter(transferMatch, 'transfer')).toBe(true);
        expect(matchesImportPreviewSignalFilter(historyRewrite, 'history')).toBe(true);
        expect(matchesImportPreviewSignalFilter(learning, 'learning')).toBe(true);
        expect(matchesImportPreviewSignalFilter(llm, 'llm')).toBe(true);
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

        expect(viewModel.dedup?.label).toBe('平台重复');
        expect(viewModel.dedup?.detailLines).toStrictEqual([
            '重复来源：支付宝|民生银行'
        ]);
    });

    test('formats platform duplicate sources from current row and source lookup fallback', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            parserId: 'alipay',
            dedupType: 'platform_bank',
            dedupSourceIds: [301, 'preview:302']
        }, {
            parserLabels: {
                alipay: '支付宝',
                icbc: '工商银行',
                cmbc: '民生银行',
                wechat: '微信'
            },
            dedupLabels: {
                'Platform Duplicate': '平台重复'
            },
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            },
            sourceRowLookup: new Map<string, string | { parserId: string; parserTags: string[] }>([
                ['301', 'icbc'],
                ['preview:302', { parserId: 'cmbc', parserTags: ['parser:wechat'] }]
            ])
        });

        expect(viewModel.dedup?.detailLines).toStrictEqual([
            '重复来源：支付宝|工商银行|民生银行|微信'
        ]);
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
            '推荐：支出|餐饮-咖啡|招商银行卡→支付宝'
        ]);
        expect(viewModel.learning?.title).toBe('推荐：支出|餐饮-咖啡|招商银行卡→支付宝');
    });

    test('keeps single-account learning summaries as one recommended line', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            learningStatus: 'pending',
            learningSummary: '收入 | 投资收入/利息收入 | 工商银行 → -'
        }, {
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            }
        });

        expect(viewModel.learning?.detailLines).toStrictEqual([
            '推荐：收入|投资收入-利息收入|工商银行'
        ]);

        const reverseViewModel = buildImportPreviewSignalViewModel({
            learningStatus: 'pending',
            learningSummary: '收入 | 投资收入/利息收入 | - → 工商银行'
        }, {
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            }
        });

        expect(reverseViewModel.learning?.detailLines).toStrictEqual([
            '推荐：收入|投资收入-利息收入|工商银行'
        ]);
    });

    test('falls back cleanly when the recommended-category label is blank', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            learningStatus: 'pending',
            learningSummary: '支出 | 餐饮/咖啡'
        }, {
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: ' ',
                accountRouteLabel: '账户链路'
            }
        });

        expect(viewModel.learning?.detailLines).toStrictEqual([
            ': 支出|餐饮-咖啡'
        ]);
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
            '推荐：支出|餐饮-咖啡'
        ]);
    });

    test('renders yellow and green learning lifecycle review actions', () => {
        const yellowPending = buildImportPreviewSignalViewModel({
            learningStatus: 'pending',
            learningSignalState: 'yellow',
            learningSummary: '支出 | 餐饮/咖啡'
        }, {
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            }
        });
        const rejected = buildImportPreviewSignalViewModel({
            learningStatus: 'rejected',
            learningSummary: '支出 | 餐饮/咖啡'
        }, {
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            }
        });
        const acceptedGreen = buildImportPreviewSignalViewModel({
            learningStatus: 'accepted',
            learningSignalState: 'green',
            learningAutoApplied: true,
            learningSummary: '支出 | 餐饮/咖啡'
        }, {
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            }
        });

        expect(yellowPending.learning?.color).toBe('warning');
        expect(yellowPending.learning?.labelKey).toBe('Learning Suggestion');
        expect(yellowPending.learning?.actions.map(action => action.labelKey)).toStrictEqual([
            'Apply Suggestion',
            'Reject Learning Suggestion'
        ]);
        expect(acceptedGreen.learning?.color).toBe('success');
        expect(acceptedGreen.learning?.labelKey).toBe('Learning Applied');
        expect(acceptedGreen.learning?.actions).toStrictEqual([
            { decision: 'reject', labelKey: 'Reject Learning Suggestion', color: 'error' }
        ]);
        expect(rejected.learning?.labelKey).toBe('Learning Suggestion Rejected');
        expect(rejected.learning?.color).toBe('error');
    });

    test('renders skipped learning feedback without review actions', () => {
        const skipped = buildImportPreviewSignalViewModel({
            learningStatus: 'skipped',
            learningTitle: 'transfer preview is protected from learning type/category overrides'
        }, {
            infoLabels: {
                sourceLabel: '来源',
                duplicateSourcesLabel: '重复来源',
                recommendedCategoryLabel: '推荐分类',
                accountRouteLabel: '账户链路'
            }
        });

        expect(skipped.learning?.labelKey).toBe('Learning Suggestion Skipped');
        expect(skipped.learning?.color).toBe('warning');
        expect(skipped.learning?.actions).toStrictEqual([]);
        expect(skipped.learning?.title).toContain('transfer preview is protected');
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

    test('does not expose remaining dedup metadata as a visible empty signal', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            dedupType: 'remaining',
            dedupSourceIds: [201],
            dedupSourceCount: 1
        });

        expect(viewModel.dedup).toBeNull();
        expect(viewModel.hasAnySignal).toBe(false);
    });

    test('filters rows by visible signal family rather than raw backend fields', () => {
        const parserViewModel = buildImportPreviewSignalViewModel({
            parserId: 'alipay'
        });
        const platformDuplicateViewModel = buildImportPreviewSignalViewModel({
            parserId: 'alipay',
            dedupType: 'platform_bank',
            dedupSourceIds: [301, 302],
            dedupSourceLabels: ['支付宝', '民生银行']
        });
        const transferViewModel = buildImportPreviewSignalViewModel({
            dedupType: 'transfer',
            dedupSourceIds: [401, 402]
        });
        const learningViewModel = buildImportPreviewSignalViewModel({
            learningStatus: 'pending',
            learningSummary: '收入 | 其他收入/原路退款 | 民生银行'
        });
        const historyViewModel = buildImportPreviewSignalViewModel({
            reconciliationTitle: '将改写/合并历史账单',
            reconciliationPlannedOperation: 'update_history',
            reconciliationHistoryBillId: 88,
            reconciliationHistoryBillVersion: 3,
            reconciliationOperationId: 'history:abc',
            reconciliationAcknowledgementToken: 'ack-token',
            reconciliationDestructiveAckRequired: true
        });
        const llmViewModel = buildImportPreviewSignalViewModel({
            llmStatus: 'pending',
            llmTitle: 'LLM 推荐'
        });

        expect(matchesImportPreviewSignalFilter(parserViewModel, 'parser')).toBe(true);
        expect(matchesImportPreviewSignalFilter(platformDuplicateViewModel, 'parser')).toBe(false);
        expect(matchesImportPreviewSignalFilter(platformDuplicateViewModel, 'platform_duplicate')).toBe(true);
        expect(matchesImportPreviewSignalFilter(transferViewModel, 'transfer')).toBe(true);

        const crossBatchTransferViewModel = buildImportPreviewSignalViewModel({
            dedupType: 'transfer_cross_batch',
            dedupSourceIds: [501, 502]
        });
        expect(matchesImportPreviewSignalFilter(crossBatchTransferViewModel, 'transfer')).toBe(true);

        expect(matchesImportPreviewSignalFilter(historyViewModel, 'history')).toBe(true);
        expect(matchesImportPreviewSignalFilter(learningViewModel, 'learning')).toBe(true);
        expect(matchesImportPreviewSignalFilter(llmViewModel, 'llm')).toBe(true);
        expect(matchesImportPreviewSignalFilter(learningViewModel, null)).toBe(true);
        expect(matchesImportPreviewSignalFilter(learningViewModel, 'unexpected' as never)).toBe(false);
    });

    test('builds explicit history rewrite signal and acknowledgement payloads', () => {
        const state = {
            reconciliationTitle: '将改写/合并历史账单',
            reconciliationPlannedOperation: 'merge_transfer_history',
            reconciliationHistoryBillId: 901,
            reconciliationHistoryBillVersion: 4,
            reconciliationOperationId: 'history:merge',
            reconciliationAcknowledgementToken: 'ack-merge',
            reconciliationDestructiveAckRequired: true,
            reconciliationSourceChain: [
                { role: 'outgoing', parser_label: '民生银行', label: '民生银行卡' },
                { role: 'incoming', parser_label: '微信', label: '微信' }
            ]
        };
        const viewModel = buildImportPreviewSignalViewModel(state);

        expect(viewModel.dedup).toBeNull();
        expect(viewModel.historyRewrite?.labelKey).toBe('Merge History Transfer');
        expect(viewModel.historyRewrite?.color).toBe('warning');
        expect(viewModel.historyRewrite?.actions).toStrictEqual([]);
        expect(viewModel.historyRewrite?.detailLines).toContain('Operation: merge_transfer_history');
        expect(viewModel.historyRewrite?.detailLines).toContain('History Bill: #901 v4');

        const operation = buildImportPreviewHistoryRewriteOperationAcknowledgement(77, state);
        expect(operation).toStrictEqual({
            preview_id: 77,
            operation_id: 'history:merge',
            planned_operation: 'merge_transfer_history',
            history_bill_id: 901,
            history_bill_version: 4,
            acknowledgement_token: 'ack-merge'
        });

        const acknowledgement = buildImportPreviewHistoryRewriteAcknowledgement({
            selectedPreviewIds: [77, '78', 77],
            operations: [operation!],
            selectionScope: {
                mode: 'visible-preview'
            }
        });
        expect(acknowledgement).toStrictEqual({
            acknowledged: true,
            selected_preview_ids: [77, 78],
            operations: [operation!],
            selection_scope: {
                mode: 'visible-preview',
                selected_count: 2,
                history_rewrite_count: 1
            }
        });
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

    test('covers fallback review titles for reconciliation, recurring, transfer, learning, and llm signals', () => {
        const viewModel = buildImportPreviewSignalViewModel({
            transferStatus: 'accepted',
            transferTitle: '手工转账确认',
            learningStatus: 'accepted',
            learningTitle: '历史学习建议',
            llmStatus: 'accepted',
            llmTitle: 'LLM 兜底建议',
            reconciliationTitle: '历史归并候选',
            reconciliationStatus: 'rejected',
            hasRecurringMatch: true,
            recurringTitle: '每月账单',
            recurringCandidateCount: 2,
            recurringPrimaryReason: '金额相同'
        });

        expect(viewModel.dedup).toStrictEqual({
            dedupType: 'reconciliation',
            labelKey: 'reconciliation',
            label: '历史归并候选',
            title: '历史归并候选',
            color: 'error',
            sourceCount: 0,
            detailLines: ['历史归并候选']
        });
        expect(viewModel.transferSuggestion?.title).toBe('手工转账确认');
        expect(viewModel.transferSuggestion?.actions).toStrictEqual([
            { decision: 'clear', labelKey: 'Clear', color: 'warning' }
        ]);
        expect(viewModel.learning?.detailLines).toStrictEqual(['历史学习建议']);
        expect(viewModel.learning?.actions).toStrictEqual([]);
        expect(viewModel.llm?.detailLines).toStrictEqual(['LLM 兜底建议']);
        expect(viewModel.recurring).toStrictEqual({
            hasMatch: true,
            title: '每月账单',
            candidateCount: 2,
            primaryReason: '金额相同'
        });
        expect(matchesImportPreviewSignalFilter(viewModel, 'transfer')).toBe(true);
    });

    test('shows dedup fallback lines when source counts or source arrays make the signal visible', () => {
        const countOnlyViewModel = buildImportPreviewSignalViewModel({
            dedupType: 'platform_bank',
            dedupSourceCount: 1
        });
        const sourceArrayViewModel = buildImportPreviewSignalViewModel({
            dedupType: 'similar',
            dedupSources: [
                {
                    position: 0,
                    parser_id: 'alipay',
                    parser_label: '支付宝'
                }
            ]
        });

        expect(countOnlyViewModel.dedup?.detailLines).toStrictEqual(['Platform Duplicate']);
        expect(countOnlyViewModel.dedup?.sourceCount).toBe(1);
        expect(sourceArrayViewModel.dedup?.labelKey).toBe('Similar Duplicate');
        expect(sourceArrayViewModel.dedup?.detailLines).toStrictEqual(['Similar Duplicate']);
    });

    test('uses default parser-source fallbacks when optional options are omitted', () => {
        expect(resolveImportCheckMatchingTransferParserIds({
            parserId: 'cmbc',
            parserTags: [undefined as unknown as string, 'note:skip', 'parser:wechat'],
            dedupType: 'transfer',
            dedupSourceIds: ['11'],
            isManuallyAnnotated: false
        })).toStrictEqual(['cmbc', 'wechat']);
    });

    test('rejects incomplete history rewrite acknowledgements and omits empty acknowledgement payloads', () => {
        expect(buildImportPreviewHistoryRewriteOperationAcknowledgement(null, {
            reconciliationPlannedOperation: 'update_history',
            reconciliationHistoryBillId: { invalid: true } as unknown as string,
            reconciliationOperationId: 'history:update',
            reconciliationAcknowledgementToken: 'ack-update',
            reconciliationDestructiveAckRequired: true
        })).toBeNull();

        expect(buildImportPreviewHistoryRewriteOperationAcknowledgement(88, {
            reconciliationPlannedOperation: 'remaining',
            reconciliationHistoryBillId: 9,
            reconciliationOperationId: 'history:update',
            reconciliationAcknowledgementToken: 'ack-update',
            reconciliationDestructiveAckRequired: true
        })).toBeNull();

        expect(buildImportPreviewHistoryRewriteAcknowledgement({
            selectedPreviewIds: [88],
            operations: [],
            selectionScope: { mode: 'empty' }
        })).toBeNull();
    });

    test('sorts multiple history rewrite operations and preserves default bill versions', () => {
        const updateOperation = buildImportPreviewHistoryRewriteOperationAcknowledgement('99', {
            reconciliationPlannedOperation: 'update_history',
            reconciliationHistoryBillId: '700',
            reconciliationOperationId: 'history:update',
            reconciliationAcknowledgementToken: 'ack-update',
            reconciliationDestructiveAckRequired: true
        });
        const mergeOperation = buildImportPreviewHistoryRewriteOperationAcknowledgement(77, {
            reconciliationPlannedOperation: 'merge_transfer_history',
            reconciliationHistoryBillId: 701,
            reconciliationHistoryBillVersion: 2,
            reconciliationOperationId: 'history:merge',
            reconciliationAcknowledgementToken: 'ack-merge',
            reconciliationDestructiveAckRequired: true
        });

        expect(updateOperation?.history_bill_version).toBe(1);
        expect(buildImportPreviewHistoryRewriteAcknowledgement({
            selectedPreviewIds: [99, 77],
            operations: [updateOperation!, mergeOperation!],
            selectionScope: { mode: 'sorted' }
        })?.operations.map(operation => operation.preview_id)).toStrictEqual([77, 99]);
    });

    test('shows source counts for similar duplicate title fallbacks', () => {
        const summary = getImportCheckMatchingContextSummary({
            dedupType: 'similar',
            dedupSourceIds: [10, 11]
        });

        expect(getImportCheckMatchingDedupTitle(summary)).toBe('Similar Duplicate');
    });
});
