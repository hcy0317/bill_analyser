import { describe, expect, test } from '@jest/globals';

import {
    buildImportPreviewSignalViewModel,
    buildImportPreviewTypeColumnViewModel,
    getImportCheckMatchingContextSummary,
    getImportCheckMatchingDedupLabel,
    getImportCheckMatchingDedupTitle,
    getImportCheckMatchingParserTagsText,
    hasImportCheckMatchingDedupContext,
    hasImportCheckMatchingContext
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

    test('surfaces non-transfer dedup rows with stable labels', () => {
        const summary = getImportCheckMatchingContextSummary({
            dedupType: 'platform_bank',
            dedupSourceIds: [301, 302]
        });

        expect(hasImportCheckMatchingDedupContext(summary)).toBe(true);
        expect(hasImportCheckMatchingContext(summary)).toBe(true);
        expect(getImportCheckMatchingDedupLabel(summary)).toBe('Platform-Bank Duplicate');
        expect(getImportCheckMatchingDedupTitle(summary)).toBe('platform_bank | 301|302');
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
