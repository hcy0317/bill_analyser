import { describe, expect, jest, test } from '@jest/globals';

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

import { AccountCategory, AccountType } from '@/core/account.ts';
import { PARENT_ACCOUNT_CURRENCY_PLACEHOLDER } from '@/consts/currency.ts';
import { Account, type AccountInfoResponse } from '@/models/account.ts';
import {
    getAccountMapByName,
    getAllFilteredAccountsBalance,
    getCategorizedAccounts,
    getCategorizedAccountsMap,
    getCategorizedAccountsWithVisibleCount,
    getFinalAccountIdsByFilteredAccountIds,
    getUnifiedSelectedAccountsCurrencyOrDefaultCurrency,
    isAccountOrSubAccountsAllChecked,
    isAccountOrSubAccountsHasButNotAllChecked,
    selectAccountOrSubAccounts,
    selectAll,
    selectAllVisible,
    selectInvert,
    selectNone
} from '@/lib/account.ts';
import { evaluateExpressionToAmount } from '@/lib/evaluator.ts';

function accountResponse(overrides: Partial<AccountInfoResponse> = {}): AccountInfoResponse {
    return {
        id: 'cash',
        name: 'Cash',
        parentId: '0',
        category: AccountCategory.Cash.type,
        type: AccountType.SingleAccount.type,
        icon: '1',
        color: '#fff',
        currency: 'CNY',
        balanceCents: 1_000,
        comment: '',
        displayOrder: 1,
        hidden: false,
        ...overrides
    };
}

function account(overrides: Partial<AccountInfoResponse> = {}): Account {
    return Account.of(accountResponse(overrides));
}

function hierarchy(): Account[] {
    return [
        account({ id: 'cash', name: 'Cash', balanceCents: 1_000 }),
        account({ id: 'hidden', name: 'Hidden', hidden: true, balanceCents: 2_000 }),
        account({
            id: 'bank',
            name: 'Bank',
            category: AccountCategory.CheckingAccount.type,
            type: AccountType.MultiSubAccounts.type,
            currency: PARENT_ACCOUNT_CURRENCY_PLACEHOLDER,
            subAccounts: [
                accountResponse({
                    id: 'cny-sub',
                    name: 'CNY sub',
                    parentId: 'bank',
                    category: AccountCategory.CheckingAccount.type,
                    balanceCents: 3_000
                }),
                accountResponse({
                    id: 'usd-sub',
                    name: 'USD sub',
                    parentId: 'bank',
                    category: AccountCategory.CheckingAccount.type,
                    currency: 'USD',
                    balanceCents: -4_000,
                    hidden: true
                })
            ]
        })
    ];
}

describe('account collection logic', () => {
    test('categorizes known accounts in category order and ignores unknown categories', () => {
        const accounts = [...hierarchy(), account({ id: 'unknown', category: 999 })];
        const map = getCategorizedAccountsMap(accounts);

        expect(map[AccountCategory.Cash.type]?.accounts.map(item => item.id)).toEqual(['cash', 'hidden']);
        expect(map[AccountCategory.CheckingAccount.type]?.accounts.map(item => item.id)).toEqual(['bank']);
        expect(map[999]).toBeUndefined();
        expect(getCategorizedAccounts(accounts).map(item => item.category)).toEqual([
            AccountCategory.Cash.type,
            AccountCategory.CheckingAccount.type
        ]);
    });

    test('indexes single accounts and subaccounts by name', () => {
        expect(getAccountMapByName(hierarchy())).toMatchObject({
            Cash: { id: 'cash' },
            Hidden: { id: 'hidden' },
            'CNY sub': { id: 'cny-sub' },
            'USD sub': { id: 'usd-sub' }
        });
        expect(getAccountMapByName(null as unknown as Account[])).toEqual({});
        expect(getAccountMapByName([
            account({ id: 'empty-parent', type: AccountType.MultiSubAccounts.type, subAccounts: undefined }),
            account({ id: 'unsupported', type: 999 })
        ])).toEqual({});
    });

    test('reports visible root and child counts with first visible indexes', () => {
        const result = getCategorizedAccountsWithVisibleCount(getCategorizedAccountsMap(hierarchy()));
        const cash = result.find(item => item.category === AccountCategory.Cash.type)!;
        const checking = result.find(item => item.category === AccountCategory.CheckingAccount.type)!;

        expect(cash).toMatchObject({ allVisibleAccountCount: 1, firstVisibleAccountIndex: 0 });
        expect(checking).toMatchObject({
            allVisibleAccountCount: 1,
            firstVisibleAccountIndex: 0,
            allVisibleSubAccountCounts: { bank: 1 },
            allFirstVisibleSubAccountIndexes: { bank: 0 }
        });
        expect(checking.allSubAccounts['bank']?.map(item => item.id)).toEqual(['cny-sub', 'usd-sub']);

        const hiddenOnly = account({
            id: 'hidden-parent',
            category: AccountCategory.CheckingAccount.type,
            type: AccountType.MultiSubAccounts.type,
            hidden: true,
            subAccounts: [accountResponse({ id: 'hidden-child', parentId: 'hidden-parent', hidden: true })]
        });
        const hiddenResult = getCategorizedAccountsWithVisibleCount(getCategorizedAccountsMap([hiddenOnly]))[0]!;
        expect(hiddenResult).toMatchObject({
            allVisibleAccountCount: 0,
            firstVisibleAccountIndex: -1,
            allVisibleSubAccountCounts: { 'hidden-parent': 0 },
            allFirstVisibleSubAccountIndexes: { 'hidden-parent': -1 }
        });
        expect(getCategorizedAccountsWithVisibleCount({})).toEqual([]);
    });

    test('projects filtered visible balances from roots and subaccounts', () => {
        const balances = getAllFilteredAccountsBalance(
            getCategorizedAccountsMap(hierarchy()),
            item => item.id !== 'hidden'
        );

        expect(balances).toEqual([
            { balanceCents: 1_000, isAsset: true, isLiability: false, currency: 'CNY' },
            { balanceCents: 3_000, isAsset: true, isLiability: false, currency: 'CNY' }
        ]);
        expect(getAllFilteredAccountsBalance({}, () => true)).toEqual([]);
    });
});

describe('account selection logic', () => {
    test('selects single or child accounts and handles empty parents', () => {
        const ids: Record<string, boolean> = {};
        const [single, , parent] = hierarchy();

        selectAccountOrSubAccounts(ids, single!, true);
        selectAccountOrSubAccounts(ids, parent!, false);
        selectAccountOrSubAccounts(ids, account({
            id: 'empty',
            type: AccountType.MultiSubAccounts.type,
            subAccounts: []
        }), true);
        selectAccountOrSubAccounts(ids, account({ id: 'unsupported', type: 999 }), true);

        expect(ids).toEqual({ cash: true, 'cny-sub': false, 'usd-sub': false });
    });

    test('selects all, none, visible and inverted single-account ids', () => {
        const accounts = hierarchy();
        const map = {
            cash: accounts[0]!,
            hidden: accounts[1]!,
            bank: accounts[2]!,
            'cny-sub': accounts[2]!.subAccounts![0]!,
            'usd-sub': accounts[2]!.subAccounts![1]!
        };

        const visible = { cash: true, hidden: true, bank: true, 'cny-sub': true, 'usd-sub': true, missing: true };
        selectAllVisible(visible, map);
        expect(visible).toEqual({ cash: false, hidden: true, bank: true, 'cny-sub': false, 'usd-sub': true, missing: true });

        const all = { cash: true, hidden: true, bank: true, 'cny-sub': true };
        selectAll(all, map, true);
        expect(all).toEqual({ cash: false, hidden: true, bank: true, 'cny-sub': false });
        selectAll(all, map, false);
        expect(all.hidden).toBe(false);

        selectNone(all, map, true);
        expect(all).toEqual({ cash: true, hidden: false, bank: true, 'cny-sub': true });
        selectNone(all, map, false);
        expect(all.hidden).toBe(true);

        selectInvert(all, map, true);
        expect(all).toEqual({ cash: false, hidden: true, bank: true, 'cny-sub': false });
        selectInvert(all, map, false);
        expect(all.hidden).toBe(false);
    });

    test('derives checked state, final ids and unified selected currency', () => {
        const accounts = hierarchy();
        const single = accounts[0]!;
        const parent = accounts[2]!;

        expect(isAccountOrSubAccountsAllChecked(single, { cash: false })).toBe(true);
        expect(isAccountOrSubAccountsAllChecked(single, { cash: true })).toBe(false);
        expect(isAccountOrSubAccountsAllChecked(parent, { 'cny-sub': false, 'usd-sub': false })).toBe(true);
        expect(isAccountOrSubAccountsAllChecked(parent, { 'cny-sub': true, 'usd-sub': false })).toBe(false);
        expect(isAccountOrSubAccountsHasButNotAllChecked(single, {})).toBe(false);
        expect(isAccountOrSubAccountsHasButNotAllChecked(parent, { 'cny-sub': false, 'usd-sub': true })).toBe(true);
        expect(isAccountOrSubAccountsHasButNotAllChecked(parent, { 'cny-sub': true, 'usd-sub': true })).toBe(false);

        const roots = { cash: single, bank: parent };
        expect(getFinalAccountIdsByFilteredAccountIds(roots, { cash: false, 'cny-sub': false, 'usd-sub': false })).toBe('cash,bank');
        expect(getFinalAccountIdsByFilteredAccountIds(roots, { cash: true, 'cny-sub': false, 'usd-sub': false })).toBe('bank');
        expect(getFinalAccountIdsByFilteredAccountIds(roots, undefined as unknown as Record<string, boolean>)).toBe('cash,bank');
        expect(getFinalAccountIdsByFilteredAccountIds(null as unknown as Record<string, Account>, {})).toBe('');

        const all = {
            cash: single,
            bank: parent,
            'cny-sub': parent.subAccounts![0]!,
            'usd-sub': parent.subAccounts![1]!
        };
        expect(getUnifiedSelectedAccountsCurrencyOrDefaultCurrency(all, { cash: true }, 'EUR')).toBe('CNY');
        expect(getUnifiedSelectedAccountsCurrencyOrDefaultCurrency(all, { cash: true, 'usd-sub': true }, 'EUR')).toBe('EUR');
        expect(getUnifiedSelectedAccountsCurrencyOrDefaultCurrency(all, { bank: true }, 'EUR')).toBe('EUR');
        expect(getUnifiedSelectedAccountsCurrencyOrDefaultCurrency(all, { missing: true }, 'EUR')).toBe('EUR');
        expect(getUnifiedSelectedAccountsCurrencyOrDefaultCurrency(all, undefined as unknown as Record<string, boolean>, 'EUR')).toBe('EUR');
    });
});

describe('fixed-point expression evaluator', () => {
    test.each([
        ['1+2*3', 700],
        ['(1+2)*3', 900],
        [' -1.25 + 2.5 ', 125],
        ['4/2+0.01', 201],
        ['1 2', 1_200],
        ['1/3', 33]
    ])('evaluates %s to integer cents', (expression, expected) => {
        expect(evaluateExpressionToAmount(expression)).toBe(expected);
    });

    test.each([
        '',
        '1+a',
        '1)',
        '(1+2',
        '1+',
        '1/0',
        '--2'
    ])('rejects malformed or overflowing expression %p', expression => {
        expect(evaluateExpressionToAmount(expression)).toBeUndefined();
    });

    test.each(['1.1234567', '1000000000', '-1000000000'])('throws on numeric overflow for %s', expression => {
        expect(() => evaluateExpressionToAmount(expression)).toThrow('Numeric Overflow');
    });
});
