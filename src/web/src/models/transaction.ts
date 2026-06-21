import { itemAndIndex } from '@/core/base.ts';
import type { TextualYearMonthDay } from '@/core/datetime.ts';
import { WeekDay } from '@/core/datetime.ts';
import { type Coordinate, getNormalizedCoordinate } from '@/core/coordinate.ts';
import { TransactionType } from '@/core/transaction.ts';

import { Account, type AccountInfoResponse } from './account.ts';
import { TransactionCategory, type TransactionCategoryInfoResponse } from './transaction_category.ts';
import { TransactionTag, type TransactionTagInfoResponse } from './transaction_tag.ts';
import { TransactionPicture, type TransactionPictureInfoBasicResponse } from './transaction_picture_info.ts';
import type {
    TransactionCreateRequest,
    TransactionDraft,
    TransactionGeoLocationRequest,
    TransactionGeoLocationResponse,
    TransactionInfoResponse,
    TransactionModifyRequest
} from './transaction/contracts.ts';

/** 前端正式交易模型，统一承载列表、编辑、批量录入和移动端页面的交易字段投影。 */
export class Transaction implements TransactionInfoResponse {
    public id: string;
    public timeSequenceId: string;
    public type: number;
    public expenseCategoryId: string = '';
    public incomeCategoryId: string = '';
    public transferCategoryId: string = '';
    public investmentCategoryId: string = '';
    public time: number;
    public timeZone?: string; // only in new transaction
    public utcOffset: number;
    public sourceAccountId: string;
    public destinationAccountId: string;
    public sourceAmountCents: number;
    public destinationAmountCents: number;
    public hideAmount: boolean;
    public tagIds: string[];
    public comment: string;
    public editable: boolean;

    private _pictures?: TransactionPicture[];
    private _geoLocation?: TransactionGeoLocation;

    private _category?: TransactionCategory; // only for displaying transaction
    private _sourceAccount?: Account; // only for displaying transaction
    private _destinationAccount?: Account; // only for displaying transaction
    private _tags?: TransactionTag[]; // only for displaying transaction

    private _gregorianCalendarYearDashMonthDashDay?: TextualYearMonthDay = undefined; // only for displaying transaction in transaction list
    private _gregorianCalendarDayOfMonth?: number = undefined; // only for displaying transaction in transaction list
    private _displayDayOfWeek?: WeekDay = undefined; // only for displaying transaction in transaction list

    protected constructor(id: string, timeSequenceId: string, type: number, categoryId: string, time: number, timeZone: string | undefined, utcOffset: number, sourceAccountId: string, destinationAccountId: string, sourceAmountCents: number, destinationAmountCents: number, hideAmount: boolean, tagIds: string[], comment: string, editable: boolean) {
        this.id = id;
        this.timeSequenceId = timeSequenceId;
        this.type = type;
        this.time = time;
        this.timeZone = timeZone;
        this.utcOffset = utcOffset;
        this.sourceAccountId = sourceAccountId;
        this.destinationAccountId = destinationAccountId;
        this.sourceAmountCents = sourceAmountCents;
        this.destinationAmountCents = destinationAmountCents;
        this.hideAmount = hideAmount;
        this.tagIds = tagIds;
        this.comment = comment;
        this.editable = editable;
        this.setCategoryId(categoryId);
    }

    public get pictures(): TransactionPictureInfoBasicResponse[] | undefined {
        const ret: TransactionPictureInfoBasicResponse[] = [];

        if (this._pictures) {
            for (const picture of this._pictures) {
                ret.push(picture);
            }
        }

        return ret;
    }

    public get geoLocation(): TransactionGeoLocationResponse | undefined {
        return this._geoLocation;
    }


    public set geoLocation(value: Coordinate) {
        this._geoLocation = TransactionGeoLocation.of(value);
    }

    public get categoryId(): string {
        return this.getCategoryId();
    }

    public get category(): TransactionCategoryInfoResponse | undefined {
        return this._category;
    }

    public get sourceAccount(): AccountInfoResponse | undefined {
        return this._sourceAccount;
    }

    public get destinationAccount(): AccountInfoResponse | undefined {
        return this._destinationAccount;
    }

    public get tags(): TransactionTagInfoResponse[] | undefined {
        const ret: TransactionTagInfoResponse[] = [];

        if (this._tags) {
            for (const tag of this._tags) {
                ret.push(tag);
            }
        }

        return ret;
    }

    public get gregorianCalendarYearDashMonthDashDay(): TextualYearMonthDay | undefined {
        return this._gregorianCalendarYearDashMonthDashDay;
    }

    public get gregorianCalendarDayOfMonth(): number | undefined {
        return this._gregorianCalendarDayOfMonth;
    }

    public get displayDayOfWeek(): number | undefined {
        // 返回数字格式 (1=周日, 2=周一, ..., 7=周六) 以匹配API响应
        return this._displayDayOfWeek ? this._displayDayOfWeek.type + 1 : undefined;
    }

    // 内部使用的WeekDay对象getter
    public getDisplayDayOfWeekObject(): WeekDay | undefined {
        return this._displayDayOfWeek;
    }

    public getCategoryId(): string {
        if (this.type === TransactionType.Expense) {
            return this.expenseCategoryId;
        } else if (this.type === TransactionType.Income) {
            return this.incomeCategoryId;
        } else if (this.type === TransactionType.Transfer) {
            return this.transferCategoryId;
        } else if (this.type === TransactionType.Investment) {
            return this.investmentCategoryId;
        } else {
            return '';
        }
    }

    public setCategoryId(categoryId: string): void {
        if (this.type === TransactionType.Expense) {
            this.expenseCategoryId = categoryId;
        } else if (this.type === TransactionType.Income) {
            this.incomeCategoryId = categoryId;
        } else if (this.type === TransactionType.Transfer) {
            this.transferCategoryId = categoryId;
        } else if (this.type === TransactionType.Investment) {
            this.investmentCategoryId = categoryId;
        }
    }

    public setCategory(category?: TransactionCategory): void {
        this._category = category;
    }

    public setSourceAccount(sourceAccount?: Account): void {
        this._sourceAccount = sourceAccount;
    }

    public setDestinationAccount(destinationAccount?: Account): void {
        this._destinationAccount = destinationAccount;
    }

    public setTags(tags: TransactionTag[]): void {
        this._tags = tags;
    }

    public getPictureIds(): string[] {
        const pictureIds: string[] = [];

        if (this._pictures) {
            for (const picture of this._pictures) {
                pictureIds.push(picture.pictureId);
            }
        }

        return pictureIds;
    }

    public setPictures(pictures: TransactionPicture[]): void {
        this._pictures = pictures;
    }

    public addPicture(pictureInfo: TransactionPictureInfoBasicResponse): void {
        if (!this._pictures) {
            this._pictures = [];
        }

        this._pictures.push(TransactionPicture.of(pictureInfo));
    }

    public removePicture(pictureInfo: TransactionPictureInfoBasicResponse): void {
        if (!this._pictures) {
            return;
        }

        for (const [picture, index] of itemAndIndex(this._pictures)) {
            if (picture.pictureId === pictureInfo.pictureId) {
                this._pictures.splice(index, 1);
            }
        }
    }

    public clearPictures(): void {
        this._pictures = [];
    }

    public getNormalizedGeoLocation(): Coordinate | undefined {
        if (!this._geoLocation) {
            return undefined;
        }

        return this._geoLocation.toNormalizedCoordinate();
    }

    public setGeoLocation(geoLocation?: Coordinate): void {
        if (geoLocation) {
            this._geoLocation = TransactionGeoLocation.createNewGeoLocation(geoLocation.latitude, geoLocation.longitude);
        } else {
            this._geoLocation = undefined;
        }
    }

    public setLatitudeAndLongitude(latitude: number, longitude: number): void {
        this._geoLocation = TransactionGeoLocation.createNewGeoLocation(latitude, longitude);
    }

    public removeGeoLocation(): void {
        this._geoLocation = undefined;
    }

    public setDisplayDate(gregorianCalendarYearDashMonthDashDay: TextualYearMonthDay, gregorianCalendarDayOfMonth: number, displayDayOfWeek: WeekDay | number): void {
        this._gregorianCalendarYearDashMonthDashDay = gregorianCalendarYearDashMonthDashDay;
        this._gregorianCalendarDayOfMonth = gregorianCalendarDayOfMonth;

        // 处理displayDayOfWeek: 可以是WeekDay对象或数字
        if (typeof displayDayOfWeek === 'number') {
            // 后端返回的数字: 1=周日, 2=周一, ..., 7=周六
            // 前端WeekDay: 0=周日, 1=周一, ..., 6=周六
            // 转换: backendValue - 1 = frontendValue
            const weekDayValue = displayDayOfWeek - 1;
            this._displayDayOfWeek = WeekDay.valueOf(weekDayValue);
        } else {
            this._displayDayOfWeek = displayDayOfWeek;
        }
    }

    public toCreateRequest(clientSessionId: string, actualTime?: number): TransactionCreateRequest {
        // ⚠️ 关键修复: 投资类型也需要传递destinationAccountId和destinationAmountCents
        const needsDestination = this.type === TransactionType.Transfer || this.type === TransactionType.Investment;

        return {
            type: this.type,
            categoryId: this.getCategoryId(),
            time: actualTime ? actualTime : this.time,
            utcOffset: this.utcOffset,
            sourceAccountId: this.sourceAccountId,
            destinationAccountId: needsDestination ? this.destinationAccountId : '0',
            sourceAmountCents: this.sourceAmountCents,
            destinationAmountCents: needsDestination ? this.destinationAmountCents : 0,
            hideAmount: this.hideAmount,
            tagIds: this.tagIds,
            pictureIds: this.getPictureIds(),
            comment: this.comment,
            geoLocation: this.getNormalizedGeoLocation(),
            clientSessionId: clientSessionId
        };
    }

    public toModifyRequest(actualTime?: number): TransactionModifyRequest {
        let categoryId = this.getCategoryId();

        if (this.type === TransactionType.ModifyBalance) {
            categoryId = '0';
        }

        return {
            id: this.id,
            type: this.type,  // 添加type字段，确保编辑时保持账单类型
            categoryId: categoryId,
            time: actualTime ? actualTime : this.time,
            utcOffset: this.utcOffset,
            sourceAccountId: this.sourceAccountId,
            destinationAccountId: this.type === TransactionType.Transfer || this.type === TransactionType.Investment ? this.destinationAccountId : '0',
            sourceAmountCents: this.sourceAmountCents,
            destinationAmountCents: this.type === TransactionType.Transfer || this.type === TransactionType.Investment ? this.destinationAmountCents : 0,
            hideAmount: this.hideAmount,
            tagIds: this.tagIds,
            pictureIds: this.getPictureIds(),
            comment: this.comment,
            geoLocation: this.getNormalizedGeoLocation()
        };
    }

    public toTransactionDraft(): TransactionDraft | null {
        // ⚠️ 修复: 支持投资类型的草稿保存
        if (this.type !== TransactionType.Expense &&
            this.type !== TransactionType.Income &&
            this.type !== TransactionType.Transfer &&
            this.type !== TransactionType.Investment) {
            return null;
        }

        const needsDestination = this.type === TransactionType.Transfer || this.type === TransactionType.Investment;

        return {
            type: this.type,
            categoryId: this.getCategoryId(),
            sourceAccountId: this.sourceAccountId,
            sourceAmountCents: this.sourceAmountCents,
            destinationAccountId: needsDestination ? this.destinationAccountId : '0',
            destinationAmountCents: needsDestination ? this.destinationAmountCents : 0,
            hideAmount: this.hideAmount,
            tagIds: this.tagIds,
            pictures: this.pictures,
            comment: this.comment,
        };
    }

    public static createNewTransaction(type: number, time: number, timeZone: string, utcOffset: number): Transaction {
        return new Transaction(
            '', // id
            '', // timeSequenceId
            type, // type
            '', // categoryId
            time, // time
            timeZone, // timeZone
            utcOffset, // utcOffset
            '', // sourceAccountId
            '', // destinationAccountId
            0, // sourceAmountCents
            0, // destinationAmountCents
            false, // hideAmount
            [], // tagIds
            '', // comment
            true // editable
        );
    }

    public static of(transactionResponse: TransactionInfoResponse): Transaction {
        const transaction: Transaction = new Transaction(
            transactionResponse.id,
            transactionResponse.timeSequenceId,
            transactionResponse.type,
            transactionResponse.categoryId,
            transactionResponse.time,
            undefined, // only in new transaction
            transactionResponse.utcOffset,
            transactionResponse.sourceAccountId,
            transactionResponse.destinationAccountId,
            transactionResponse.sourceAmountCents,
            transactionResponse.destinationAmountCents,
            transactionResponse.hideAmount,
            transactionResponse.tagIds,
            transactionResponse.comment,
            transactionResponse.editable
        );

        if (transactionResponse.category) {
            transaction.setCategory(TransactionCategory.of(transactionResponse.category));
        }

        if (transactionResponse.sourceAccount) {
            transaction.setSourceAccount(Account.of(transactionResponse.sourceAccount));
        }

        if (transactionResponse.destinationAccount) {
            transaction.setDestinationAccount(Account.of(transactionResponse.destinationAccount));
        }

        if (transactionResponse.tags) {
            transaction.setTags(TransactionTag.ofMulti(transactionResponse.tags));
        }

        if (transactionResponse.pictures) {
            const pictures: TransactionPicture[] = [];

            for (const picture of transactionResponse.pictures) {
                pictures.push(TransactionPicture.of(picture));
            }

            transaction.setPictures(pictures);
        }

        if (transactionResponse.geoLocation) {
            transaction.setLatitudeAndLongitude(transactionResponse.geoLocation.latitude, transactionResponse.geoLocation.longitude);
        }

        // 设置日期显示字段（用于交易列表分组显示）
        if (transactionResponse.gregorianCalendarYearDashMonthDashDay &&
            transactionResponse.gregorianCalendarDayOfMonth &&
            transactionResponse.displayDayOfWeek) {
            transaction.setDisplayDate(
                transactionResponse.gregorianCalendarYearDashMonthDashDay as TextualYearMonthDay,
                transactionResponse.gregorianCalendarDayOfMonth,
                transactionResponse.displayDayOfWeek
            );
        }

        return transaction;
    }

    public static ofMulti(transactionResponses: TransactionInfoResponse[]): Transaction[] {
        const transactions: Transaction[] = [];

        for (const transactionResponse of transactionResponses) {
            transactions.push(Transaction.of(transactionResponse));
        }

        return transactions;
    }

    public static ofDraft(transactionDraft?: TransactionDraft | null): Transaction | null {
        if (!transactionDraft) {
            return null;
        }

        if (transactionDraft.type !== TransactionType.Expense &&
            transactionDraft.type !== TransactionType.Income &&
            transactionDraft.type !== TransactionType.Transfer) {
            return null;
        }

        const transaction: Transaction = new Transaction(
            '', // id
            '', // timeSequenceId
            transactionDraft.type, // type
            transactionDraft.categoryId ?? '', // categoryId
            0, // time
            undefined, // only in new transaction
            0, // utcOffset
            transactionDraft.sourceAccountId ?? '', // sourceAccountId
            transactionDraft.destinationAccountId ?? '', // destinationAccountId
            transactionDraft.sourceAmountCents ?? 0, // sourceAmountCents
            transactionDraft.destinationAmountCents ?? 0, // destinationAmountCents
            transactionDraft.hideAmount ?? false, // hideAmount
            transactionDraft.tagIds ?? [], // tagIds
            transactionDraft.comment ?? '', // comment
            true // editable
        );

        if (transactionDraft.pictures) {
            const pictures: TransactionPicture[] = [];

            for (const picture of transactionDraft.pictures) {
                pictures.push(TransactionPicture.of(picture));
            }

            transaction.setPictures(pictures);
        }

        return transaction;
    }
}

/** 交易地理位置请求模型，保持经纬度、地址和坐标类型字段的 API 合同。 */
export class TransactionGeoLocation implements TransactionGeoLocationRequest {
    public latitude: number;
    public longitude: number;

    private constructor(latitude: number, longitude: number) {
        this.latitude = latitude;
        this.longitude = longitude;
    }

    public static createNewGeoLocation(latitude: number, longitude: number): TransactionGeoLocation {
        return new TransactionGeoLocation(latitude, longitude);
    }

    public static of(coordinate: Coordinate): TransactionGeoLocation {
        return new TransactionGeoLocation(coordinate.latitude, coordinate.longitude);
    }

    public toNormalizedCoordinate(): Coordinate {
        return getNormalizedCoordinate(this);
    }
}

export * from './transaction/contracts.ts';
