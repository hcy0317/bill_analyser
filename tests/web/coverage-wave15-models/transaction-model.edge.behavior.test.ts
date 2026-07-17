import { describe, expect, test } from '@jest/globals';

import { AccountCategory, AccountType } from '@/core/account.ts';
import { CategoryType } from '@/core/category.ts';
import { WeekDay } from '@/core/datetime.ts';
import { TransactionType } from '@/core/transaction.ts';
import { Account } from '@/models/account.ts';
import {
    Transaction,
    TransactionGeoLocation,
    type TransactionDraft,
    type TransactionInfoResponse,
} from '@/models/transaction.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';
import { TransactionPicture } from '@/models/transaction_picture_info.ts';
import { TransactionTag } from '@/models/transaction_tag.ts';

function transactionInfo(overrides: Partial<TransactionInfoResponse> = {}): TransactionInfoResponse {
    return {
        id: 'transaction-1',
        timeSequenceId: 'sequence-1',
        type: TransactionType.Expense,
        categoryId: 'category-1',
        time: 1_700_000_000,
        utcOffset: 480,
        sourceAccountId: 'source-1',
        destinationAccountId: 'destination-1',
        sourceAmountCents: 1_234,
        destinationAmountCents: 5_678,
        hideAmount: false,
        tagIds: [],
        comment: 'memo',
        editable: true,
        ...overrides,
    };
}

function category(type = CategoryType.Expense): TransactionCategory {
    return TransactionCategory.of({
        id: 'category-1',
        name: 'Category',
        parentId: '0',
        type,
        icon: '1',
        color: '#fff',
        comment: '',
        displayOrder: 1,
        hidden: false,
    });
}

function account(id: string): Account {
    return Account.of({
        id,
        name: id,
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
    });
}

function tag(id: string): TransactionTag {
    return TransactionTag.of({ id, name: id, displayOrder: 1, hidden: false });
}

describe('transaction model edge behavior', () => {
    test('routes category ids for every supported type and ignores unknown types', () => {
        const cases = [
            [TransactionType.Expense, 'expenseCategoryId'],
            [TransactionType.Income, 'incomeCategoryId'],
            [TransactionType.Transfer, 'transferCategoryId'],
            [TransactionType.Investment, 'investmentCategoryId'],
        ] as const;

        for (const [type, property] of cases) {
            const model = Transaction.createNewTransaction(type, 1, 'UTC', 0);
            model.setCategoryId(`category-${type}`);
            expect(model.getCategoryId()).toBe(`category-${type}`);
            expect(model[property]).toBe(`category-${type}`);
        }

        const unknown = Transaction.createNewTransaction(999, 1, 'UTC', 0);
        unknown.setCategoryId('ignored');
        expect(unknown.getCategoryId()).toBe('');
        expect(unknown.categoryId).toBe('');
    });

    test('projects optional related entities and picture mutations without leaking backing arrays', () => {
        const model = Transaction.createNewTransaction(TransactionType.Expense, 1, 'UTC', 0);
        const source = account('source-1');
        const destination = account('destination-1');
        const visibleTag = tag('tag-1');
        const modelCategory = category();

        expect(model.pictures).toEqual([]);
        expect(model.tags).toEqual([]);
        expect(model.getPictureIds()).toEqual([]);
        model.removePicture({ pictureId: 'missing', originalUrl: '/missing' });

        model.setCategory(modelCategory);
        model.setSourceAccount(source);
        model.setDestinationAccount(destination);
        model.setTags([visibleTag]);
        model.addPicture({ pictureId: 'picture-1', originalUrl: '/one' });
        model.setPictures([
            TransactionPicture.of({ pictureId: 'picture-1', originalUrl: '/one' }),
            TransactionPicture.of({ pictureId: 'picture-2', originalUrl: '/two' }),
        ]);

        expect(model.category?.id).toBe('category-1');
        expect(model.sourceAccount?.id).toBe('source-1');
        expect(model.destinationAccount?.id).toBe('destination-1');
        expect(model.tags?.map(item => item.id)).toEqual(['tag-1']);
        expect(model.pictures?.map(item => item.pictureId)).toEqual(['picture-1', 'picture-2']);
        expect(model.getPictureIds()).toEqual(['picture-1', 'picture-2']);

        model.removePicture({ pictureId: 'missing', originalUrl: '/missing' });
        expect(model.getPictureIds()).toEqual(['picture-1', 'picture-2']);
        model.removePicture({ pictureId: 'picture-1', originalUrl: '/one' });
        expect(model.getPictureIds()).toEqual(['picture-2']);
        model.clearPictures();
        expect(model.getPictureIds()).toEqual([]);

        model.setCategory(undefined);
        model.setSourceAccount(undefined);
        model.setDestinationAccount(undefined);
        expect(model.category).toBeUndefined();
        expect(model.sourceAccount).toBeUndefined();
        expect(model.destinationAccount).toBeUndefined();
    });

    test('normalizes coordinates and accepts numeric or object weekday display values', () => {
        const model = Transaction.createNewTransaction(TransactionType.Expense, 1, 'UTC', 0);
        expect(model.geoLocation).toBeUndefined();
        expect(model.getNormalizedGeoLocation()).toBeUndefined();
        expect(model.displayDayOfWeek).toBeUndefined();
        expect(model.getDisplayDayOfWeekObject()).toBeUndefined();

        model.setGeoLocation({ latitude: 120, longitude: 540 });
        expect(model.getNormalizedGeoLocation()).toEqual({ latitude: 90, longitude: -180 });
        model.geoLocation = { latitude: 31.2, longitude: 121.5 };
        expect(model.geoLocation).toEqual({ latitude: 31.2, longitude: 121.5 });
        model.setLatitudeAndLongitude(-12.5, 220);
        expect(model.getNormalizedGeoLocation()).toEqual({ latitude: -12.5, longitude: -140 });
        model.removeGeoLocation();
        expect(model.geoLocation).toBeUndefined();
        model.setGeoLocation(undefined);

        model.setDisplayDate('2026-07-16', 16, 2);
        expect(model.gregorianCalendarYearDashMonthDashDay).toBe('2026-07-16');
        expect(model.gregorianCalendarDayOfMonth).toBe(16);
        expect(model.displayDayOfWeek).toBe(2);
        expect(model.getDisplayDayOfWeekObject()).toBe(WeekDay.Monday);
        model.setDisplayDate('2026-07-17', 17, WeekDay.Friday);
        expect(model.displayDayOfWeek).toBe(6);

        const location = TransactionGeoLocation.createNewGeoLocation(91, 181);
        expect(location.toNormalizedCoordinate()).toEqual({ latitude: 90, longitude: -179 });
        expect(TransactionGeoLocation.of({ latitude: 1, longitude: 2 })).toEqual({ latitude: 1, longitude: 2 });
    });

    test('builds create, modify and draft payloads for destination and non-destination types', () => {
        for (const type of [TransactionType.Expense, TransactionType.Income, TransactionType.Transfer, TransactionType.Investment]) {
            const model = Transaction.createNewTransaction(type, 100, 'UTC', 480);
            model.id = 'transaction-1';
            model.setCategoryId('category-1');
            model.sourceAccountId = 'source-1';
            model.destinationAccountId = 'destination-1';
            model.sourceAmountCents = 123;
            model.destinationAmountCents = 456;
            model.hideAmount = true;
            model.tagIds = ['tag-1'];
            model.comment = 'memo';
            model.addPicture({ pictureId: 'picture-1', originalUrl: '/one' });
            model.setGeoLocation({ latitude: 31, longitude: 121 });

            const needsDestination = type === TransactionType.Transfer || type === TransactionType.Investment;
            expect(model.toCreateRequest('session-1', 200)).toMatchObject({
                type,
                time: 200,
                destinationAccountId: needsDestination ? 'destination-1' : '0',
                destinationAmountCents: needsDestination ? 456 : 0,
                clientSessionId: 'session-1',
                pictureIds: ['picture-1'],
                geoLocation: { latitude: 31, longitude: 121 },
            });
            expect(model.toCreateRequest('session-2', 0).time).toBe(100);
            expect(model.toModifyRequest(300)).toMatchObject({
                id: 'transaction-1',
                type,
                time: 300,
                destinationAccountId: needsDestination ? 'destination-1' : '0',
                destinationAmountCents: needsDestination ? 456 : 0,
            });
            expect(model.toModifyRequest(0).time).toBe(100);
            expect(model.toTransactionDraft()).toMatchObject({
                type,
                destinationAccountId: needsDestination ? 'destination-1' : '0',
                destinationAmountCents: needsDestination ? 456 : 0,
            });
        }

        const balance = Transaction.createNewTransaction(TransactionType.ModifyBalance, 5, 'UTC', 0);
        balance.setCategoryId('ignored');
        expect(balance.toModifyRequest()).toMatchObject({ categoryId: '0', destinationAccountId: '0' });
        expect(balance.toTransactionDraft()).toBeNull();
    });

    test('hydrates optional response relations, display fields, arrays and draft defaults', () => {
        const full = Transaction.of(transactionInfo({
            category: category(),
            sourceAccount: account('source-1'),
            destinationAccount: account('destination-1'),
            tags: [tag('tag-1')],
            pictures: [{ pictureId: 'picture-1', originalUrl: '/one' }],
            geoLocation: { latitude: 10, longitude: 20 },
            gregorianCalendarYearDashMonthDashDay: '2026-07-16',
            gregorianCalendarDayOfMonth: 16,
            displayDayOfWeek: 5,
        }));
        expect(full).toMatchObject({ id: 'transaction-1', categoryId: 'category-1' });
        expect(full.category?.id).toBe('category-1');
        expect(full.sourceAccount?.id).toBe('source-1');
        expect(full.destinationAccount?.id).toBe('destination-1');
        expect(full.tags?.map(item => item.id)).toEqual(['tag-1']);
        expect(full.pictures?.map(item => item.pictureId)).toEqual(['picture-1']);
        expect(full.geoLocation).toEqual({ latitude: 10, longitude: 20 });
        expect(full.displayDayOfWeek).toBe(5);

        expect(Transaction.ofMulti([transactionInfo({ id: 'one' }), transactionInfo({ id: 'two' })]).map(item => item.id)).toEqual(['one', 'two']);
        expect(Transaction.ofDraft()).toBeNull();
        expect(Transaction.ofDraft(null)).toBeNull();
        expect(Transaction.ofDraft({ type: TransactionType.Investment } as TransactionDraft)).toBeNull();
        expect(Transaction.ofDraft({ type: 999 } as TransactionDraft)).toBeNull();

        const draft = Transaction.ofDraft({
            type: TransactionType.Transfer,
            categoryId: 'transfer-category',
            sourceAccountId: 'source-1',
            destinationAccountId: 'destination-1',
            sourceAmountCents: 100,
            destinationAmountCents: 100,
            hideAmount: true,
            tagIds: ['tag-1'],
            pictures: [{ pictureId: 'picture-1', originalUrl: '/one' }],
            comment: 'draft',
        });
        expect(draft).toMatchObject({
            type: TransactionType.Transfer,
            transferCategoryId: 'transfer-category',
            destinationAccountId: 'destination-1',
            destinationAmountCents: 100,
            comment: 'draft',
        });
        expect(draft?.getPictureIds()).toEqual(['picture-1']);

        const defaults = Transaction.ofDraft({ type: TransactionType.Expense } as TransactionDraft);
        expect(defaults).toMatchObject({
            expenseCategoryId: '', sourceAccountId: '', destinationAccountId: '',
            sourceAmountCents: 0, destinationAmountCents: 0, hideAmount: false, tagIds: [], comment: '',
        });
    });
});
