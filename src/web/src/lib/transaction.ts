import { CategoryType } from '@/core/category.ts';
import { TransactionType } from '@/core/transaction.ts';
import { Account } from '@/models/account.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';
import { TransactionTag } from '@/models/transaction_tag.ts';
import { TransactionPicture } from '@/models/transaction_picture_info.ts';
import { Transaction } from '@/models/transaction.ts';
import logger from '@/lib/logger.ts';

import {
    isDefined,
    isNumber
} from './common.ts';
import {
    getBrowserTimezoneOffsetMinutes,
    getDummyUnixTimeForLocalUsage
} from './datetime.ts';
import {
    categoryTypeToTransactionType,
    isSubCategoryIdAvailable,
    getFirstAvailableCategoryId,
    getFirstAvailableSubCategoryId
} from './category.ts';

export interface SetTransactionOptions {
    time?: number;
    type?: number;
    categoryId?: string;
    accountId?: string;
    destinationAccountId?: string;
    sourceAmountCents?: number;
    destinationAmountCents?: number;
    tagIds?: string;
    comment?: string;
}

export function setTransactionModelByTransaction(transaction: Transaction, transaction2: Transaction | null | undefined, allCategories: Record<number, TransactionCategory[]>, allCategoriesMap: Record<string, TransactionCategory>, allVisibleAccounts: Account[], allAccountsMap: Record<string, Account>, allTagsMap: Record<string, TransactionTag>, defaultAccountId: string, options: SetTransactionOptions, setContextData: boolean, convertContextTime: boolean): void {
    if (isDefined(options.time)) {
        transaction.time = options.time;
    }

    if (!options.type && options.categoryId && options.categoryId !== '0' && allCategoriesMap[options.categoryId]) {
        const category = allCategoriesMap[options.categoryId] as TransactionCategory;
        const type = categoryTypeToTransactionType(category.type);

        if (isNumber(type)) {
            transaction.type = type;
        }
    }

    if (isDefined(options.sourceAmountCents)) {
        transaction.sourceAmountCents = options.sourceAmountCents;
    }

    if (isDefined(options.destinationAmountCents)) {
        transaction.destinationAmountCents = options.destinationAmountCents;
    }

    if (allCategories[CategoryType.Expense] &&
        allCategories[CategoryType.Expense].length) {
        if (options.categoryId && options.categoryId !== '0') {
            if (isSubCategoryIdAvailable(allCategories[CategoryType.Expense], options.categoryId)) {
                transaction.expenseCategoryId = options.categoryId;
            } else {
                transaction.expenseCategoryId = getFirstAvailableSubCategoryId(allCategories[CategoryType.Expense], options.categoryId);
            }
        }

        if (!transaction.expenseCategoryId) {
            transaction.expenseCategoryId = getFirstAvailableCategoryId(allCategories[CategoryType.Expense]);
        }
    }

    if (allCategories[CategoryType.Income] &&
        allCategories[CategoryType.Income].length) {
        if (options.categoryId && options.categoryId !== '0') {
            if (isSubCategoryIdAvailable(allCategories[CategoryType.Income], options.categoryId)) {
                transaction.incomeCategoryId = options.categoryId;
            } else {
                transaction.incomeCategoryId = getFirstAvailableSubCategoryId(allCategories[CategoryType.Income], options.categoryId);
            }
        }

        if (!transaction.incomeCategoryId) {
            transaction.incomeCategoryId = getFirstAvailableCategoryId(allCategories[CategoryType.Income]);
        }
    }

    if (allCategories[CategoryType.Transfer] &&
        allCategories[CategoryType.Transfer].length) {
        if (options.categoryId && options.categoryId !== '0') {
            if (isSubCategoryIdAvailable(allCategories[CategoryType.Transfer], options.categoryId)) {
                transaction.transferCategoryId = options.categoryId;
            } else {
                transaction.transferCategoryId = getFirstAvailableSubCategoryId(allCategories[CategoryType.Transfer], options.categoryId);
            }
        }

        if (!transaction.transferCategoryId) {
            transaction.transferCategoryId = getFirstAvailableCategoryId(allCategories[CategoryType.Transfer]);
        }
    }

    if (allVisibleAccounts.length) {
        if (options.accountId && options.accountId !== '0') {
            for (const account of allVisibleAccounts) {
                if (account.id === options.accountId) {
                    transaction.sourceAccountId = options.accountId;
                    transaction.destinationAccountId = options.accountId;
                    break;
                }
            }
        }

        if (options.destinationAccountId && options.destinationAccountId !== '0') {
            for (const account of allVisibleAccounts) {
                if (account.id === options.destinationAccountId) {
                    transaction.destinationAccountId = options.destinationAccountId;
                    break;
                }
            }
        }

        if (!transaction.sourceAccountId) {
            if (defaultAccountId && allAccountsMap[defaultAccountId] && !allAccountsMap[defaultAccountId].hidden) {
                transaction.sourceAccountId = defaultAccountId;
            } else {
                transaction.sourceAccountId = allVisibleAccounts[0]!.id;
            }
        }

        if (!transaction.destinationAccountId) {
            if (defaultAccountId && allAccountsMap[defaultAccountId] && !allAccountsMap[defaultAccountId].hidden) {
                transaction.destinationAccountId = defaultAccountId;
            } else {
                transaction.destinationAccountId = allVisibleAccounts[0]!.id;
            }
        }
    }

    if (allTagsMap && options.tagIds) {
        const tagIds = options.tagIds.split(',');
        const finalTagIds = [];

        for (const tagId of tagIds) {
            const tag = allTagsMap[tagId];

            if (tag && !tag.hidden) {
                finalTagIds.push(tag.id);
            }
        }

        transaction.tagIds = finalTagIds;
    }

    if (options.comment) {
        transaction.comment = options.comment;
    }

    if (transaction2) {
        // 🆕 [数据传递日志] 记录transaction2的关键数据（v6.21.7）
        logger.debug(`[数据传递] setTransactionModelByTransaction: transaction2.id=${transaction2.id}, type=${transaction2.type}, categoryId=${transaction2.categoryId}`);
        logger.debug(`[数据传递] transaction2.category存在: ${!!transaction2.category}, tags数量: ${transaction2.tags?.length || 0}`);
        if (transaction2.category) {
            logger.debug(`[数据传递] transaction2.category内容: ${JSON.stringify({id: transaction2.category.id, name: transaction2.category.name, type: transaction2.category.type})}`);
        }

        if (setContextData) {
            transaction.id = transaction2.id;
        }

        transaction.type = transaction2.type;

        if (transaction.type === TransactionType.Expense) {
            transaction.expenseCategoryId = transaction2.categoryId || '';
        } else if (transaction.type === TransactionType.Income) {
            transaction.incomeCategoryId = transaction2.categoryId || '';
        } else if (transaction.type === TransactionType.Transfer) {
            transaction.transferCategoryId = transaction2.categoryId || '';
        } else if (transaction.type === TransactionType.Investment) {
            // 🆕 添加投资类型支持
            transaction.investmentCategoryId = transaction2.categoryId || '';
            logger.debug(`[数据传递] 设置investmentCategoryId: ${transaction.investmentCategoryId}`);
        }

        // 🆕 传递category对象（v6.21.7修复）
        if (transaction2.category) {
            // 转换TransactionCategoryInfoResponse为TransactionCategory对象
            transaction.setCategory(TransactionCategory.of(transaction2.category));
            logger.debug(`[数据传递] 已调用transaction.setCategory(), category.id=${transaction2.category.id}`);
        } else {
        logger.debug(`[数据传递] transaction2.category为空，跳过setCategory`);
        }

        // 🆕 传递tags对象（v6.21.7修复）
        if (transaction2.tags && transaction2.tags.length > 0) {
            // 转换TransactionTagInfoResponse[]为TransactionTag[]对象
            transaction.setTags(TransactionTag.ofMulti(transaction2.tags));
            logger.debug(`[数据传递] 已调用transaction.setTags(), tags数量=${transaction2.tags.length}`);
        }

        if (setContextData) {
            transaction.utcOffset = transaction2.utcOffset;
            transaction.timeZone = transaction2.timeZone;

            if (convertContextTime) {
                transaction.time = getDummyUnixTimeForLocalUsage(transaction2.time, transaction.utcOffset, getBrowserTimezoneOffsetMinutes());
            } else {
                transaction.time = transaction2.time;
            }
        }

        transaction.sourceAccountId = transaction2.sourceAccountId;

        if (transaction2.destinationAccountId) {
            transaction.destinationAccountId = transaction2.destinationAccountId;
        } else {
            transaction.destinationAccountId = '';
        }

        transaction.sourceAmountCents = transaction2.sourceAmountCents;

        if (transaction2.destinationAmountCents) {
            transaction.destinationAmountCents = transaction2.destinationAmountCents;
        } else {
            transaction.destinationAmountCents = 0;
        }

        transaction.hideAmount = transaction2.hideAmount;
        transaction.tagIds = transaction2.tagIds || [];
        transaction.setPictures(TransactionPicture.ofMulti(transaction2.pictures || []));

        transaction.comment = transaction2.comment;

        if (setContextData) {
            transaction.setGeoLocation(transaction2.geoLocation);
        }
    }
}
