<template>
    <f7-popover class="date-popover-menu"
                :opened="showDatePopover"
                @update:opened="emit('update:showDatePopover', $event)"
                @popover:open="emit('popoverOpen', $event)">
        <f7-list dividers>
            <f7-list-item :title="dateRange.displayName"
                          :class="{ 'list-item-selected': queryDateType === dateRange.type }"
                          :key="dateRange.type"
                          v-for="dateRange in allDateRanges"
                          @click="emit('dateFilter', dateRange.type)">
                <template #after>
                    <f7-icon class="list-item-checked-icon" f7="checkmark_alt" v-if="queryDateType === dateRange.type"></f7-icon>
                </template>
                <template #footer>
                    <div v-if="dateRange.isUserCustomRange && canShowCustomDateRange(dateRange.type)">
                        <span>{{ queryStartTime }}</span>
                        <span>&nbsp;-&nbsp;</span>
                        <br/>
                        <span>{{ queryEndTime }}</span>
                    </div>
                </template>
            </f7-list-item>
        </f7-list>
    </f7-popover>

    <f7-popover class="date-aggregation-popover-menu"
                :opened="showDateAggregationPopover"
                @update:opened="emit('update:showDateAggregationPopover', $event)"
                @popover:open="emit('popoverOpen', $event)">
        <f7-list dividers v-if="analysisType === StatisticsAnalysisType.TrendAnalysis">
            <f7-list-item :title="aggregationType.displayName"
                          :class="{ 'list-item-selected': trendDateAggregationType === aggregationType.type }"
                          :key="aggregationType.type"
                          v-for="aggregationType in allTrendAnalysisDateAggregationTypes"
                          @click="emit('trendDateAggregationType', aggregationType.type)">
                <template #after>
                    <f7-icon class="list-item-checked-icon" f7="checkmark_alt" v-if="trendDateAggregationType === aggregationType.type"></f7-icon>
                </template>
            </f7-list-item>
        </f7-list>
        <f7-list dividers v-else-if="analysisType === StatisticsAnalysisType.AssetTrends">
            <f7-list-item :title="aggregationType.displayName"
                          :class="{ 'list-item-selected': assetTrendsDateAggregationType === aggregationType.type }"
                          :key="aggregationType.type"
                          v-for="aggregationType in allAssetTrendsDateAggregationTypes"
                          @click="emit('assetTrendsDateAggregationType', aggregationType.type)">
                <template #after>
                    <f7-icon class="list-item-checked-icon" f7="checkmark_alt" v-if="assetTrendsDateAggregationType === aggregationType.type"></f7-icon>
                </template>
            </f7-list-item>
        </f7-list>
    </f7-popover>

    <date-range-selection-sheet :title="tt('Custom Date Range')"
                                :min-time="query.categoricalChartStartTime"
                                :max-time="query.categoricalChartEndTime"
                                :show="showCustomDateRangeSheet"
                                @update:show="emit('update:showCustomDateRangeSheet', $event)"
                                @dateRange:change="onCustomDateRangeChange">
    </date-range-selection-sheet>

    <month-range-selection-sheet :title="tt('Custom Date Range')"
                                 :min-time="query.trendChartStartYearMonth"
                                 :max-time="query.trendChartEndYearMonth"
                                 :show="showCustomMonthRangeSheet"
                                 @update:show="emit('update:showCustomMonthRangeSheet', $event)"
                                 @dateRange:change="onCustomDateRangeChange">
    </month-range-selection-sheet>

    <f7-actions close-by-outside-click close-on-escape :opened="showMoreActionSheet" @actions:closed="emit('update:showMoreActionSheet', false)">
        <f7-actions-group>
            <f7-actions-button :class="{ 'disabled': reloading }" @click="emit('filterAccounts')">{{ tt('Filter Accounts') }}</f7-actions-button>
            <f7-actions-button :class="{ 'disabled': reloading }" @click="emit('filterCategories')" v-if="canUseCategoryFilter">{{ tt('Filter Transaction Categories') }}</f7-actions-button>
            <f7-actions-button :class="{ 'disabled': reloading }" @click="emit('filterTags')" v-if="canUseTagFilter">{{ tt('Filter Transaction Tags') }}</f7-actions-button>
        </f7-actions-group>
        <f7-actions-group v-if="canUseKeywordFilter">
            <f7-actions-label v-if="query.keyword">{{ query.keyword }}</f7-actions-label>
            <f7-actions-button :class="{ 'disabled': reloading }" @click="emit('filterDescription')">{{ tt('Filter transaction description') }}</f7-actions-button>
        </f7-actions-group>
        <f7-actions-group>
            <f7-actions-button @click="emit('settings')">{{ tt('Settings') }}</f7-actions-button>
        </f7-actions-group>
        <f7-actions-group>
            <f7-actions-button bold close>{{ tt('Cancel') }}</f7-actions-button>
        </f7-actions-group>
    </f7-actions>
</template>

<script setup lang="ts">
import { useI18n } from '@/locales/helpers.ts';

import type { TypeAndDisplayName } from '@/core/base.ts';
import type { LocalizedDateRange, TextualYearMonth } from '@/core/datetime.ts';
import { StatisticsAnalysisType } from '@/core/statistics.ts';
import type { Framework7Dom } from '@/lib/ui/mobile.ts';
import type { TransactionStatisticsFilter } from '@/stores/statistics.ts';

defineProps<{
    showDatePopover: boolean;
    showDateAggregationPopover: boolean;
    showCustomDateRangeSheet: boolean;
    showCustomMonthRangeSheet: boolean;
    showMoreActionSheet: boolean;
    analysisType: StatisticsAnalysisType;
    reloading: boolean;
    query: TransactionStatisticsFilter;
    queryDateType: number | null;
    queryStartTime: string;
    queryEndTime: string;
    allDateRanges: LocalizedDateRange[];
    trendDateAggregationType: number;
    assetTrendsDateAggregationType: number;
    allTrendAnalysisDateAggregationTypes: TypeAndDisplayName[];
    allAssetTrendsDateAggregationTypes: TypeAndDisplayName[];
    canUseCategoryFilter: boolean;
    canUseTagFilter: boolean;
    canUseKeywordFilter: boolean;
    canShowCustomDateRange: (dateRangeType: number) => boolean;
}>();

const emit = defineEmits<{
    'update:showDatePopover': [opened: boolean];
    'update:showDateAggregationPopover': [opened: boolean];
    'update:showCustomDateRangeSheet': [opened: boolean];
    'update:showCustomMonthRangeSheet': [opened: boolean];
    'update:showMoreActionSheet': [opened: boolean];
    popoverOpen: [event: { $el: Framework7Dom }];
    dateFilter: [dateType: number];
    trendDateAggregationType: [type: number];
    assetTrendsDateAggregationType: [type: number];
    customDateFilter: [startTime: number | TextualYearMonth, endTime: number | TextualYearMonth];
    filterAccounts: [];
    filterCategories: [];
    filterTags: [];
    filterDescription: [];
    settings: [];
}>();

const { tt } = useI18n();

function onCustomDateRangeChange(startTime: number | TextualYearMonth, endTime: number | TextualYearMonth): void {
    emit('customDateFilter', startTime, endTime);
}
</script>
