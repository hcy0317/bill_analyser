import { describe, expect, jest, test } from '@jest/globals';

import { AccountCategory, AccountType } from '@/core/account.ts';
import { CategoryType } from '@/core/category.ts';
import { TransactionType } from '@/core/transaction.ts';
import { Account, type AccountInfoResponse } from '@/models/account.ts';
import { Transaction, type TransactionInfoResponse } from '@/models/transaction.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';
import { TransactionTag } from '@/models/transaction_tag.ts';
import { setTransactionModelByTransaction } from '@/lib/transaction.ts';

jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

function accountInfo(overrides: Partial<AccountInfoResponse> = {}): AccountInfoResponse {
    return {
        id: 'cash',
        name: 'Cash',
        parentId: '0',
        category: AccountCategory.Cash.type,
        type: AccountType.SingleAccount.type,
        icon: '1',
        color: '#fff',
        currency: 'CNY',
        balanceCents: 0,
        comment: '',
        displayOrder: 1,
        hidden: false,
        ...overrides
    };
}

function category(type: CategoryType, id: string, subId: string): TransactionCategory {
    return TransactionCategory.of({
        id,
        name: id,
        parentId: '0',
        type,
        icon: '1',
        color: '#fff',
        comment: '',
        displayOrder: 1,
        hidden: false,
        subCategories: [{
            id: subId,
            name: subId,
            parentId: id,
            type,
            icon: '1',
            color: '#fff',
            comment: '',
            displayOrder: 1,
            hidden: false
        }]
    });
}

function categoryRuntime(): {
    allCategories: Record<number, TransactionCategory[]>;
    allCategoriesMap: Record<string, TransactionCategory>;
} {
    const expense = category(CategoryType.Expense, 'expense', 'expense-sub');
    const income = category(CategoryType.Income, 'income', 'income-sub');
    const transfer = category(CategoryType.Transfer, 'transfer', 'transfer-sub');
    const allCategories: Record<number, TransactionCategory[]> = {
        [CategoryType.Expense]: [expense],
        [CategoryType.Income]: [income],
        [CategoryType.Transfer]: [transfer]
    };
    const allCategoriesMap: Record<string, TransactionCategory> = {};
    for (const root of [expense, income, transfer]) {
        allCategoriesMap[root.id] = root;
        allCategoriesMap[root.subCategories![0]!.id] = root.subCategories![0]!;
    }
    return { allCategories, allCategoriesMap };
}

function transactionInfo(type: TransactionType, overrides: Partial<TransactionInfoResponse> = {}): TransactionInfoResponse {
    const categoryType = type === TransactionType.Expense
        ? CategoryType.Expense
        : type === TransactionType.Income
            ? CategoryType.Income
            : type === TransactionType.Transfer
                ? CategoryType.Transfer
                : CategoryType.Investment;
    return {
        id: `transaction-${type}`,
        timeSequenceId: '',
        type,
        categoryId: `category-${type}`,
        time: 1_700_000_000,
        utcOffset: 480,
        sourceAccountId: 'cash',
        destinationAccountId: 'bank',
        sourceAmountCents: 12_345,
        destinationAmountCents: 23_456,
        hideAmount: true,
        tagIds: ['tag-1'],
        comment: 'copied',
        editable: true,
        category: {
            id: `category-${type}`,
            name: 'Copied category',
            parentId: '0',
            type: categoryType,
            icon: '1',
            color: '#fff',
            comment: '',
            displayOrder: 1,
            hidden: false
        },
        tags: [{ id: 'tag-1', name: 'Tag', displayOrder: 1, hidden: false }],
        pictures: [{ pictureId: 'picture-1', originalUrl: '/pictures/1' }],
        geoLocation: { latitude: 31.2, longitude: 121.5 },
        ...overrides
    };
}

describe('transaction initialization from options', () => {
    test('derives type, categories, accounts, cents, tags and comment from valid options', () => {
        const target = Transaction.createNewTransaction(TransactionType.Expense, 0, 'UTC', 0);
        const { allCategories, allCategoriesMap } = categoryRuntime();
        const accounts = [
            Account.of(accountInfo({ id: 'cash' })),
            Account.of(accountInfo({ id: 'bank', name: 'Bank' }))
        ];
        const tags = {
            visible: TransactionTag.of({ id: 'visible', name: 'Visible', displayOrder: 1, hidden: false }),
            hidden: TransactionTag.of({ id: 'hidden', name: 'Hidden', displayOrder: 2, hidden: true })
        };

        setTransactionModelByTransaction(
            target,
            null,
            allCategories,
            allCategoriesMap,
            accounts,
            { cash: accounts[0]!, bank: accounts[1]! },
            tags,
            'cash',
            {
                time: 123,
                categoryId: 'income-sub',
                accountId: 'bank',
                destinationAccountId: 'cash',
                sourceAmountCents: 0,
                destinationAmountCents: -500,
                tagIds: 'visible,hidden,missing',
                comment: 'note'
            },
            false,
            false
        );

        expect(target).toMatchObject({
            time: 123,
            type: TransactionType.Income,
            expenseCategoryId: 'expense-sub',
            incomeCategoryId: 'income-sub',
            transferCategoryId: 'transfer-sub',
            sourceAccountId: 'bank',
            destinationAccountId: 'cash',
            sourceAmountCents: 0,
            destinationAmountCents: -500,
            tagIds: ['visible'],
            comment: 'note'
        });
    });

    test('falls back to first visible account and first category when ids are unavailable', () => {
        const target = Transaction.createNewTransaction(TransactionType.Expense, 0, 'UTC', 0);
        const { allCategories, allCategoriesMap } = categoryRuntime();
        const hiddenDefault = Account.of(accountInfo({ id: 'hidden-default', hidden: true }));
        const visible = Account.of(accountInfo({ id: 'visible' }));

        setTransactionModelByTransaction(
            target,
            undefined,
            allCategories,
            allCategoriesMap,
            [visible],
            { 'hidden-default': hiddenDefault, visible },
            {},
            'hidden-default',
            { type: TransactionType.Expense, categoryId: 'missing', accountId: 'missing', destinationAccountId: 'missing' },
            false,
            false
        );

        expect(target).toMatchObject({
            type: TransactionType.Expense,
            expenseCategoryId: 'expense-sub',
            incomeCategoryId: 'income-sub',
            transferCategoryId: 'transfer-sub',
            sourceAccountId: 'visible',
            destinationAccountId: 'visible'
        });

        const empty = Transaction.createNewTransaction(TransactionType.Expense, 0, 'UTC', 0);
        setTransactionModelByTransaction(empty, null, {}, {}, [], {}, {}, '', {}, false, false);
        expect(empty.sourceAccountId).toBe('');
    });

    test('keeps available expense/transfer subcategories and uses a visible default account', () => {
        const { allCategories, allCategoriesMap } = categoryRuntime();
        const defaultAccount = Account.of(accountInfo({ id: 'default' }));

        const expense = Transaction.createNewTransaction(TransactionType.Expense, 0, 'UTC', 0);
        setTransactionModelByTransaction(
            expense,
            null,
            allCategories,
            allCategoriesMap,
            [defaultAccount],
            { default: defaultAccount },
            null as unknown as Record<string, TransactionTag>,
            'default',
            { categoryId: 'expense-sub', accountId: '0', destinationAccountId: '0', comment: '' },
            false,
            false
        );
        expect(expense).toMatchObject({
            expenseCategoryId: 'expense-sub',
            sourceAccountId: 'default',
            destinationAccountId: 'default'
        });

        const transfer = Transaction.createNewTransaction(TransactionType.Transfer, 0, 'UTC', 0);
        setTransactionModelByTransaction(
            transfer,
            null,
            allCategories,
            allCategoriesMap,
            [defaultAccount],
            { default: defaultAccount },
            {},
            'default',
            { categoryId: 'transfer-sub' },
            false,
            false
        );
        expect(transfer.transferCategoryId).toBe('transfer-sub');
    });
});

describe('transaction copy from existing model', () => {
    test.each([
        [TransactionType.Expense, 'expenseCategoryId'],
        [TransactionType.Income, 'incomeCategoryId'],
        [TransactionType.Transfer, 'transferCategoryId'],
        [TransactionType.Investment, 'investmentCategoryId']
    ] as const)('copies type %s into its category slot and all context fields', (type, categoryField) => {
        const source = Transaction.of(transactionInfo(type));
        const target = Transaction.createNewTransaction(TransactionType.Expense, 0, 'UTC', 0);

        setTransactionModelByTransaction(target, source, {}, {}, [], {}, {}, '', {}, true, true);

        expect((target as unknown as Record<string, unknown>)[categoryField]).toBe(`category-${type}`);
        expect(target).toMatchObject({
            id: `transaction-${type}`,
            type,
            utcOffset: 480,
            timeZone: undefined,
            sourceAccountId: 'cash',
            destinationAccountId: 'bank',
            sourceAmountCents: 12_345,
            destinationAmountCents: 23_456,
            hideAmount: true,
            tagIds: ['tag-1'],
            comment: 'copied'
        });
        expect(target.category?.id).toBe(`category-${type}`);
        expect(target.tags?.map(tag => tag.id)).toEqual(['tag-1']);
        expect(target.pictures?.map(picture => picture.pictureId)).toEqual(['picture-1']);
        expect(target.geoLocation).toEqual({ latitude: 31.2, longitude: 121.5 });
        expect(target.time).not.toBe(0);
    });

    test('preserves target context when requested and clears optional destination fields', () => {
        const source = Transaction.of(transactionInfo(TransactionType.Expense, {
            destinationAccountId: '',
            destinationAmountCents: 0,
            category: undefined,
            tags: [],
            pictures: undefined,
            geoLocation: undefined
        }));
        const target = Transaction.createNewTransaction(TransactionType.Income, 99, 'UTC', 0);
        target.id = 'target-id';
        target.setGeoLocation({ latitude: 1, longitude: 2 });

        setTransactionModelByTransaction(target, source, {}, {}, [], {}, {}, '', {}, false, false);

        expect(target.id).toBe('target-id');
        expect(target.time).toBe(99);
        expect(target.geoLocation).toEqual({ latitude: 1, longitude: 2 });
        expect(target.destinationAccountId).toBe('');
        expect(target.destinationAmountCents).toBe(0);
        expect(target.category).toBeUndefined();
        expect(target.tags).toEqual([]);
        expect(target.pictures).toEqual([]);
    });

    test('uses original time without conversion when context copy is enabled', () => {
        const source = Transaction.of(transactionInfo(TransactionType.Expense, { time: 1_234 }));
        const target = Transaction.createNewTransaction(TransactionType.Expense, 0, 'UTC', 0);

        setTransactionModelByTransaction(target, source, {}, {}, [], {}, {}, '', {}, true, false);

        expect(target.time).toBe(1_234);
    });
});
