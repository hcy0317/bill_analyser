import { type PartialRecord } from '@/core/base.ts';
import type { Year1BasedMonth, YearMonthDay, StartEndTime } from '@/core/datetime.ts';
import type { Coordinate } from '@/core/coordinate.ts';
import type { Account, AccountInfoResponse } from '../account.ts';
import type { TransactionCategory, TransactionCategoryInfoResponse } from '../transaction_category.ts';
import type { TransactionTagInfoResponse } from '../transaction_tag.ts';
import type { TransactionPictureInfoBasicResponse } from '../transaction_picture_info.ts';
import type { Transaction } from '../transaction.ts';

export interface TransactionDraft {
    readonly type?: number;
    readonly categoryId?: string;
    readonly sourceAccountId?: string;
    readonly sourceAmountCents?: number;
    readonly destinationAccountId?: string;
    readonly destinationAmountCents?: number;
    readonly hideAmount?: boolean;
    readonly tagIds?: string[];
    readonly pictures?: TransactionPictureInfoBasicResponse[];
    readonly comment?: string;
}

export interface TransactionGeoLocationRequest {
    readonly latitude: number;
    readonly longitude: number;
}

export interface TransactionCreateRequest {
    readonly type: number;
    readonly categoryId: string;
    readonly time: number;
    readonly utcOffset: number;
    readonly sourceAccountId: string;
    readonly destinationAccountId: string;
    readonly sourceAmountCents: number;
    readonly destinationAmountCents: number;
    readonly hideAmount: boolean;
    readonly tagIds: string[];
    readonly pictureIds: string[];
    readonly comment: string;
    readonly geoLocation?: TransactionGeoLocationRequest;
    readonly clientSessionId: string;
}

export interface TransactionModifyRequest {
    readonly id: string;
    readonly type: number;  // 添加type字段，确保编辑时保持账单类型
    readonly categoryId: string;
    readonly time: number;
    readonly utcOffset: number;
    readonly sourceAccountId: string;
    readonly destinationAccountId: string;
    readonly sourceAmountCents: number;
    readonly destinationAmountCents: number;
    readonly hideAmount: boolean;
    readonly tagIds: string[];
    readonly pictureIds: string[];
    readonly comment: string;
    readonly geoLocation?: TransactionGeoLocationRequest;
}

export interface TransactionMoveBetweenAccountsRequest {
    readonly fromAccountId: string;
    readonly toAccountId: string;
    readonly password: string;
}

export interface TransactionDeleteRequest {
    readonly id: string;
}

export interface TransactionImportRequest {
    readonly transactions: TransactionCreateRequest[];
    readonly clientSessionId: string;
}

export interface TransactionListByMaxTimeRequest {
    readonly maxTime: number;
    readonly minTime: number;
    readonly count: number;
    readonly page: number;
    readonly withCount: boolean;
    readonly type: number;
    readonly categoryIds: string;
    readonly accountIds: string;
    readonly flowDirection?: string;
    readonly tagIds: string;
    readonly tagFilterType: number;
    readonly amountFilterCents: string;
    readonly keyword: string;
}

export interface TransactionListInMonthByPageRequest {
    readonly year: number;
    readonly month: number; // 1-based (1 = January, 12 = December)
    readonly type: number;
    readonly categoryIds: string;
    readonly accountIds: string;
    readonly flowDirection?: string;
    readonly tagIds: string;
    readonly tagFilterType: number;
    readonly amountFilterCents: string;
    readonly keyword: string;
}

export interface TransactionReconciliationStatementRequest {
    readonly accountId: string;
    readonly startTime: number;
    readonly endTime: number;
}

export type TransactionGeoLocationResponse = Coordinate;

export interface TransactionInfoResponse {
    readonly id: string;
    readonly timeSequenceId: string;
    readonly type: number;
    readonly categoryId: string;
    readonly category?: TransactionCategoryInfoResponse;
    readonly time: number;
    readonly utcOffset: number;
    readonly sourceAccountId: string;
    readonly sourceAccount?: AccountInfoResponse;
    readonly destinationAccountId: string;
    readonly destinationAccount?: AccountInfoResponse;
    readonly sourceAmountCents: number;
    readonly destinationAmountCents: number;
    readonly hideAmount: boolean;
    readonly tagIds: string[];
    readonly tags?: TransactionTagInfoResponse[];
    readonly pictures?: TransactionPictureInfoBasicResponse[];
    readonly comment: string;
    readonly geoLocation?: TransactionGeoLocationResponse;
    readonly editable: boolean;
    // 日期显示字段 (用于交易列表分组)
    readonly gregorianCalendarYearDashMonthDashDay?: string;  // 格式: "2025-11-21"
    readonly gregorianCalendarDayOfMonth?: number;            // 日期中的天数: 21
    readonly displayDayOfWeek?: number;                       // 星期: 1=周日, 2=周一, ..., 7=周六
}

export interface TransactionStatisticRequest {
    readonly startTime: number;
    readonly endTime: number;
    readonly tagIds: string;
    readonly tagFilterType: number;
    readonly keyword: string;
    readonly useTransactionTimezone: boolean;
}

export interface YearMonthRangeRequest {
    readonly startYearMonth: string;
    readonly endYearMonth: string;
}

export interface TransactionStatisticTrendsRequest extends YearMonthRangeRequest {
    readonly tagIds: string;
    readonly tagFilterType: number;
    readonly keyword: string;
    readonly useTransactionTimezone: boolean;
}

export interface TransactionStatisticAssetTrendsRequest {
    readonly startTime: number;
    readonly endTime: number;
}

export const ALL_TRANSACTION_AMOUNTS_REQUEST_TYPE = [
    'today',
    'thisWeek',
    'thisMonth',
    'thisYear',
    'lastMonth',
    'monthBeforeLastMonth',
    'monthBeforeLast2Months',
    'monthBeforeLast3Months',
    'monthBeforeLast4Months',
    'monthBeforeLast5Months',
    'monthBeforeLast6Months',
    'monthBeforeLast7Months',
    'monthBeforeLast8Months',
    'monthBeforeLast9Months',
    'monthBeforeLast10Months'
] as const;

export type TransactionAmountsRequestType = typeof ALL_TRANSACTION_AMOUNTS_REQUEST_TYPE[number];

export const LATEST_12MONTHS_TRANSACTION_AMOUNTS_REQUEST_TYPES: TransactionAmountsRequestType[] = [
    'monthBeforeLast10Months',
    'monthBeforeLast9Months',
    'monthBeforeLast8Months',
    'monthBeforeLast7Months',
    'monthBeforeLast6Months',
    'monthBeforeLast5Months',
    'monthBeforeLast4Months',
    'monthBeforeLast3Months',
    'monthBeforeLast2Months',
    'monthBeforeLastMonth',
    'lastMonth',
    'thisMonth'
];

export interface TransactionAmountsRequestParams extends PartialRecord<TransactionAmountsRequestType, StartEndTime> {
    readonly useTransactionTimezone: boolean;
    today?: StartEndTime;
    thisWeek?: StartEndTime;
    thisMonth?: StartEndTime;
    thisYear?: StartEndTime;
    lastMonth?: StartEndTime;
    monthBeforeLastMonth?: StartEndTime;
    monthBeforeLast2Months?: StartEndTime;
    monthBeforeLast3Months?: StartEndTime;
    monthBeforeLast4Months?: StartEndTime;
    monthBeforeLast5Months?: StartEndTime;
    monthBeforeLast6Months?: StartEndTime;
    monthBeforeLast7Months?: StartEndTime;
    monthBeforeLast8Months?: StartEndTime;
    monthBeforeLast9Months?: StartEndTime;
    monthBeforeLast10Months?: StartEndTime;
}

/** 统计请求时间窗集合，负责把多组交易金额查询参数序列化为 API payload。 */
export class TransactionAmountsRequest {
    public readonly useTransactionTimezone: boolean;
    public readonly query: string;

    public constructor(useTransactionTimezone: boolean, query: string) {
        this.useTransactionTimezone = useTransactionTimezone;
        this.query = query;
    }

    public buildQuery(): string {
        return `use_transaction_timezone=${this.useTransactionTimezone}` + (this.query.length ? '&query=' + this.query : '');
    }

    public static of(params: TransactionAmountsRequestParams): TransactionAmountsRequest {
        const queryParams: string[] = [];

        ALL_TRANSACTION_AMOUNTS_REQUEST_TYPE.forEach((type) => {
            if (params[type]) {
                queryParams.push(`${type}_${params[type].startTime}_${params[type].endTime}`);
            }
        });

        return new TransactionAmountsRequest(params.useTransactionTimezone, (queryParams.length ? queryParams.join('|') : ''));
    }
}

export interface TransactionInfoPageWrapperResponse {
    readonly items: TransactionInfoResponse[];
    readonly nextTimeSequenceId?: number;
    readonly totalCount?: number;
}

export interface TransactionInfoPageWrapperResponse2 {
    readonly items: TransactionInfoResponse[];
    readonly totalCount: number;
}

export interface TransactionReconciliationStatementResponseItem extends TransactionInfoResponse {
    readonly accountOpeningBalanceCents: number;
    readonly accountClosingBalanceCents: number;
}

export interface TransactionReconciliationStatementResponse {
    readonly transactions: TransactionReconciliationStatementResponseItem[];
    readonly totalInflowsCents: number;
    readonly totalOutflowsCents: number;
    readonly openingBalanceCents: number;
    readonly closingBalanceCents: number;
    readonly netFlowCents: number;
}

export interface TransactionPageWrapper {
    readonly items: Transaction[];
    readonly totalCount?: number;
}

export interface TransactionStatisticResponse {
    readonly startTime: number;
    readonly endTime: number;
    readonly items: TransactionStatisticResponseItem[];
}

export interface TransactionStatisticResponseItem {
    readonly categoryId: string;
    readonly accountId: string;
    readonly relatedAccountId?: string;
    readonly relatedAccountType?: number;
    readonly amountCents: number;
    readonly openingAmountCents?: number;
}

export interface TransactionStatisticTrendsResponseItem {
    readonly year: number;
    readonly month: number; // 1-based (1 = January, 12 = December)
    readonly items: TransactionStatisticResponseItem[];
}

export interface TransactionStatisticAssetTrendsResponseItem extends YearMonthDay {
    readonly year: number;
    readonly month: number; // 1-based (1 = January, 12 = December)
    readonly day: number;
    readonly items: TransactionStatisticAssetTrendsResponseDataItem[];
}

export interface TransactionStatisticAssetTrendsResponseDataItem {
    readonly accountId: string;
    readonly accountOpeningBalanceCents: number;
    readonly accountClosingBalanceCents: number;
}

export interface YearMonthDataItem extends Year1BasedMonth, Record<string, unknown> {}

export interface YearMonthDayDataItem extends YearMonthDay, Record<string, unknown> {}

export interface YearMonthItems<T extends Year1BasedMonth> extends Record<string, unknown> {
    readonly items: T[];
}

export interface YearMonthDayItems<T extends YearMonthDay> extends Record<string, unknown> {
    readonly items: T[];
}

export interface SortableTransactionStatisticDataItem {
    readonly name: string;
    readonly displayOrders: number[];
    readonly totalAmountCents: number;
}

export interface TransactionStatisticResponseItemWithInfo extends TransactionStatisticResponseItem {
    categoryId: string;
    accountId: string;
    relatedAccountId?: string;
    amountCents: number;
    openingAmountCents?: number;
    account?: Account;
    primaryAccount?: Account;
    relatedAccount?: Account;
    relatedPrimaryAccount?: Account;
    relatedAccountType?: number;
    category?: TransactionCategory;
    primaryCategory?: TransactionCategory;
    amountInDefaultCurrencyCents: number | null;
    openingAmountInDefaultCurrencyCents?: number | null;
}

export interface TransactionStatisticResponseWithInfo {
    readonly startTime: number;
    readonly endTime: number;
    readonly items: TransactionStatisticResponseItemWithInfo[];
}

export interface TransactionStatisticTrendsResponseItemWithInfo {
    readonly year: number;
    readonly month: number; // 1-based (1 = January, 12 = December)
    readonly items: TransactionStatisticResponseItemWithInfo[];
}

export interface TransactionStatisticAssetTrendsResponseItemWithInfo {
    readonly year: number;
    readonly month: number; // 1-based (1 = January, 12 = December)
    readonly day: number;
    readonly items: TransactionStatisticResponseItemWithInfo[];
}

export type TransactionStatisticDataItemType = 'category' | 'account' | 'total';

export interface TransactionStatisticDataItemBase extends SortableTransactionStatisticDataItem {
    readonly name: string;
    readonly type: TransactionStatisticDataItemType;
    readonly id: string;
    readonly icon: string;
    readonly color: string;
    readonly hidden: boolean;
    readonly displayOrders: number[];
    readonly totalAmountCents: number;
}

export interface TransactionCategoricalOverviewAnalysisData {
    readonly totalIncomeCents: number;
    readonly totalExpenseCents: number;
    readonly items: TransactionCategoricalOverviewAnalysisDataItem[];
}

export enum TransactionCategoricalOverviewAnalysisDataItemType {
    IncomeByPrimaryCategory = 'incomeByPrimaryCategory',
    IncomeBySecondaryCategory = 'incomeBySecondaryCategory',
    IncomeByAccount = 'incomeByAccount',
    ExpenseByAccount = 'expenseByAccount',
    NetCashFlow = 'netCashFlow',
    ExpenseBySecondaryCategory = 'expenseBySecondaryCategory',
    ExpenseByPrimaryCategory = 'expenseByPrimaryCategory'
}

export interface TransactionCategoricalOverviewAnalysisDataItem extends SortableTransactionStatisticDataItem {
    readonly id: string;
    readonly name: string;
    readonly type: TransactionCategoricalOverviewAnalysisDataItemType;
    readonly displayOrders: number[];
    readonly hidden: boolean;
    readonly inflows: TransactionCategoricalOverviewAnalysisDataItemOutflowItem[];
    readonly outflows: TransactionCategoricalOverviewAnalysisDataItemOutflowItem[];
    totalAmountCents: number;
    totalNonNegativeAmountCents: number;
    includeInPercent?: boolean;
    percent?: number;
}

export interface TransactionCategoricalOverviewAnalysisDataItemOutflowItem {
    readonly relatedItem: TransactionCategoricalOverviewAnalysisDataItem;
    amountCents: number;
}

export interface TransactionCategoricalAnalysisData {
    readonly totalAmountCents: number;
    readonly items: TransactionCategoricalAnalysisDataItem[];
}

export interface TransactionCategoricalAnalysisDataItem extends Record<string, unknown> , TransactionStatisticDataItemBase {
    readonly percent: number;
}

export interface TransactionTrendsAnalysisData {
    readonly items: TransactionTrendsAnalysisDataItem[];
}

export interface TransactionTrendsAnalysisDataItem extends Record<string, unknown>, TransactionStatisticDataItemBase {
    readonly items: TransactionTrendsAnalysisDataAmount[];
}

export interface TransactionTrendsAnalysisDataAmount extends Record<string, unknown>, Year1BasedMonth {
    readonly year: number;
    readonly month1base: number;
    readonly totalAmountCents: number;
}

export interface TransactionAssetTrendsAnalysisData {
    readonly items: TransactionAssetTrendsAnalysisDataItem[];
}

export interface TransactionAssetTrendsAnalysisDataItem extends Record<string, unknown>, TransactionStatisticDataItemBase {
    readonly items: TransactionAssetTrendsAnalysisDataAmount[];
}

export interface TransactionAssetTrendsAnalysisDataAmount extends Record<string, unknown>, YearMonthDay {
    readonly year: number;
    readonly month: number;
    readonly day: number;
    readonly totalAmountCents: number;
    readonly totalOpeningAmountCents?: number;
}

export type TransactionAmountsResponse = PartialRecord<TransactionAmountsRequestType, TransactionAmountsResponseItem>;

export interface TransactionAmountsResponseItem {
    readonly startTime: number;
    readonly endTime: number;
    readonly amounts: TransactionAmountsResponseItemAmountInfo[];
}

export interface TransactionAmountsResponseItemAmountInfo {
    readonly currency: string;
    readonly incomeAmountCents: number;
    readonly expenseAmountCents: number;
}

export type TransactionOverviewResponse = PartialRecord<TransactionAmountsRequestType, TransactionOverviewResponseItem>;

export type TransactionOverviewDisplayTime = PartialRecord<TransactionAmountsRequestType, TransactionOverviewDisplayTimeItem>;

export interface TransactionOverviewDisplayTimeItem {
    readonly displayTime?: string;
    readonly startTime?: string;
    readonly endTime?: string;
}

export interface TransactionOverviewResponseItem {
    readonly valid: boolean;
    readonly incomeAmountCents: number;
    readonly expenseAmountCents: number;
    readonly incompleteIncomeAmount: boolean;
    readonly incompleteExpenseAmount: boolean;
    readonly amounts?: TransactionAmountsResponseItemAmountInfo[];
}

export interface TransactionMonthlyIncomeAndExpenseData {
    readonly monthStartTime: number;
    readonly incomeAmountCents: number;
    readonly expenseAmountCents: number;
    readonly incompleteIncomeAmount: boolean;
    readonly incompleteExpenseAmount: boolean;
}

export const EMPTY_TRANSACTION_RESULT: TransactionPageWrapper = {
    items: [],
    totalCount: 0
}
