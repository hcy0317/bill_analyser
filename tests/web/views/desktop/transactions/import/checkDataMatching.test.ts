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
    resolveImportPreviewInvestmentDecisionState
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
        expect(getImportCheckMatchingDedupLabel(summary)).toBe('Platform-Bank Duplicate');
        const title = getImportCheckMatchingDedupTitle(summary, {
            dedupLabels: {
                'Platform-Bank Duplicate': '平台-银行重复'
            }
        });
        expect(title).toBe('平台-银行重复 · 2');
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
            sourceRows: [
                { id: 9, parserSource: 'wechat' }
            ]
        });

        expect(viewModel.parser).toStrictEqual({
            parserId: 'alipay',
            label: '支付宝',
            color: 'blue',
            title: 'parser:alipay · channel:wallet'
        });
        expect(viewModel.dedup?.labelKey).toBe('Transfer Match');
        expect(viewModel.dedup?.title).toBe('匹配 | 支付宝 | 微信');
        expect(viewModel.isManuallyAnnotated).toBe(true);
        expect(viewModel.investment?.labelKey).toBe('Investment Signal');
        expect(viewModel.investment?.actions.map(action => action.labelKey)).toStrictEqual(['Accept', 'Reject']);
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
