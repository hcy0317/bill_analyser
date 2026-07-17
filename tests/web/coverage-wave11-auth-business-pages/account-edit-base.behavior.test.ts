import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockGetAllAccountCategories = jest.fn(() => [
    { type: 1, displayName: 'Cash', defaultAccountIconId: 'cash-icon' },
    { type: 3, displayName: 'Credit Card', defaultAccountIconId: 'card-icon' }
]);
const mockGetAllAccountTypes = jest.fn(() => [
    { type: 1, displayName: 'Single' },
    { type: 2, displayName: 'Multiple' }
]);

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getAllAccountCategories: () => mockGetAllAccountCategories(),
        getAllAccountTypes: () => mockGetAllAccountTypes(),
        getMonthdayShortName: (day: number) => `day:${day}`
    })
}));
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({ currentUserDefaultCurrency: 'USD' })
}));
jest.mock('@/lib/datetime.ts', () => ({
    getCurrentUnixTime: () => 1_700_000_000
}));

import { AccountCategory, AccountType } from '@/core/account.ts';
import { Account } from '@/models/account.ts';
import { useAccountEditPageBase } from '@/views/base/accounts/AccountEditPageBase.ts';

function account(overrides: Partial<Account> = {}): Account {
    const value = Account.createNewAccount('USD', 1_700_000_000);
    Object.assign(value, overrides);
    return value;
}

beforeEach(() => {
    jest.clearAllMocks();
});

describe('useAccountEditPageBase presentation state', () => {
    test('exposes localized choices, add/edit titles, and credit-card statement days', () => {
        const base = useAccountEditPageBase();

        expect(base.title.value).toBe('Add Account');
        expect(base.saveButtonTitle.value).toBe('Add');
        base.editAccountId.value = 'account-1';
        expect(base.title.value).toBe('Edit Account');
        expect(base.saveButtonTitle.value).toBe('Save');

        expect(base.allAccountCategories.value).toEqual(mockGetAllAccountCategories());
        expect(base.allAccountTypes.value).toEqual(mockGetAllAccountTypes());
        expect(base.allAvailableMonthDays.value).toHaveLength(29);
        expect(base.allAvailableMonthDays.value[0]).toEqual({ day: 0, displayName: 'tt:Not set' });
        expect(base.allAvailableMonthDays.value[28]).toEqual({ day: 28, displayName: 'day:28' });
        expect(base.getAccountCreditCardStatementDate(7)).toBe('day:7');
        expect(base.getAccountCreditCardStatementDate(31)).toBeNull();

        expect(base.isAccountSupportCreditCardStatementDate.value).toBe(false);
        base.account.value.category = AccountCategory.CreditCard.type;
        expect(base.isAccountSupportCreditCardStatementDate.value).toBe(true);
    });

    test('reports every parent and sub-account required-field boundary', () => {
        const base = useAccountEditPageBase();

        base.account.value.category = 0;
        expect(base.inputEmptyProblemMessage.value).toBe('Account category cannot be blank');
        base.account.value.category = AccountCategory.Cash.type;
        base.account.value.type = 0;
        expect(base.inputEmptyProblemMessage.value).toBe('Account type cannot be blank');
        base.account.value.type = AccountType.SingleAccount.type;
        base.account.value.name = '';
        expect(base.inputEmptyProblemMessage.value).toBe('Account name cannot be blank');
        base.account.value.name = 'Wallet';
        base.account.value.currency = '';
        expect(base.inputEmptyProblemMessage.value).toBe('Account currency cannot be blank');
        expect(base.inputIsEmpty.value).toBe(true);

        base.account.value.currency = 'USD';
        expect(base.inputEmptyProblemMessage.value).toBeNull();
        expect(base.inputIsEmpty.value).toBe(false);

        base.account.value.type = AccountType.MultiSubAccounts.type;
        const child = account({ category: 0, name: '', currency: '' });
        base.subAccounts.value = [child];
        expect(base.inputEmptyProblemMessage.value).toBe('Account name cannot be blank');
        (base.subAccounts.value[0] as Account).name = 'USD wallet';
        expect(base.inputEmptyProblemMessage.value).toBe('Account currency cannot be blank');
        (base.subAccounts.value[0] as Account).currency = 'USD';
        expect(base.inputEmptyProblemMessage.value).toBeNull();
    });
});

describe('useAccountEditPageBase account lifecycle', () => {
    test('adds sub-accounts only for multi-account parents and identifies new identities', () => {
        const base = useAccountEditPageBase();

        expect(base.isNewAccount(account({ id: '' }))).toBe(true);
        expect(base.isNewAccount(account({ id: '0' }))).toBe(true);
        expect(base.isNewAccount(account({ id: 'persisted' }))).toBe(false);
        expect(base.addSubAccount()).toBe(false);
        expect(base.subAccounts.value).toHaveLength(0);

        base.account.value.type = AccountType.MultiSubAccounts.type;
        expect(base.addSubAccount()).toBe(true);
        expect(base.subAccounts.value).toHaveLength(1);
        expect(base.subAccounts.value[0]).toEqual(expect.objectContaining({
            parentId: base.account.value.id,
            currency: 'USD',
            balanceTime: 1_700_000_000
        }));
    });

    test('copies persisted parent and child values without retaining child object identity', () => {
        const base = useAccountEditPageBase();
        const firstChild = account({ id: 'child-1', name: 'Cash child', balanceCents: 12_345 });
        const secondChild = account({ id: 'child-2', name: 'Card child', currency: 'CNY' });
        const persisted = account({
            id: 'parent-1',
            name: 'Portfolio',
            type: AccountType.MultiSubAccounts.type,
            subAccounts: [firstChild, secondChild]
        });

        base.setAccount(persisted);

        expect(base.account.value).toEqual(expect.objectContaining({ id: 'parent-1', name: 'Portfolio' }));
        expect(base.subAccounts.value).toHaveLength(2);
        expect(base.subAccounts.value[0]).not.toBe(firstChild);
        expect(base.subAccounts.value[0]).toEqual(expect.objectContaining({
            id: 'child-1', name: 'Cash child', balanceCents: 12_345
        }));
        expect(base.subAccounts.value[1]).not.toBe(secondChild);
        expect(base.subAccounts.value[1]).toEqual(expect.objectContaining({
            id: 'child-2', name: 'Card child', currency: 'CNY'
        }));
    });

    test('updates the suitable icon when the selected account category changes', async () => {
        const base = useAccountEditPageBase();
        const iconSpy = jest.spyOn(base.account.value, 'setSuitableIcon').mockImplementation(() => undefined);
        const oldCategory = base.account.value.category;

        base.account.value.category = AccountCategory.CreditCard.type;
        await (jest.requireActual('vue') as any).nextTick();

        expect(iconSpy).toHaveBeenCalledWith(oldCategory, AccountCategory.CreditCard.type);
    });
});
