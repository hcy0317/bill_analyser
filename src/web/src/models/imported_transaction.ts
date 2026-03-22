import { TransactionType } from '@/core/transaction.ts';

import type { TransactionCreateRequest, TransactionGeoLocationResponse } from './transaction.ts';

export class ImportTransaction implements ImportTransactionResponse {
    public type: number;
    public categoryId: string;
    public originalCategoryName: string;
    public time: number;
    public utcOffset: number;
    public sourceAccountId: string;
    public originalSourceAccountName: string;
    public originalSourceAccountCurrency: string;
    public destinationAccountId: string;
    public originalDestinationAccountName?: string;
    public originalDestinationAccountCurrency?: string;
    public sourceAmount: number;
    public destinationAmount: number;
    public tagIds: string[];
    public originalTagNames: string[];
    public comment: string;
    public geoLocation?: TransactionGeoLocationResponse;
    // v6.32新增: 交易对方和支付方式字段
    public counterparty: string;
    public paymentMethod: string;
    public suggestedType?: number;
    public transferSuggestionScore: number;
    public transferSuggestionLevel: string;
    public transferSuggestionReason: string;
    public investmentSignalScore: number;
    public investmentSignalLevel: string;
    public investmentSignalReason: string;
    public investmentPlatform: string;
    public investmentProduct: string;
    public recurringTemplateId: string;
    public recurringTemplateName: string;
    public recurringCandidateCount: number;
    public recurringMatchScore: number;
    public recurringMatchReasons: string;
    public recurringMatchedDate: string;

    public actualCategoryName: string;
    public actualSourceAccountName: string;
    public actualDestinationAccountName?: string;
    public index: number;
    public selected: boolean;
    public valid: boolean;

    private constructor(response: ImportTransactionResponse, index: number) {
        this.type = response.type;
        this.categoryId = response.categoryId;
        this.originalCategoryName = response.originalCategoryName;
        this.time = response.time;
        this.utcOffset = response.utcOffset;
        this.sourceAccountId = response.sourceAccountId;
        this.originalSourceAccountName = response.originalSourceAccountName;
        this.originalSourceAccountCurrency = response.originalSourceAccountCurrency;
        this.destinationAccountId = response.destinationAccountId || '';
        this.originalDestinationAccountName = response.originalDestinationAccountName;
        this.originalDestinationAccountCurrency = response.originalDestinationAccountCurrency;
        this.sourceAmount = response.sourceAmount;
        this.destinationAmount = response.destinationAmount || 0;
        this.tagIds = response.tagIds || [];
        this.originalTagNames = response.originalTagNames || [];
        this.comment = response.comment;
        this.geoLocation = response.geoLocation;
        // v6.32新增
        this.counterparty = response.counterparty || '';
        this.paymentMethod = response.paymentMethod || '';
        this.suggestedType = response.suggestedType;
        this.transferSuggestionScore = response.transferSuggestionScore || 0;
        this.transferSuggestionLevel = response.transferSuggestionLevel || '';
        this.transferSuggestionReason = response.transferSuggestionReason || '';
        this.investmentSignalScore = response.investmentSignalScore || 0;
        this.investmentSignalLevel = response.investmentSignalLevel || '';
        this.investmentSignalReason = response.investmentSignalReason || '';
        this.investmentPlatform = response.investmentPlatform || '';
        this.investmentProduct = response.investmentProduct || '';
        this.recurringTemplateId = response.recurringTemplateId || '';
        this.recurringTemplateName = response.recurringTemplateName || '';
        this.recurringCandidateCount = response.recurringCandidateCount || 0;
        this.recurringMatchScore = response.recurringMatchScore || 0;
        this.recurringMatchReasons = response.recurringMatchReasons || '';
        this.recurringMatchedDate = response.recurringMatchedDate || '';

        this.actualCategoryName = response.originalCategoryName;
        this.actualSourceAccountName = response.originalSourceAccountName;
        this.actualDestinationAccountName = response.originalDestinationAccountName;
        this.index = index;
        this.selected = false;
        this.valid = this.isTransactionValid();
    }

    /**
     * 判断交易是否需要目标账户（转账或投资类型）
     */
    public requiresDestinationAccount(): boolean {
        return this.type === TransactionType.Transfer || this.type === TransactionType.Investment;
    }

    public hasTransferSuggestion(): boolean {
        return !!this.suggestedType &&
            this.suggestedType === TransactionType.Transfer &&
            this.type !== TransactionType.Transfer &&
            this.transferSuggestionScore > 0;
    }

    public hasInvestmentSignal(): boolean {
        return this.type === TransactionType.Investment && this.investmentSignalScore > 0;
    }

    public getInvestmentProfileText(): string {
        return [this.investmentPlatform, this.investmentProduct].filter(item => !!item).join(' · ');
    }

    public hasRecurringMatch(): boolean {
        return !!this.recurringTemplateId;
    }

    public clearRecurringMatch(resetCandidateCount: boolean = true): void {
        this.recurringTemplateId = '';
        this.recurringTemplateName = '';
        if (resetCandidateCount) {
            this.recurringCandidateCount = 0;
        }
        this.recurringMatchScore = 0;
        this.recurringMatchReasons = '';
        this.recurringMatchedDate = '';
    }

    public toCreateRequest(): TransactionCreateRequest {
        // 转账和投资类型都需要目标账户
        const needsDestAccount = this.requiresDestinationAccount();

        return {
            type: this.type,
            categoryId: this.categoryId,
            time: this.time,
            utcOffset: this.utcOffset,
            sourceAccountId: this.sourceAccountId,
            destinationAccountId: needsDestAccount ? this.destinationAccountId : '0',
            sourceAmount: this.sourceAmount,
            destinationAmount: needsDestAccount ? this.destinationAmount : 0,
            hideAmount: false,
            tagIds: this.tagIds,
            pictureIds: [],
            comment: this.comment,
            geoLocation: this.geoLocation,
            clientSessionId: ''
        };
    }

    public isTransactionValid(): boolean {
        if (this.type !== TransactionType.ModifyBalance && (!this.categoryId || this.categoryId === '0')) {
            return false;
        }

        if (!this.sourceAccountId || this.sourceAccountId === '0') {
            return false;
        }

        // 转账和投资类型都需要验证目标账户
        if (this.requiresDestinationAccount() && (!this.destinationAccountId || this.destinationAccountId === '0')) {
            return false;
        }

        if (this.tagIds && this.tagIds.length) {
            for (const tagId of this.tagIds) {
                if (!tagId || tagId === '0') {
                    return false;
                }
            }
        }

        return true;
    }

    public static of(response: ImportTransactionResponse, index: number): ImportTransaction {
        return new ImportTransaction(response, index);
    }
}

export interface ImportTransactionRequest {
    readonly transactions: ImportTransactionRequestItem[];
}

export interface ImportTransactionRequestItem {
    readonly time: string;
    readonly utcOffset: string;
    readonly type: string;
    readonly categoryName?: string;
    readonly sourceAccountName?: string;
    readonly destinationAccountName?: string;
    readonly sourceAmount: string;
    readonly destinationAmount?: string;
    readonly geoLocation?: string;
    readonly tagNames?: string;
    readonly comment?: string;
}

export interface ImportTransactionResponse {
    readonly type: number;
    readonly categoryId: string;
    readonly originalCategoryName: string;
    readonly time: number;
    readonly utcOffset: number;
    readonly sourceAccountId: string;
    readonly originalSourceAccountName: string;
    readonly originalSourceAccountCurrency: string;
    readonly destinationAccountId?: string;
    readonly originalDestinationAccountName?: string;
    readonly originalDestinationAccountCurrency?: string;
    readonly sourceAmount: number;
    readonly destinationAmount?: number;
    readonly tagIds: string[];
    readonly originalTagNames: string[];
    readonly comment: string;
    readonly geoLocation?: TransactionGeoLocationResponse;
    // v6.32新增: 交易对方和支付方式字段
    readonly counterparty?: string;
    readonly paymentMethod?: string;
    readonly suggestedType?: number;
    readonly transferSuggestionScore?: number;
    readonly transferSuggestionLevel?: string;
    readonly transferSuggestionReason?: string;
    readonly investmentSignalScore?: number;
    readonly investmentSignalLevel?: string;
    readonly investmentSignalReason?: string;
    readonly investmentPlatform?: string;
    readonly investmentProduct?: string;
    readonly recurringTemplateId?: string;
    readonly recurringTemplateName?: string;
    readonly recurringCandidateCount?: number;
    readonly recurringMatchScore?: number;
    readonly recurringMatchReasons?: string;
    readonly recurringMatchedDate?: string;
}

export interface ImportTransactionResponsePageWrapper {
    readonly items: ImportTransactionResponse[];
    readonly totalCount: number;
}
