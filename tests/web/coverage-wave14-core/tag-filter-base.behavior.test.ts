/* eslint-disable @typescript-eslint/no-explicit-any */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockUpdateStatisticsFilter = jest.fn();
const mockUpdateStatisticsInvalid = jest.fn();
const mockUpdateTransactionFilter = jest.fn();
const mockUpdateTransactionInvalid = jest.fn();

const visibleTag = { id: 'tag-visible', name: 'Visible', hidden: false };
const hiddenTag = { id: 'tag-hidden', name: 'Hidden', hidden: true };
const mockTagStore = actualVue.reactive({
    allTransactionTags: [visibleTag, hiddenTag],
    allTransactionTagsMap: {
        'tag-visible': visibleTag,
        'tag-hidden': hiddenTag,
    } as Record<string, any>,
    allAvailableTagsCount: 2,
    allVisibleTagsCount: 1,
});
const mockTransactionsStore = actualVue.reactive({
    allFilterTagIds: { 'tag-visible': true, missing: true } as Record<string, boolean>,
    updateTransactionListFilter: mockUpdateTransactionFilter,
    updateTransactionListInvalidState: mockUpdateTransactionInvalid,
});
const mockStatisticsStore = actualVue.reactive({
    transactionStatisticsFilter: {
        tagIds: 'tag-hidden,missing',
        tagFilterType: 7,
    },
    updateTransactionStatisticsFilter: mockUpdateStatisticsFilter,
    updateTransactionStatisticsInvalidState: mockUpdateStatisticsInvalid,
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        getAllTransactionTagFilterTypes: () => [{ type: 0, displayName: 'Default' }],
    }),
}));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/stores/statistics.ts', () => ({ useStatisticsStore: () => mockStatisticsStore }));
jest.mock('@/core/transaction.ts', () => ({ TransactionTagFilterType: { Default: { type: 0 } } }));

import { useTransactionTagFilterSettingPageBase } from '@/views/base/settings/TransactionTagFilterSettingPageBase.ts';

beforeEach(() => {
    jest.clearAllMocks();
    mockUpdateStatisticsFilter.mockReturnValue(true);
    mockUpdateTransactionFilter.mockReturnValue(true);
    mockTagStore.allAvailableTagsCount = 2;
    mockTagStore.allVisibleTagsCount = 1;
    mockStatisticsStore.transactionStatisticsFilter.tagIds = 'tag-hidden,missing';
    mockStatisticsStore.transactionStatisticsFilter.tagFilterType = 7;
    mockTransactionsStore.allFilterTagIds = { 'tag-visible': true, missing: true };
});

describe('TransactionTagFilterSettingPageBase production behavior', () => {
    test('projects labels, options, tags, and visible/hidden availability', () => {
        const base = useTransactionTagFilterSettingPageBase('statisticsCurrent');
        expect(base.loading.value).toBe(true);
        expect(base.title.value).toBe('Filter Transaction Tags');
        expect(base.applyText.value).toBe('Apply');
        expect(base.allTags.value).toEqual([visibleTag, hiddenTag]);
        expect(base.allTagFilterTypes.value).toEqual([{ type: 0, displayName: 'Default' }]);
        expect(base.hasAnyAvailableTag.value).toBe(true);
        expect(base.hasAnyVisibleTag.value).toBe(true);

        mockTagStore.allVisibleTagsCount = 0;
        expect(base.hasAnyVisibleTag.value).toBe(false);
        base.showHidden.value = true;
        expect(base.hasAnyVisibleTag.value).toBe(true);
        mockTagStore.allAvailableTagsCount = 0;
        expect(base.hasAnyAvailableTag.value).toBe(false);
        expect(base.hasAnyVisibleTag.value).toBe(false);
    });

    test('loads the statistics filter while ignoring missing tag identities', () => {
        const base = useTransactionTagFilterSettingPageBase('statisticsCurrent');
        expect(base.loadFilterTagIds()).toBe(true);
        expect(base.filterTagIds.value).toEqual({
            'tag-visible': true,
            'tag-hidden': false,
        });
        expect(base.tagFilterType.value).toBe(7);

        mockStatisticsStore.transactionStatisticsFilter.tagIds = '';
        const empty = useTransactionTagFilterSettingPageBase('statisticsCurrent');
        expect(empty.loadFilterTagIds()).toBe(true);
        expect(empty.filterTagIds.value).toEqual({ 'tag-visible': true, 'tag-hidden': true });
    });

    test('loads transaction-list selections from true map entries only', () => {
        const base = useTransactionTagFilterSettingPageBase('transactionListCurrent');
        expect(base.loadFilterTagIds()).toBe(true);
        expect(base.filterTagIds.value).toEqual({
            'tag-visible': false,
            'tag-hidden': true,
        });
    });

    test('rejects unsupported filter contexts without mutating selections', () => {
        const base = useTransactionTagFilterSettingPageBase('unsupported');
        expect(base.loadFilterTagIds()).toBe(false);
        expect(base.filterTagIds.value).toEqual({});
        expect(base.saveFilterTagIds()).toBe(true);
        expect(mockUpdateStatisticsFilter).not.toHaveBeenCalled();
        expect(mockUpdateTransactionFilter).not.toHaveBeenCalled();
    });

    test('saves canonical statistics ids and invalidates only changed filters', () => {
        const base = useTransactionTagFilterSettingPageBase('statisticsCurrent');
        base.filterTagIds.value = {
            'tag-visible': false,
            'tag-hidden': false,
            missing: false,
        };
        base.tagFilterType.value = 9;
        expect(base.saveFilterTagIds()).toBe(true);
        expect(mockUpdateStatisticsFilter).toHaveBeenCalledWith({
            tagIds: 'tag-visible,tag-hidden',
            tagFilterType: 9,
        });
        expect(mockUpdateStatisticsInvalid).toHaveBeenCalledWith(true);

        mockUpdateStatisticsFilter.mockReturnValueOnce(false);
        expect(base.saveFilterTagIds()).toBe(false);
        expect(mockUpdateStatisticsInvalid).toHaveBeenCalledTimes(1);
    });

    test('saves transaction-list ids, preserves selected tags, and handles unchanged state', () => {
        const base = useTransactionTagFilterSettingPageBase('transactionListCurrent');
        base.filterTagIds.value = {
            'tag-visible': true,
            'tag-hidden': false,
        };
        expect(base.saveFilterTagIds()).toBe(true);
        expect(mockUpdateTransactionFilter).toHaveBeenCalledWith({ tagIds: 'tag-hidden' });
        expect(mockUpdateTransactionInvalid).toHaveBeenCalledWith(true);

        mockUpdateTransactionFilter.mockReturnValueOnce(false);
        expect(base.saveFilterTagIds()).toBe(false);
        expect(mockUpdateTransactionInvalid).toHaveBeenCalledTimes(1);
    });
});
