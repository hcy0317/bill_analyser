import { TransactionType } from '@/core/transaction.ts';

import type { ImportMatchingPayload } from './import_matching.ts';
import type { TransactionCreateRequest, TransactionGeoLocationResponse } from './transaction.ts';

function getFirstNonEmptyString(...values: Array<string | null | undefined>): string {
    for (const value of values) {
        if (typeof value === 'string' && value) {
            return value;
        }
    }

    return '';
}

function getFirstDefinedIdString(...values: Array<number | string | null | undefined>): string {
    for (const value of values) {
        if (typeof value === 'number') {
            return String(value);
        }

        if (typeof value === 'string' && value) {
            return value;
        }
    }

    return '';
}

function getSuggestedTypeFromMatchingCandidate(candidateType: string | undefined): number | undefined {
    const normalizedCandidateType = (candidateType || '').trim().toLowerCase();
    if (
        normalizedCandidateType === '转账'
        || normalizedCandidateType === '4'
        || normalizedCandidateType.includes('transfer')
    ) {
        return TransactionType.Transfer;
    }

    return undefined;
}

function normalizeDedupSourceIds(rawValue: Array<number | string> | string | undefined): Array<number | string> {
    if (Array.isArray(rawValue)) {
        return rawValue.map(value => {
            if (typeof value === 'string') {
                const trimmedValue = value.trim();
                const parsedValue = Number(trimmedValue);
                return Number.isNaN(parsedValue) ? trimmedValue : parsedValue;
            }

            return value;
        }).filter(value => value !== '');
    }

    if (typeof rawValue === 'string' && rawValue) {
        return rawValue.split(',').map(value => value.trim()).filter(value => !!value).map(value => {
            const parsedValue = Number(value);
            return Number.isNaN(parsedValue) ? value : parsedValue;
        });
    }

    return [];
}

function hasDedupSourceIds(rawValue: Array<number | string> | string | undefined): boolean {
    return normalizeDedupSourceIds(rawValue).length > 0;
}

type SparseImportMatchingPayload = {
    transfer?: Partial<ImportMatchingPayload['transfer']>;
    investment?: Partial<ImportMatchingPayload['investment']>;
    learning?: Partial<ImportMatchingPayload['learning']>;
    recurring?: Partial<ImportMatchingPayload['recurring']>;
    dedup?: Partial<ImportMatchingPayload['dedup']>;
    parser?: Partial<ImportMatchingPayload['parser']>;
    annotation?: Partial<ImportMatchingPayload['annotation']>;
    reconciliation?: ImportMatchingPayload['reconciliation'];
    stage2_baseline?: ImportMatchingPayload['stage2_baseline'];
};

function normalizeImportMatchingPayload(matching?: SparseImportMatchingPayload): ImportMatchingPayload | undefined {
    if (!matching) {
        return undefined;
    }

    const parser = matching.parser;
    const parserTags = Array.isArray(parser?.tags) ? parser.tags : [];
    const dedupSourceIds = normalizeDedupSourceIds(matching.dedup?.source_ids);

    return {
        transfer: {
            candidate_type: '',
            score: 0,
            level: '',
            reason: '',
            review_status: '',
            reviewed_type: '',
            suppressed: false,
            pair_order: '',
            source_chain: [],
            ...matching.transfer,
        },
        investment: {
            score: 0,
            level: '',
            reason: '',
            platform: '',
            product: '',
            review_status: '',
            suppressed: false,
            ...matching.investment,
        },
        learning: {
            rule_id: null,
            score: 0,
            level: '',
            reason: '',
            recommended_type: '',
            summary: '',
            review_status: '',
            suppressed: false,
            source: '',
            mode: '',
            auto_apply: false,
            model_version: '',
            recommendation_key: '',
            lifecycle_status: '',
            signal_state: '',
            accepted_count: 0,
            rejected_count: 0,
            auto_applied_count: 0,
            ...matching.learning,
        },
        recurring: {
            id: null,
            name: '',
            candidate_count: 0,
            match_score: 0,
            match_reasons: '',
            matched_date: '',
            ...matching.recurring,
        },
        dedup: {
            type: matching.dedup?.type || '',
            source_ids: dedupSourceIds,
            source_count: matching.dedup?.source_count ?? dedupSourceIds.length,
            source_labels: matching.dedup?.source_labels || [],
            sources: matching.dedup?.sources || [],
        },
        parser: {
            id: parser?.id || '',
            tags: parserTags,
            source_chain: parser?.source_chain || [],
        },
        annotation: {
            is_manually_annotated: !!matching.annotation?.is_manually_annotated,
        },
        reconciliation: matching.reconciliation || {},
        stage2_baseline: matching.stage2_baseline,
    };
}

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
    public sourceAmountCents: number;
    public destinationAmountCents: number;
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
    public learningRecommendationScore: number;
    public learningRecommendationLevel: string;
    public learningRecommendationReason: string;
    public learningRecommendationType: string;
    public learningRecommendationSummary: string;
    public investmentPlatform: string;
    public investmentProduct: string;
    public recurringTemplateId: string;
    public recurringTemplateName: string;
    public recurringCandidateCount: number;
    public recurringMatchScore: number;
    public recurringMatchReasons: string;
    public recurringMatchedDate: string;

    // v7: 解析器来源标识
    public parserId: string;
    public parserTags: string[];
    public dedupType: string;
    public dedupSourceIds: Array<number | string>;
    public matching?: ImportMatchingPayload;

    // v7: 标记用户是否已人工标注
    public isManuallyAnnotated: boolean;

    public actualCategoryName: string;
    public actualSourceAccountName: string;
    public actualDestinationAccountName?: string;
    public index: number;
    public selected: boolean;
    public valid: boolean;

    private constructor(response: ImportTransactionResponse, index: number) {
        const matching = normalizeImportMatchingPayload(response.matching as SparseImportMatchingPayload | undefined);
        const transfer = matching?.transfer;
        const investment = matching?.investment;
        const learning = matching?.learning;
        const recurring = matching?.recurring;
        const parser = matching?.parser;
        const dedup = matching?.dedup;
        const annotation = matching?.annotation;

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
        this.sourceAmountCents = response.sourceAmountCents;
        this.destinationAmountCents = response.destinationAmountCents || 0;
        this.tagIds = response.tagIds || [];
        this.originalTagNames = response.originalTagNames || [];
        this.comment = response.comment;
        this.geoLocation = response.geoLocation;
        // v6.32新增
        this.counterparty = response.counterparty || '';
        this.paymentMethod = response.paymentMethod || '';
        this.suggestedType = getSuggestedTypeFromMatchingCandidate(transfer?.candidate_type) ?? response.suggestedType;
        this.transferSuggestionScore = (transfer?.score ?? 0) > 0
            ? (transfer?.score ?? 0)
            : (response.transferSuggestionScore || 0);
        this.transferSuggestionLevel = getFirstNonEmptyString(transfer?.level, response.transferSuggestionLevel);
        this.transferSuggestionReason = getFirstNonEmptyString(transfer?.reason, response.transferSuggestionReason);
        this.investmentSignalScore = (investment?.score ?? 0) > 0
            ? (investment?.score ?? 0)
            : (response.investmentSignalScore || 0);
        this.investmentSignalLevel = getFirstNonEmptyString(investment?.level, response.investmentSignalLevel);
        this.investmentSignalReason = getFirstNonEmptyString(investment?.reason, response.investmentSignalReason);
        this.learningRecommendationScore = (learning?.score ?? 0) > 0
            ? (learning?.score ?? 0)
            : (response.learningRecommendationScore || 0);
        this.learningRecommendationLevel = getFirstNonEmptyString(learning?.level, response.learningRecommendationLevel);
        this.learningRecommendationReason = getFirstNonEmptyString(learning?.reason, response.learningRecommendationReason);
        this.learningRecommendationType = getFirstNonEmptyString(learning?.recommended_type, response.learningRecommendationType);
        this.learningRecommendationSummary = getFirstNonEmptyString(learning?.summary, response.learningRecommendationSummary);
        this.investmentPlatform = getFirstNonEmptyString(investment?.platform, response.investmentPlatform);
        this.investmentProduct = getFirstNonEmptyString(investment?.product, response.investmentProduct);
        this.recurringTemplateId = getFirstDefinedIdString(recurring?.id, response.recurringTemplateId);
        this.recurringTemplateName = getFirstNonEmptyString(recurring?.name, response.recurringTemplateName);
        this.recurringCandidateCount = (recurring?.candidate_count ?? 0) > 0
            ? (recurring?.candidate_count ?? 0)
            : (response.recurringCandidateCount || 0);
        this.recurringMatchScore = (recurring?.match_score ?? 0) > 0
            ? (recurring?.match_score ?? 0)
            : (response.recurringMatchScore || 0);
        this.recurringMatchReasons = getFirstNonEmptyString(recurring?.match_reasons, response.recurringMatchReasons);
        this.recurringMatchedDate = getFirstNonEmptyString(recurring?.matched_date, response.recurringMatchedDate);

        this.parserId = getFirstNonEmptyString(parser?.id);
        this.parserTags = Array.isArray(parser?.tags) && parser.tags.length > 0
            ? parser.tags
            : [];
        this.dedupType = getFirstNonEmptyString(dedup?.type, response.dedupType);
        this.dedupSourceIds = hasDedupSourceIds(dedup?.source_ids)
            ? normalizeDedupSourceIds(dedup?.source_ids)
            : normalizeDedupSourceIds(response.dedupSourceIds);
        this.matching = matching;
        this.isManuallyAnnotated = !!annotation?.is_manually_annotated || !!response.isManuallyAnnotated;

        this.actualCategoryName = response.originalCategoryName;
        this.actualSourceAccountName = response.originalSourceAccountName;
        this.actualDestinationAccountName = response.originalDestinationAccountName;
        this.index = index;
        this.selected = !!response.selected;
        this.valid = this.isTransactionValid();
    }

    /**
     * 判断交易是否需要目标账户（转账或投资类型）
     */
    public requiresDestinationAccount(): boolean {
        return this.type === TransactionType.Transfer || this.type === TransactionType.Investment;
    }

    public hasTransferSuggestion(): boolean {
        return !!this.suggestedType
            && this.suggestedType === TransactionType.Transfer
            && this.type !== TransactionType.Transfer
            && this.transferSuggestionScore > 0
            && !this.isTransferSuggestionSuppressed()
            && !this.isTransferSuggestionAccepted();
    }

    public getTransferSuggestionReviewStatus(): string {
        return (this.matching?.transfer?.review_status || '').trim().toLowerCase();
    }

    public isTransferSuggestionSuppressed(): boolean {
        return !!this.matching?.transfer?.suppressed || this.isTransferSuggestionRejected();
    }

    public isTransferSuggestionAccepted(): boolean {
        return this.getTransferSuggestionReviewStatus() === 'accepted';
    }

    public isTransferSuggestionRejected(): boolean {
        return this.getTransferSuggestionReviewStatus() === 'rejected';
    }

    public canClearTransferSuggestionDecision(): boolean {
        return this.isTransferSuggestionAccepted() || this.isTransferSuggestionRejected();
    }

    public resetTransferSuggestionDecisionState(): void {
        if (!this.matching?.transfer) {
            return;
        }

        const hasTransferCandidate = !!this.suggestedType
            && this.suggestedType === TransactionType.Transfer
            && this.transferSuggestionScore > 0;

        this.matching.transfer.review_status = hasTransferCandidate ? 'pending' : '';
        this.matching.transfer.reviewed_type = '';
        this.matching.transfer.suppressed = false;
    }

    public hasInvestmentSignal(): boolean {
        return this.type === TransactionType.Investment && this.investmentSignalScore > 0;
    }

    public hasPendingInvestmentSignal(): boolean {
        return this.hasInvestmentSignal()
            && !this.isInvestmentSignalAccepted()
            && !this.isInvestmentSignalRejected()
            && !this.isInvestmentSignalSuppressed();
    }

    public getInvestmentSignalReviewStatus(): string {
        return (this.matching?.investment?.review_status || '').trim().toLowerCase();
    }

    public isInvestmentSignalSuppressed(): boolean {
        return !!this.matching?.investment?.suppressed || this.isInvestmentSignalRejected();
    }

    public isInvestmentSignalAccepted(): boolean {
        return this.getInvestmentSignalReviewStatus() === 'accepted';
    }

    public isInvestmentSignalRejected(): boolean {
        return this.getInvestmentSignalReviewStatus() === 'rejected';
    }

    public hasLearningRecommendation(): boolean {
        if (this.isTransferProtectedLearningSkip()) {
            return false;
        }

        const learningRuleId = this.matching?.learning?.rule_id;
        return this.learningRecommendationScore > 0
            || !!this.learningRecommendationSummary
            || !!this.learningRecommendationReason
            || !!this.learningRecommendationType
            || (typeof learningRuleId === 'number' && learningRuleId > 0)
            || this.getLearningRecommendationReviewStatus() !== '';
    }

    public hasPendingLearningRecommendation(): boolean {
        return this.hasLearningRecommendation()
            && !this.isLearningRecommendationAccepted()
            && !this.isLearningRecommendationRejected()
            && !this.isLearningRecommendationSkipped()
            && !this.isLearningRecommendationSuppressed();
    }

    public getLearningRecommendationReviewStatus(): string {
        return (this.matching?.learning?.review_status || '').trim().toLowerCase();
    }

    public isLearningRecommendationSuppressed(): boolean {
        return !!this.matching?.learning?.suppressed || this.isLearningRecommendationRejected();
    }

    public isLearningRecommendationAccepted(): boolean {
        const reviewStatus = this.getLearningRecommendationReviewStatus();
        return reviewStatus === 'accepted'
            || reviewStatus === 'auto_applied'
            || reviewStatus === 'auto-applied';
    }

    public isLearningRecommendationRejected(): boolean {
        return this.getLearningRecommendationReviewStatus() === 'rejected';
    }

    public isLearningRecommendationSkipped(): boolean {
        return this.getLearningRecommendationReviewStatus() === 'skipped';
    }

    public isTransferProtectedLearningSkip(): boolean {
        if (!this.isLearningRecommendationSkipped()) {
            return false;
        }

        const reason = getFirstNonEmptyString(
            this.matching?.learning?.reason,
            this.learningRecommendationReason
        ).trim().toLowerCase();
        if (!reason.includes('transfer preview is protected')) {
            return false;
        }

        const transferCandidateType = (this.matching?.transfer?.candidate_type || '').trim().toLowerCase();
        const dedupType = getFirstNonEmptyString(this.matching?.dedup?.type, this.dedupType)
            .trim()
            .toLowerCase();
        return transferCandidateType === '转账'
            || transferCandidateType.includes('transfer')
            || dedupType.includes('transfer');
    }

    public canClearLearningRecommendationDecision(): boolean {
        return this.isLearningRecommendationAccepted() || this.isLearningRecommendationRejected();
    }

    public getLearningRecommendationInputFingerprint(): string {
        return JSON.stringify({
            parserId: (this.parserId || '').trim(),
            counterparty: (this.counterparty || '').trim(),
            paymentMethod: (this.paymentMethod || '').trim(),
            comment: (this.comment || '').trim()
        });
    }

    public getInvestmentProfileText(): string {
        return [this.investmentPlatform, this.investmentProduct].filter(item => !!item).join(' · ');
    }

    public hasRecurringMatch(): boolean {
        return !!this.recurringTemplateId;
    }

    public hasMatchingDedupContext(): boolean {
        return this.dedupType !== '' && this.dedupType !== 'remaining' && this.dedupSourceIds.length > 0;
    }

    public hasMatchingContextSummary(): boolean {
        return !!this.parserId || this.hasMatchingDedupContext() || this.isManuallyAnnotated;
    }

    public getMatchingDedupSummary(): string {
        return [
            this.dedupType,
            this.dedupSourceIds.length > 0 ? this.dedupSourceIds.join('|') : ''
        ].filter(text => !!text).join(' | ');
    }

    public getMatchingParserTagText(): string {
        return (this.parserTags || []).filter(tag => !!tag).join(' · ');
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
            sourceAmountCents: this.sourceAmountCents,
            destinationAmountCents: needsDestAccount ? this.destinationAmountCents : 0,
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
    readonly sourceAmountCents: string;
    readonly destinationAmountCents?: string;
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
    readonly sourceAmountCents: number;
    readonly destinationAmountCents?: number;
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
    readonly learningRecommendationScore?: number;
    readonly learningRecommendationLevel?: string;
    readonly learningRecommendationReason?: string;
    readonly learningRecommendationType?: string;
    readonly learningRecommendationSummary?: string;
    readonly investmentPlatform?: string;
    readonly investmentProduct?: string;
    readonly recurringTemplateId?: string;
    readonly recurringTemplateName?: string;
    readonly recurringCandidateCount?: number;
    readonly recurringMatchScore?: number;
    readonly recurringMatchReasons?: string;
    readonly recurringMatchedDate?: string;
    readonly dedupType?: string;
    readonly dedupSourceIds?: Array<number | string> | string;
    readonly matching?: ImportMatchingPayload;
    readonly isManuallyAnnotated?: boolean;
    readonly selected?: boolean;
}

export interface ImportTransactionResponsePageWrapper {
    readonly items: ImportTransactionResponse[];
    readonly totalCount: number;
}
