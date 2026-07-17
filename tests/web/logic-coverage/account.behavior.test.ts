import { describe, expect, test } from '@jest/globals';

import { AccountCategory, AccountType } from '@/core/account.ts';
import { DEFAULT_ACCOUNT_COLOR } from '@/consts/color.ts';
import { PARENT_ACCOUNT_CURRENCY_PLACEHOLDER } from '@/consts/currency.ts';
import { DEFAULT_ACCOUNT_ICON_ID } from '@/consts/icon.ts';
import {
    Account,
    AccountWithDisplayBalance,
    CategorizedAccountWithDisplayBalance,
    type AccountInfoResponse
} from '@/models/account.ts';

function response(overrides: Partial<AccountInfoResponse> = {}): AccountInfoResponse {
    return {
        id: 'account-1',
        name: 'Main account',
        parentId: '0',
        category: AccountCategory.Cash.type,
        type: AccountType.SingleAccount.type,
        icon: AccountCategory.Cash.defaultAccountIconId,
        color: '#112233',
        currency: 'CNY',
        balanceCents: 12_345,
        comment: 'memo',
        creditCardStatementDate: undefined,
        displayOrder: 1,
        hidden: false,
        ...overrides
    };
}

function account(overrides: Partial<AccountInfoResponse> = {}): Account {
    return Account.of(response(overrides));
}

function multiAccount(subAccounts?: AccountInfoResponse[], overrides: Partial<AccountInfoResponse> = {}): Account {
    return account({
        id: 'parent',
        name: 'Parent',
        category: AccountCategory.CheckingAccount.type,
        type: AccountType.MultiSubAccounts.type,
        currency: PARENT_ACCOUNT_CURRENCY_PLACEHOLDER,
        balanceCents: 0,
        subAccounts,
        ...overrides
    });
}

describe('Account construction and value semantics', () => {
    test('derives asset, liability, visibility and supports explicit server flags', () => {
        const cash = account();
        const credit = account({ category: AccountCategory.CreditCard.type });
        const unknown = account({ category: 999, hidden: true });
        const overridden = account({ category: AccountCategory.CreditCard.type, isAsset: true, isLiability: false });

        expect(cash.isAsset).toBe(true);
        expect(cash.isLiability).toBe(false);
        expect(cash.hidden).toBe(false);
        expect(credit.isAsset).toBe(false);
        expect(credit.isLiability).toBe(true);
        expect(unknown.isAsset).toBe(false);
        expect(unknown.isLiability).toBe(false);
        expect(unknown.hidden).toBe(true);
        expect(overridden.isAsset).toBe(true);
        expect(overridden.isLiability).toBe(false);
    });

    test('compares every scalar field and nested subaccounts', () => {
        const original = account();
        expect(original.equals(original.clone())).toBe(true);

        const mutations: Array<[keyof Account, unknown]> = [
            ['id', 'different'],
            ['name', 'different'],
            ['parentId', 'parent'],
            ['category', AccountCategory.CreditCard.type],
            ['type', AccountType.MultiSubAccounts.type],
            ['icon', '999'],
            ['color', '#ffffff'],
            ['currency', 'USD'],
            ['balanceCents', 1],
            ['balanceTime', 123],
            ['comment', 'different'],
            ['displayOrder', 9],
            ['visible', false],
            ['creditCardStatementDate', 20]
        ];

        for (const [field, value] of mutations) {
            const changed = original.clone();
            (changed as unknown as Record<string, unknown>)[field] = value;
            expect(original.equals(changed)).toBe(false);
        }

        const first = multiAccount([response({ id: 'sub-1', parentId: 'parent' })]);
        const same = first.clone();
        expect(first.equals(same)).toBe(true);

        same.subAccounts![0]!.comment = 'changed';
        expect(first.equals(same)).toBe(false);
        expect(first.equals(multiAccount([
            response({ id: 'sub-1', parentId: 'parent' }),
            response({ id: 'sub-2', parentId: 'parent' })
        ]))).toBe(false);
        expect(first.equals(multiAccount(undefined))).toBe(false);
        expect(multiAccount([]).equals(multiAccount(undefined))).toBe(true);
    });

    test('fills editable fields from another account without changing hierarchy metadata', () => {
        const target = account({ id: 'target', parentId: 'original-parent', displayOrder: 7 });
        const source = account({
            id: 'source',
            name: 'Replacement',
            category: AccountCategory.CreditCard.type,
            type: AccountType.MultiSubAccounts.type,
            icon: '300',
            color: '#abcdef',
            currency: 'USD',
            balanceCents: 77,
            comment: 'replacement',
            creditCardStatementDate: 12,
            hidden: true
        });

        target.fillFrom(source);

        expect(target).toMatchObject({
            id: 'source',
            parentId: 'original-parent',
            displayOrder: 7,
            name: 'Replacement',
            category: AccountCategory.CreditCard.type,
            type: AccountType.MultiSubAccounts.type,
            icon: '300',
            color: '#abcdef',
            currency: 'USD',
            balanceCents: 77,
            comment: 'replacement',
            creditCardStatementDate: 12,
            visible: false
        });
    });

    test('updates a default category icon but preserves a custom icon', () => {
        const defaultIcon = account({ icon: AccountCategory.Cash.defaultAccountIconId });
        defaultIcon.setSuitableIcon(AccountCategory.Cash.type, AccountCategory.CreditCard.type);
        expect(defaultIcon.icon).toBe(AccountCategory.CreditCard.defaultAccountIconId);

        const customIcon = account({ icon: 'custom' });
        customIcon.setSuitableIcon(AccountCategory.Cash.type, AccountCategory.CreditCard.type);
        expect(customIcon.icon).toBe('custom');

        const unknownOld = account({ icon: 'custom' });
        unknownOld.setSuitableIcon(999, AccountCategory.VirtualAccount.type);
        expect(unknownOld.icon).toBe(AccountCategory.VirtualAccount.defaultAccountIconId);

        unknownOld.setSuitableIcon(AccountCategory.VirtualAccount.type, 999);
        expect(unknownOld.icon).toBe(AccountCategory.VirtualAccount.defaultAccountIconId);
    });
});

describe('Account request projections', () => {
    test('projects single and credit-card account creates', () => {
        const single = Account.createNewAccount('CNY', 1_700_000_000);
        single.name = 'Wallet';

        expect(single).toMatchObject({
            category: AccountCategory.Cash.type,
            type: AccountType.SingleAccount.type,
            icon: DEFAULT_ACCOUNT_ICON_ID,
            color: DEFAULT_ACCOUNT_COLOR,
            currency: 'CNY',
            balanceCents: 0,
            balanceTime: 1_700_000_000,
            visible: true
        });
        expect(single.toCreateRequest('session')).toStrictEqual({
            name: 'Wallet',
            category: AccountCategory.Cash.type,
            type: AccountType.SingleAccount.type,
            icon: DEFAULT_ACCOUNT_ICON_ID,
            color: DEFAULT_ACCOUNT_COLOR,
            currency: 'CNY',
            balanceCents: 0,
            balanceTime: 1_700_000_000,
            comment: '',
            creditCardStatementDate: undefined,
            subAccounts: undefined,
            clientSessionId: 'session'
        });

        const credit = account({ category: AccountCategory.CreditCard.type, creditCardStatementDate: 18 });
        expect(credit.toCreateRequest('session').creditCardStatementDate).toBe(18);
    });

    test('projects a multi-account and recursively applies parent semantics to subaccounts', () => {
        const parent = multiAccount([
            response({ id: 'sub-existing', parentId: 'parent', currency: 'CNY', balanceCents: 100 }),
            response({ id: 'sub-usd', parentId: 'parent', currency: 'USD', balanceCents: 200 })
        ]);
        const create = parent.toCreateRequest('session');

        expect(create).toMatchObject({
            category: AccountCategory.CheckingAccount.type,
            type: AccountType.MultiSubAccounts.type,
            currency: PARENT_ACCOUNT_CURRENCY_PLACEHOLDER,
            balanceCents: 0,
            balanceTime: 0,
            clientSessionId: 'session'
        });
        expect(create.subAccounts).toHaveLength(2);
        expect(create.subAccounts![0]).toMatchObject({
            category: AccountCategory.CheckingAccount.type,
            type: AccountType.SingleAccount.type,
            currency: 'CNY',
            balanceCents: 100,
            clientSessionId: undefined,
            subAccounts: undefined
        });

        const withoutChildren = multiAccount(undefined).toCreateRequest('session');
        expect(withoutChildren.subAccounts).toStrictEqual([]);
        expect(parent.toCreateRequest('session', [])?.subAccounts).toStrictEqual([]);
    });

    test('projects root and new/existing subaccount modifications', () => {
        const parent = multiAccount([
            response({ id: '', parentId: 'parent', currency: 'EUR', balanceCents: 50, hidden: true }),
            response({ id: 'existing', parentId: 'parent', currency: 'USD', balanceCents: 60 })
        ]);
        const modify = parent.toModifyRequest('session');

        expect(modify).toMatchObject({
            id: 'parent',
            category: AccountCategory.CheckingAccount.type,
            currency: undefined,
            balanceCents: undefined,
            hidden: false,
            clientSessionId: 'session'
        });
        expect(modify.subAccounts![0]).toMatchObject({
            id: '0',
            category: AccountCategory.CheckingAccount.type,
            currency: 'EUR',
            balanceCents: 50,
            hidden: true,
            clientSessionId: undefined,
            subAccounts: undefined
        });
        expect(modify.subAccounts![1]).toMatchObject({
            id: 'existing',
            currency: undefined,
            balanceCents: undefined
        });

        expect(multiAccount(undefined).toModifyRequest('session').subAccounts).toStrictEqual([]);
        expect(parent.toModifyRequest('session', []).subAccounts).toStrictEqual([]);
        expect(account({ id: '', category: AccountCategory.CreditCard.type, creditCardStatementDate: 8 })
            .toModifyRequest('session')).toMatchObject({ id: '0', creditCardStatementDate: 8 });
    });
});

describe('Account hierarchy lookup and cloning', () => {
    const children = [
        response({ id: 'cny-visible', parentId: 'parent', currency: 'CNY', comment: 'visible' }),
        response({ id: 'usd-hidden', parentId: 'parent', currency: 'USD', comment: 'hidden', hidden: true }),
        response({ id: 'cny-second', parentId: 'parent', currency: 'CNY', comment: 'second' })
    ];

    test('resolves ids, hidden flags, comments and objects for single and multi accounts', () => {
        const single = account({ id: 'single', comment: 'single-comment', hidden: true });
        const parent = multiAccount(children);
        const invalid = account({ type: 999 });

        expect(single.getAccountOrSubAccountId('ignored')).toBe('single');
        expect(single.isAccountOrSubAccountHidden('ignored')).toBe(true);
        expect(single.getAccountOrSubAccountComment('ignored')).toBe('single-comment');
        expect(single.getAccountOrSubAccount('ignored')).toBe(single);

        expect(parent.getAccountOrSubAccountId()).toBe('parent');
        expect(parent.getAccountOrSubAccountId('usd-hidden')).toBe('usd-hidden');
        expect(parent.getAccountOrSubAccountId('missing')).toBeNull();
        expect(parent.isAccountOrSubAccountHidden()).toBe(false);
        expect(parent.isAccountOrSubAccountHidden('usd-hidden')).toBe(true);
        expect(parent.isAccountOrSubAccountHidden('missing')).toBe(false);
        expect(parent.getAccountOrSubAccountComment()).toBe('memo');
        expect(parent.getAccountOrSubAccountComment('cny-visible')).toBe('visible');
        expect(parent.getAccountOrSubAccountComment('missing')).toBeNull();
        expect(parent.getAccountOrSubAccount()).toBe(parent);
        expect(parent.getAccountOrSubAccount('cny-visible')?.id).toBe('cny-visible');
        expect(parent.getAccountOrSubAccount('missing')).toBeNull();

        expect(invalid.getAccountOrSubAccountId()).toBeNull();
        expect(invalid.isAccountOrSubAccountHidden()).toBe(false);
        expect(invalid.getAccountOrSubAccountComment()).toBeNull();
        expect(invalid.getAccountOrSubAccount()).toBeNull();
    });

    test('handles empty child collections and returns unique visible/all currencies', () => {
        const parent = multiAccount(children);
        const empty = multiAccount();

        expect(parent.getSubAccount('cny-visible')?.comment).toBe('visible');
        expect(parent.getSubAccount('missing')).toBeNull();
        expect(empty.getSubAccount('missing')).toBeNull();
        expect(empty.getAccountOrSubAccountId('missing')).toBeNull();
        expect(empty.isAccountOrSubAccountHidden('missing')).toBe(false);
        expect(empty.getAccountOrSubAccountComment('missing')).toBeNull();
        expect(empty.getAccountOrSubAccount('missing')).toBeNull();

        expect(parent.getSubAccountCurrencies(false)).toStrictEqual(['CNY']);
        expect(parent.getSubAccountCurrencies(true)).toStrictEqual(['CNY', 'USD']);
        expect(parent.getSubAccountCurrencies(true, 'usd-hidden')).toStrictEqual(['USD']);
        expect(parent.getSubAccountCurrencies(false, 'usd-hidden')).toStrictEqual(['CNY']);
        expect(empty.getSubAccountCurrencies(true)).toStrictEqual([]);
    });

    test('deep-clones accounts and creates a blank subaccount with inherited appearance', () => {
        const parent = multiAccount(children, { icon: '800', color: '#445566' });
        const clone = parent.clone();
        const clones = Account.cloneAccounts([parent]);
        const newSubAccount = parent.createNewSubAccount('JPY', 987);

        expect(clone).not.toBe(parent);
        expect(clone.subAccounts).not.toBe(parent.subAccounts);
        expect(clone.equals(parent)).toBe(true);
        expect(clones[0]).not.toBe(parent);
        expect(clones[0]!.equals(parent)).toBe(true);
        expect(newSubAccount).toMatchObject({
            id: '',
            name: '',
            icon: '800',
            color: '#445566',
            currency: 'JPY',
            balanceCents: 0,
            balanceTime: 987,
            visible: true
        });
    });
});

describe('Account collection helpers and display wrappers', () => {
    test('maps response arrays, finds names and supplies a fallback', () => {
        const accounts = Account.ofMulti([response({ id: 'a', name: 'A' }), response({ id: 'b', name: 'B' })]);

        expect(accounts.map(item => item.id)).toStrictEqual(['a', 'b']);
        expect(Account.findAccountNameById(accounts, 'b')).toBe('B');
        expect(Account.findAccountNameById(accounts, 'missing', 'Fallback')).toBe('Fallback');
        expect(Account.findAccountNameById(accounts, 'missing')).toBeUndefined();
    });

    test('sorts by category, sibling order, hierarchy, parent order and id fallback', () => {
        const cash = account({ id: 'cash', category: AccountCategory.Cash.type });
        const credit = account({ id: 'credit', category: AccountCategory.CreditCard.type });
        const unknown = account({ id: 'unknown', category: 999 });
        expect(Account.sortAccounts([unknown, cash, credit]).map(item => item.id)).toStrictEqual(['cash', 'credit', 'unknown']);

        const siblingOne = account({ id: 's1', parentId: 'p', category: AccountCategory.CheckingAccount.type, displayOrder: 1 });
        const siblingTwo = account({ id: 's2', parentId: 'p', category: AccountCategory.CheckingAccount.type, displayOrder: 2 });
        expect(Account.sortAccounts([siblingTwo, siblingOne]).map(item => item.id)).toStrictEqual(['s1', 's2']);

        const parent = account({ id: 'p', parentId: '0', category: AccountCategory.CheckingAccount.type, displayOrder: 2 });
        const child = account({ id: 'child', parentId: 'p', category: AccountCategory.CheckingAccount.type, displayOrder: 1 });
        expect(Account.sortAccounts([child, parent]).map(item => item.id)).toStrictEqual(['p', 'child']);
        expect(Account.sortAccounts([parent, child]).map(item => item.id)).toStrictEqual(['p', 'child']);

        const parentA = account({ id: 'pa', category: AccountCategory.CheckingAccount.type, displayOrder: 1 });
        const parentB = account({ id: 'pb', category: AccountCategory.CheckingAccount.type, displayOrder: 3 });
        const childA = account({ id: 'za', parentId: 'pa', category: AccountCategory.CheckingAccount.type });
        const childB = account({ id: 'ab', parentId: 'pb', category: AccountCategory.CheckingAccount.type });
        expect(Account.sortAccounts([childB, childA], { pa: parentA, pb: parentB }).map(item => item.id)).toStrictEqual(['za', 'ab']);
        expect(Account.sortAccounts([childA, childB]).map(item => item.id)).toStrictEqual(['ab', 'za']);

        expect(Account.sortAccounts([])).toStrictEqual([]);
    });

    test('wraps accounts and categories with presentation balances', () => {
        const base = account({ id: 'display' });
        const display = AccountWithDisplayBalance.fromAccount(base, '¥123.45');
        const category = CategorizedAccountWithDisplayBalance.of({
            category: AccountCategory.Cash.type,
            name: 'Cash',
            icon: '1',
            accounts: [base]
        }, [display], '¥123.45');

        expect(display).toBeInstanceOf(Account);
        expect(display).toMatchObject({ id: 'display', displayBalance: '¥123.45' });
        expect(category).toMatchObject({
            category: AccountCategory.Cash.type,
            name: 'Cash',
            icon: '1',
            accounts: [display],
            displayBalance: '¥123.45'
        });
    });
});
