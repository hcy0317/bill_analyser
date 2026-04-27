<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-layout>
                    <!-- 左侧导航抽屉 -->
                    <v-navigation-drawer :permanent="alwaysShowNav" v-model="showNav">
                        <div class="mx-6 mt-4">
                            <btn-vertical-group
                                class="budget-nav-buttons"
                                :disabled="loading || forecastLoading"
                                :buttons="viewModeButtons"
                                v-model="activeViewMode"
                                @update:model-value="switchViewMode"
                            />
                        </div>
                        <v-divider class="mt-4" />
                        <div class="mx-6 mt-4">
                            <!-- 类型切换：支出 / 投资（横向排列，宽度与上方按钮一致）-->
                            <btn-horizontal-group class="budget-nav-buttons" :disabled="loading" :buttons="[
                                { name: tt('Expense'), value: BudgetType.Expense },
                                { name: tt('Investment'), value: BudgetType.Investment }
                            ]" v-model="activeBudgetType" @update:model-value="switchBudgetType" />
                        </div>
                        <template v-if="activeViewMode === 'history'">
                            <v-divider class="mt-4" />
                            <v-tabs show-arrows
                                    class="my-4 budget-level-tabs"
                                    direction="vertical"
                                    :disabled="loading"
                                    v-model="historicalBudgetLevel">
                                <v-tab class="tab-text-truncate"
                                       v-for="level in historicalLevelButtons"
                                       :key="level.value"
                                       :value="level.value">
                                    <span class="text-truncate">{{ level.name }}</span>
                                </v-tab>
                            </v-tabs>
                        </template>
                        <template v-else>
                            <v-divider class="mt-4" />
                            <!-- 时间筛选列表（类似交易列表的月份选择）-->
                            <v-tabs show-arrows class="my-4" direction="vertical"
                                    :disabled="loading" v-model="activePeriodFilterIndex">
                                <v-tab class="tab-text-truncate" :key="idx" :value="idx"
                                       v-for="(filter, idx) in visiblePeriodFilters"
                                       @click="setPeriodFilter(filter.value)">
                                    <span class="text-truncate">{{ filter.name }}</span>
                                </v-tab>
                            </v-tabs>
                        </template>
                    </v-navigation-drawer>

                    <!-- 主内容区 -->
                    <v-main>
                        <v-card variant="flat" min-height="920">
                            <template #title>
                                <div class="title-and-toolbar d-flex align-center text-no-wrap">
                                    <v-btn class="me-3 d-md-none" density="compact" color="default" variant="plain"
                                           :ripple="false" :icon="true" @click="showNav = !showNav">
                                        <v-icon :icon="mdiMenu" size="24" />
                                    </v-btn>
                                    <span>{{ currentViewTitle }}</span>
                                    <!-- 操作按钮 -->
                                    <v-btn class="ms-3" color="default" variant="outlined"
                                           :disabled="loading || updating" @click="add" v-if="activeViewMode === 'budget'">
                                        {{ tt('Add') }}
                                    </v-btn>
                                    <v-btn class="ms-3" color="default" variant="outlined"
                                           :disabled="loading || updating" @click="importBudgets" v-if="activeViewMode === 'budget'">
                                        {{ tt('Import') }}
                                    </v-btn>
                                    <v-btn class="ms-3" color="default" variant="outlined"
                                           :disabled="loading || updating" @click="exportBudgets" v-if="activeViewMode === 'budget'">
                                        {{ tt('Export') }}
                                    </v-btn>
                                    <input type="file" ref="fileInput" style="display: none" accept=".json" @change="onFileSelected" />

                                    <!-- 全部展开/折叠切换按钮（单一按钮带动画） -->
                                    <v-btn class="ms-2" density="compact" color="default" variant="text" size="24"
                                           :icon="true" :disabled="loading || updating || groupedBudgets.length === 0"
                                           @click="toggleAllCategories" v-if="activeViewMode === 'budget'">
                                        <v-icon
                                            :icon="isAllExpanded ? mdiUnfoldLessHorizontal : mdiUnfoldMoreHorizontal"
                                            size="20"
                                            class="toggle-icon"
                                            :class="{ 'rotated': !isAllExpanded }"
                                        />
                                        <v-tooltip activator="parent">{{ isAllExpanded ? tt('Collapse All') : tt('Expand All') }}</v-tooltip>
                                    </v-btn>

                                    <!-- 刷新按钮 -->
                                    <v-btn v-if="activeViewMode === 'budget'"
                                           density="compact" color="default" variant="text" size="24"
                                           class="ms-2" :icon="true" :loading="loading || updating" @click="reload(true)">
                                        <template #loader>
                                            <v-progress-circular indeterminate size="20"/>
                                        </template>
                                        <v-icon :icon="mdiRefresh" size="24" />
                                        <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                                    </v-btn>

                                    <v-btn-group v-if="activeViewMode === 'history'" class="ms-4" color="default" density="comfortable" variant="outlined" divided>
                                        <v-btn class="button-icon-with-direction" :icon="mdiArrowLeft"
                                               :disabled="loading || !canShiftHistoricalDateRange"
                                               @click="shiftHistoricalDateRange(-1)"/>
                                        <v-menu location="bottom">
                                            <template #activator="{ props }">
                                                <v-btn :disabled="loading" v-bind="props">{{ historicalDateRangeName }}</v-btn>
                                            </template>
                                            <v-list :selected="[historicalDateType]">
                                                <v-list-item :key="dateRange.type" :value="dateRange.type"
                                                             :append-icon="(historicalDateType === dateRange.type ? mdiCheck : undefined)"
                                                             v-for="dateRange in allHistoricalDateRanges">
                                                    <v-list-item-title class="cursor-pointer"
                                                                       @click="setHistoricalDateFilter(dateRange.type)">
                                                        <div class="d-flex align-center">
                                                            <span>{{ dateRange.displayName }}</span>
                                                        </div>
                                                        <div class="statistics-custom-datetime-range smaller"
                                                             v-if="dateRange.isUserCustomRange && canShowHistoricalCustomDateRange(dateRange.type)">
                                                            <span>{{ historicalDateStartText }}</span>
                                                            <span>&nbsp;-&nbsp;</span>
                                                            <br/>
                                                            <span>{{ historicalDateEndText }}</span>
                                                        </div>
                                                    </v-list-item-title>
                                                </v-list-item>
                                            </v-list>
                                        </v-menu>
                                        <v-btn class="button-icon-with-direction" :icon="mdiArrowRight"
                                               :disabled="loading || !canShiftHistoricalDateRange"
                                               @click="shiftHistoricalDateRange(1)"/>
                                    </v-btn-group>

                                    <v-menu location="bottom" v-if="activeViewMode === 'history'">
                                        <template #activator="{ props }">
                                            <v-btn class="ms-3" color="default" variant="outlined"
                                                   :prepend-icon="mdiCalendarRangeOutline" :disabled="loading"
                                                   v-bind="props">{{ historicalAggregationLabel }}</v-btn>
                                        </template>
                                        <v-list>
                                            <v-list-item class="cursor-pointer"
                                                         :key="aggregation.value"
                                                         :value="aggregation.value"
                                                         :append-icon="(historicalAggregationType === aggregation.value ? mdiCheck : undefined)"
                                                         :title="aggregation.name"
                                                         v-for="aggregation in historicalAggregationOptions"
                                                         @click="historicalAggregationType = aggregation.value">
                                            </v-list-item>
                                        </v-list>
                                    </v-menu>

                                    <v-btn v-if="activeViewMode === 'history'"
                                           density="compact" color="default" variant="text" size="32"
                                           class="ms-2" :icon="true" :loading="loading || updating" @click="reload(true)">
                                        <template #loader>
                                            <v-progress-circular indeterminate size="20"/>
                                        </template>
                                        <v-icon :icon="mdiRefresh" size="24" />
                                        <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                                    </v-btn>

                                    <div v-if="activeViewMode === 'forecast'" class="budget-forecast-title-actions d-flex align-center ga-2 ms-4">
                                        <v-btn class="budget-forecast-title-button"
                                               density="compact"
                                               :color="forecastOnlyLowConfidence ? 'warning' : 'default'"
                                               :variant="forecastOnlyLowConfidence ? 'flat' : 'outlined'"
                                               :disabled="loading || forecastLoading"
                                               @click="forecastOnlyLowConfidence = !forecastOnlyLowConfidence">
                                            {{ tt('Low Confidence Only') }}
                                        </v-btn>

                                        <v-btn class="budget-forecast-title-button"
                                               density="compact"
                                               :color="forecastOnlyOverBudget ? 'error' : 'default'"
                                               :variant="forecastOnlyOverBudget ? 'flat' : 'outlined'"
                                               :disabled="loading || forecastLoading"
                                               @click="forecastOnlyOverBudget = !forecastOnlyOverBudget">
                                            {{ tt('Over Budget Only') }}
                                        </v-btn>

                                        <v-btn class="budget-forecast-title-button"
                                               color="default"
                                               variant="outlined"
                                               density="compact"
                                               :disabled="loading || forecastLoading"
                                               @click="showForecastSettingsDialog = true">
                                            {{ tt('Forecast Settings') }}
                                        </v-btn>

                                        <v-btn density="compact"
                                               color="default"
                                               variant="text"
                                               size="24"
                                               :icon="true"
                                               :loading="loading || forecastLoading"
                                               @click="reload(true)">
                                            <template #loader>
                                                <v-progress-circular indeterminate size="20"/>
                                            </template>
                                            <v-icon :icon="mdiRefresh" size="24" />
                                            <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                                        </v-btn>
                                    </div>

                                    <v-spacer />

                                    <div class="budget-right-tools d-flex align-center">
                                        <!-- 搜索框 -->
                                        <div class="budget-keyword-filter" v-if="activeViewMode !== 'history'">
                                            <v-text-field density="compact" :disabled="loading"
                                                          :prepend-inner-icon="mdiMagnify"
                                                          :append-inner-icon="filterKeyword !== searchKeyword ? mdiCheck : undefined"
                                                          :placeholder="tt('Filter budget description')"
                                                          hide-details
                                                          v-model="filterKeyword"
                                                          @click:append-inner="setKeywordFilter(filterKeyword)"
                                                          @keyup.enter="setKeywordFilter(filterKeyword)"
                                            />
                                        </div>

                                        <!-- 更多选项菜单（包含所有筛选功能） -->
                                        <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                                               :disabled="loading" :icon="true">
                                            <v-icon :icon="mdiDotsVertical" />
                                            <v-menu activator="parent" :close-on-content-click="false" width="320">
                                                <v-list density="compact" class="budget-filter-menu">
                                                <!-- 分类筛选 -->
                                                <v-list-item :disabled="loading"
                                                             :prepend-icon="mdiShapeOutline"
                                                             @click="showFilterCategoryDialog = true">
                                                    <v-list-item-title class="d-flex align-center justify-space-between">
                                                        <span>{{ tt('Filter Categories') }}</span>
                                                        <v-chip v-if="categoryFilter" size="x-small" color="primary" class="ms-2">
                                                            {{ getCategoryFilterDisplayName() }}
                                                        </v-chip>
                                                    </v-list-item-title>
                                                </v-list-item>

                                                <!-- 账户筛选 -->
                                                <v-list-item :disabled="loading"
                                                             :prepend-icon="mdiWalletOutline"
                                                             @click="showFilterAccountDialog = true">
                                                    <v-list-item-title class="d-flex align-center justify-space-between">
                                                        <span>{{ tt('Filter Accounts') }}</span>
                                                        <v-chip v-if="accountFilter.length > 0" size="x-small" color="primary" class="ms-2">
                                                            {{ accountFilter.length }}
                                                        </v-chip>
                                                    </v-list-item-title>
                                                </v-list-item>

                                                <!-- 标签筛选 -->
                                                <v-list-item :disabled="loading"
                                                             :prepend-icon="mdiTagOutline"
                                                             @click="showFilterTagDialog = true">
                                                    <v-list-item-title class="d-flex align-center justify-space-between">
                                                        <span>{{ tt('Filter Tags') }}</span>
                                                        <v-chip v-if="tagFilter.length > 0" size="x-small" color="primary" class="ms-2">
                                                            {{ tagFilter.length }}
                                                        </v-chip>
                                                    </v-list-item-title>
                                                </v-list-item>

                                                <v-divider class="my-2" />

                                                <!-- 执行率筛选 -->
                                                <v-list-group>
                                                    <template #activator="{ props }">
                                                        <v-list-item v-bind="props" :disabled="loading" :prepend-icon="mdiChartDonut">
                                                            <v-list-item-title class="d-flex align-center justify-space-between">
                                                                <span>{{ tt('Execution Rate') }}</span>
                                                                <v-chip v-if="executionRateFilter" size="x-small" color="primary" class="ms-2">
                                                                    {{ executionRateFilter.label }}
                                                                </v-chip>
                                                            </v-list-item-title>
                                                        </v-list-item>
                                                    </template>
                                                    <v-list-item key="" class="text-sm"
                                                                 :class="{ 'list-item-selected': !executionRateFilter }"
                                                                 @click="setExecutionRateFilter(null)">
                                                        <v-list-item-title>{{ tt('All') }}</v-list-item-title>
                                                    </v-list-item>
                                                    <v-list-item v-for="option in executionRateOptions" :key="option.value"
                                                                 class="text-sm"
                                                                 :class="{ 'list-item-selected': executionRateFilter?.value === option.value }"
                                                                 @click="setExecutionRateFilter(option)">
                                                        <v-list-item-title>{{ option.label }}</v-list-item-title>
                                                    </v-list-item>
                                                </v-list-group>

                                                <!-- 已花费筛选 -->
                                                <v-list-group>
                                                    <template #activator="{ props }">
                                                        <v-list-item v-bind="props" :disabled="loading" :prepend-icon="mdiCashMinus">
                                                            <v-list-item-title class="d-flex align-center justify-space-between">
                                                                <span>{{ tt('Spent Amount') }}</span>
                                                                <v-chip v-if="spentAmountFilter" size="x-small" color="primary" class="ms-2">
                                                                    {{ getSpentFilterLabel() }}
                                                                </v-chip>
                                                            </v-list-item-title>
                                                        </v-list-item>
                                                    </template>
                                                    <v-list-item class="text-sm"
                                                                 :class="{ 'list-item-selected': !spentAmountFilter }"
                                                                 @click="changeSpentFilter('')">
                                                        <v-list-item-title>{{ tt('All') }}</v-list-item-title>
                                                    </v-list-item>
                                                    <template v-for="filterType in AmountFilterType.values()" :key="filterType.type">
                                                        <v-list-item class="text-sm"
                                                                     :class="{ 'list-item-selected': currentSpentFilterType === filterType.type }"
                                                                     @click="onSpentFilterTypeClick(filterType.type)">
                                                            <v-list-item-title class="d-flex align-center flex-wrap ga-2">
                                                                <span>{{ tt(filterType.name) }}</span>
                                                                <template v-if="currentSpentFilterType === filterType.type">
                                                                    <amount-input class="budget-amount-filter-value" density="compact"
                                                                                  :currency="defaultCurrency"
                                                                                  v-model="currentSpentFilterValue1" />
                                                                    <template v-if="filterType.paramCount === 2">
                                                                        <span>~</span>
                                                                        <amount-input class="budget-amount-filter-value" density="compact"
                                                                                      :currency="defaultCurrency"
                                                                                      v-model="currentSpentFilterValue2" />
                                                                    </template>
                                                                    <v-btn size="x-small" color="primary" variant="tonal"
                                                                           @click.stop="changeSpentFilter(filterType.type)">
                                                                        {{ tt('Apply') }}
                                                                    </v-btn>
                                                                </template>
                                                            </v-list-item-title>
                                                        </v-list-item>
                                                    </template>
                                                </v-list-group>

                                                <!-- 总预算筛选 -->
                                                <v-list-group>
                                                    <template #activator="{ props }">
                                                        <v-list-item v-bind="props" :disabled="loading" :prepend-icon="mdiCashPlus">
                                                            <v-list-item-title class="d-flex align-center justify-space-between">
                                                                <span>{{ tt('Budget Amount') }}</span>
                                                                <v-chip v-if="budgetAmountFilter" size="x-small" color="primary" class="ms-2">
                                                                    {{ getBudgetFilterLabel() }}
                                                                </v-chip>
                                                            </v-list-item-title>
                                                        </v-list-item>
                                                    </template>
                                                    <v-list-item class="text-sm"
                                                                 :class="{ 'list-item-selected': !budgetAmountFilter }"
                                                                 @click="changeBudgetFilter('')">
                                                        <v-list-item-title>{{ tt('All') }}</v-list-item-title>
                                                    </v-list-item>
                                                    <template v-for="filterType in AmountFilterType.values()" :key="filterType.type">
                                                        <v-list-item class="text-sm"
                                                                     :class="{ 'list-item-selected': currentBudgetFilterType === filterType.type }"
                                                                     @click="onBudgetFilterTypeClick(filterType.type)">
                                                            <v-list-item-title class="d-flex align-center flex-wrap ga-2">
                                                                <span>{{ tt(filterType.name) }}</span>
                                                                <template v-if="currentBudgetFilterType === filterType.type">
                                                                    <amount-input class="budget-amount-filter-value" density="compact"
                                                                                  :currency="defaultCurrency"
                                                                                  v-model="currentBudgetFilterValue1" />
                                                                    <template v-if="filterType.paramCount === 2">
                                                                        <span>~</span>
                                                                        <amount-input class="budget-amount-filter-value" density="compact"
                                                                                      :currency="defaultCurrency"
                                                                                      v-model="currentBudgetFilterValue2" />
                                                                    </template>
                                                                    <v-btn size="x-small" color="primary" variant="tonal"
                                                                           @click.stop="changeBudgetFilter(filterType.type)">
                                                                        {{ tt('Apply') }}
                                                                    </v-btn>
                                                                </template>
                                                            </v-list-item-title>
                                                        </v-list-item>
                                                    </template>
                                                </v-list-group>

                                                <v-divider class="my-2" />

                                                <!-- 清除所有筛选 -->
                                                <v-list-item :prepend-icon="mdiFilterRemoveOutline"
                                                             :title="tt('Clear All Filters')"
                                                             :disabled="!hasActiveFilters"
                                                             @click="clearAllFilters"></v-list-item>

                                                <v-divider class="my-2" />

                                                <!-- 筛选预设管理 -->
                                                <v-list-group>
                                                    <template #activator="{ props }">
                                                        <v-list-item v-bind="props" :prepend-icon="mdiBookmarkOutline">
                                                            <v-list-item-title>{{ tt('Filter Presets') }}</v-list-item-title>
                                                        </v-list-item>
                                                    </template>

                                                    <!-- 保存当前筛选 -->
                                                    <v-list-item @click="showSavePresetDialog = true" :disabled="!hasActiveFilters">
                                                        <template #prepend>
                                                            <v-icon :icon="mdiContentSaveOutline" size="20" class="ms-4" />
                                                        </template>
                                                        <v-list-item-title>{{ tt('Save Current Filters') }}</v-list-item-title>
                                                    </v-list-item>

                                                    <v-divider v-if="filterPresets.length > 0" class="my-1" />

                                                    <!-- 预设列表 -->
                                                    <v-list-item v-for="preset in filterPresets" :key="preset.id"
                                                                 @click="loadPreset(preset)">
                                                        <template #prepend>
                                                            <v-icon :icon="mdiBookmark" size="20" class="ms-4" />
                                                        </template>
                                                        <v-list-item-title>{{ preset.name }}</v-list-item-title>
                                                        <template #append>
                                                            <v-btn icon size="x-small" variant="text"
                                                                   @click.stop="deletePreset(preset.id)">
                                                                <v-icon :icon="mdiClose" size="16" />
                                                            </v-btn>
                                                        </template>
                                                    </v-list-item>

                                                    <v-list-item v-if="filterPresets.length === 0" class="text-medium-emphasis">
                                                        <template #prepend>
                                                            <v-icon :icon="mdiInformationOutline" size="20" class="ms-4" />
                                                        </template>
                                                        <v-list-item-title class="text-body-2">{{ tt('No saved presets') }}</v-list-item-title>
                                                    </v-list-item>
                                                </v-list-group>
                                                </v-list>
                                            </v-menu>
                                        </v-btn>
                                    </div>
                                </div>
                            </template>

                            <!-- 汇总信息行（在工具栏下方，参考统计分析页面的样式） -->
                            <v-card-text v-if="currentExecution && activeViewMode === 'budget'" class="py-3 border-b budget-summary-row">
                                <div class="d-flex align-center flex-wrap ga-6">
                                    <div class="d-flex align-center">
                                        <span class="budget-summary-label me-2">{{ tt('Total Budget') }}:</span>
                                        <span class="budget-summary-amount text-expense">{{ formatAmount(filteredSummary.totalBudget / 100) }}</span>
                                    </div>
                                    <div class="d-flex align-center">
                                        <span class="budget-summary-label me-2">{{ tt('Total Spent') }}:</span>
                                        <span class="budget-summary-amount text-income">{{ formatAmount(filteredSummary.totalSpent / 100) }}</span>
                                    </div>
                                    <div class="d-flex align-center">
                                        <span class="budget-summary-label me-2">{{ tt('Overall Execution Rate') }}:</span>
                                        <span class="budget-summary-amount" :class="getExecutionRateColorClass(filteredSummary.totalExecutionRate)">
                                            {{ filteredSummary.totalExecutionRate.toFixed(1) }}%
                                        </span>
                                    </div>
                                </div>
                            </v-card-text>

                            <v-window class="d-flex flex-grow-1 disable-tab-transition w-100-window-container" v-model="activeViewMode">
                                <!-- 预算管理视图 -->
                                <v-window-item value="budget">
                                    <!-- 活动筛选器标签 -->
                                    <v-card-text v-if="hasActiveFilters" class="py-2 border-b">
                                        <div class="d-flex align-center flex-wrap ga-2">
                                            <span class="text-body-2 text-medium-emphasis">{{ tt('Active Filters') }}:</span>
                                            <v-chip v-if="categoryFilter" closable size="small" @click:close="categoryFilter = null">
                                                {{ tt('Category') }}: {{ getCategoryFilterDisplayName() }}
                                            </v-chip>
                                            <v-chip v-if="accountFilter.length > 0" closable size="small" @click:close="accountFilter = []">
                                                {{ tt('Account') }}: {{ getAccountFilterDisplayName() }}
                                            </v-chip>
                                            <v-chip v-if="tagFilter.length > 0" closable size="small" @click:close="tagFilter = []">
                                                {{ tt('Tag') }}: {{ getTagFilterDisplayName() }}
                                            </v-chip>
                                            <v-chip v-if="executionRateFilter" closable size="small" @click:close="executionRateFilter = null">
                                                {{ tt('Execution Rate') }}: {{ executionRateFilter.label }}
                                            </v-chip>
                                            <v-chip v-if="spentAmountFilter" closable size="small" @click:close="spentAmountFilter = ''">
                                                {{ getSpentFilterDisplayName() }}
                                            </v-chip>
                                            <v-chip v-if="budgetAmountFilter" closable size="small" @click:close="budgetAmountFilter = ''">
                                                {{ getBudgetFilterDisplayName() }}
                                </v-chip>
                            </div>
                        </v-card-text>

                        <!-- 预算表格（无表头） -->
                        <v-table class="budget-table" :hover="!loading">
                            <tbody v-if="loading && filteredBudgets.length === 0">
                            <tr :key="itemIdx" v-for="itemIdx in [1, 2, 3, 4, 5]">
                                <td class="px-0" colspan="4">
                                    <v-skeleton-loader type="text" :loading="true"></v-skeleton-loader>
                                </td>
                            </tr>
                            </tbody>

                            <tbody v-if="!loading && filteredBudgets.length === 0">
                            <tr>
                                <td colspan="4">
                                    <div class="d-flex align-center justify-center py-8">
                                        <span class="text-grey">{{ tt('No budgets found') }}</span>
                                    </div>
                                </td>
                            </tr>
                            </tbody>

                            <tbody v-if="filteredBudgets.length > 0">
                            <!-- 层级显示：按一级分类分组 -->
                            <template v-for="(group, gIdx) in groupedBudgets" :key="group.category">
                                <!-- 一级分类行（可折叠） -->
                                <tr class="budget-group-header budget-list-row"
                                    :class="{ 'budget-group-last-row': gIdx < groupedBudgets.length - 1 && (group.isCollapsed || !groupHasExpandedRows(group)) }"
                                    @click="toggleCategoryCollapse(group.category)"
                                    tabindex="0">
                                    <td colspan="4" class="pa-0">
                                        <div class="budget-item budget-primary d-flex px-4 py-3"
                                             :class="{ 'bg-grey-lighten-4': !isDarkMode, 'bg-grey-darken-3': isDarkMode }">
                                            <!-- 折叠/展开图标 -->
                                            <v-icon
                                                :icon="group.isCollapsed ? mdiChevronRight : mdiChevronDown"
                                                size="20"
                                                class="me-2 text-grey flex-shrink-0"
                                            />
                                            <!-- 分类图标（放大，与文字+进度条等高） -->
                                            <item-icon
                                                v-if="group.categoryIcon"
                                                class="me-3 flex-shrink-0"
                                                icon-type="category"
                                                :icon-id="group.categoryIcon"
                                                :color="group.categoryColor"
                                                :size="36"
                                            />
                                            <!-- 右侧内容区域（分类名称+进度条） -->
                                            <div class="d-flex flex-column flex-grow-1">
                                                <!-- 第一行：分类名称 + 执行率 + 操作按钮 + 金额 -->
                                                <div class="d-flex align-center justify-space-between mb-1">
                                                    <div class="d-flex align-center flex-grow-1">
                                                        <span class="budget-category-name text-body-1 font-weight-bold">
                                                            {{ group.category }}
                                                        </span>
                                                        <!-- 一级分类执行率（始终显示，无论是否有一级分类预算） -->
                                                        <span class="budget-percent text-body-2 ms-2"
                                                              :class="getExecutionRateTextClass(getGroupExecutionRate(group))">
                                                            {{ getGroupExecutionRateText(group) }}
                                                        </span>
                                                    </div>
                                                        <!-- 一级分类预算操作按钮（悬停时显示） -->
                                                    <div class="budget-row-actions d-flex align-center">
                                                        <!-- 如果没有一级分类预算，显示添加按钮 -->
                                                        <v-btn v-if="group.primaryBudgets.length === 0"
                                                               density="compact" color="default" variant="text" size="x-small"
                                                               :icon="mdiPlusCircleOutline"
                                                               :disabled="loading || updating"
                                                               @click.stop="addPrimaryBudget(group)">
                                                            <v-icon :icon="mdiPlusCircleOutline" size="16" />
                                                            <v-tooltip activator="parent">{{ tt('Add Primary Budget') }}</v-tooltip>
                                                        </v-btn>
                                                        <!-- 如果有一级分类预算，显示编辑和删除按钮 -->
                                                        <v-btn v-if="group.primaryBudgets.length === 1"
                                                               density="compact" color="default" variant="text" size="x-small"
                                                               :icon="mdiPencilOutline"
                                                               :disabled="loading || updating"
                                                               @click.stop="edit(getPrimaryBudgetForHeader(group)!)">
                                                            <v-icon :icon="mdiPencilOutline" size="16" />
                                                            <v-tooltip activator="parent">{{ tt('Edit') }}</v-tooltip>
                                                        </v-btn>
                                                        <v-btn v-if="group.primaryBudgets.length === 1"
                                                               density="compact" color="default" variant="text" size="x-small"
                                                               :icon="mdiDeleteOutline"
                                                               :loading="budgetRemoving[getPrimaryBudgetForHeader(group)!.id]"
                                                               :disabled="loading || updating"
                                                               @click.stop="remove(getPrimaryBudgetForHeader(group)!)">
                                                            <template #loader>
                                                                <v-progress-circular indeterminate size="14" width="2"/>
                                                            </template>
                                                            <v-icon :icon="mdiDeleteOutline" size="16" />
                                                            <v-tooltip activator="parent">{{ tt('Delete') }}</v-tooltip>
                                                        </v-btn>
                                                    </div>
                                                    <!-- 汇总金额显示（左对齐在最右边） -->
                                                    <div class="budget-amounts d-flex align-center justify-end ms-auto" style="min-width: 150px;">
                                                        <span class="budget-spent text-body-2">
                                                            {{ formatAmount(group.totalSpent / 100) }}
                                                        </span>
                                                        <span class="budget-separator text-body-2 text-medium-emphasis mx-1">/</span>
                                                        <span class="budget-total text-body-2 text-medium-emphasis">
                                                            {{ formatAmount(group.totalAmount / 100) }}
                                                        </span>
                                                    </div>
                                                </div>
                                                <!-- 一级分类进度条（始终显示，无论是否有一级分类预算，点击跳转到账单列表） -->
                                                <div class="budget-progress-container cursor-pointer"
                                                       @click.stop="navigateToTransactions(group.category, null, getPrimaryBudgetForHeader(group))"
                                                     :title="tt('Click to view transactions')">
                                                    <v-progress-linear
                                                        :model-value="Math.min(getGroupExecutionRate(group), 100)"
                                                        :color="getGroupProgressColor(group)"
                                                        :bg-color="isDarkMode ? '#444444' : '#f0f0f0'"
                                                        :bg-opacity="1"
                                                        :height="6"
                                                        rounded
                                                    />
                                                </div>
                                            </div>
                                        </div>
                                    </td>
                                </tr>

                                <!-- 二级分类预算列表（展开时显示） -->
                                <template v-if="!group.isCollapsed">
                                    <tr v-for="(budget, pIdx) in getExpandedPrimaryBudgets(group)" :key="budget.id"
                                        class="budget-list-row budget-sub-row budget-primary-row"
                                        :class="{ 'budget-group-last-row': gIdx < groupedBudgets.length - 1 && pIdx === getExpandedPrimaryBudgets(group).length - 1 && group.subBudgets.length === 0 }"
                                        @dblclick="edit(budget)" tabindex="0"
                                        @keydown.delete="remove(budget)" @keydown.enter="edit(budget)">
                                        <td colspan="4" class="pa-0">
                                            <div class="budget-item budget-secondary d-flex px-4 py-2"
                                                 style="padding-left: 56px !important;">
                                                <item-icon
                                                    v-if="budget.categoryIcon"
                                                    class="me-3 flex-shrink-0"
                                                    icon-type="category"
                                                    :icon-id="budget.categoryIcon"
                                                    :color="budget.categoryColor"
                                                    :size="28"
                                                />
                                                <div class="d-flex flex-column flex-grow-1">
                                                    <div class="d-flex align-center justify-space-between mb-1">
                                                        <div class="d-flex align-center flex-grow-1">
                                                            <span class="budget-category-name text-body-2 font-weight-medium">
                                                                {{ budget.name || group.category }}
                                                            </span>
                                                            <span class="budget-percent text-body-2 ms-2"
                                                                  :class="getExecutionRateTextClass(budget.executionRate)">
                                                                {{ budget.executionRateText }}
                                                            </span>
                                                            <v-icon v-if="budget.alertTriggered && !budget.isOverBudget"
                                                                    :icon="mdiAlertCircle" color="warning" class="ms-1" size="14" />
                                                            <v-icon v-if="budget.isOverBudget"
                                                                    :icon="mdiAlertOctagon" color="error" class="ms-1" size="14" />
                                                        </div>
                                                        <div class="budget-row-actions d-flex align-center">
                                                            <v-btn density="compact" color="default" variant="text" size="x-small"
                                                                   :icon="mdiPencilOutline"
                                                                   :disabled="loading || updating"
                                                                   @click.stop="edit(budget)">
                                                                <v-icon :icon="mdiPencilOutline" size="16" />
                                                                <v-tooltip activator="parent">{{ tt('Edit') }}</v-tooltip>
                                                            </v-btn>
                                                            <v-btn density="compact" color="default" variant="text" size="x-small"
                                                                   :icon="mdiDeleteOutline"
                                                                   :loading="budgetRemoving[budget.id]"
                                                                   :disabled="loading || updating"
                                                                   @click.stop="remove(budget)">
                                                                <template #loader>
                                                                    <v-progress-circular indeterminate size="14" width="2"/>
                                                                </template>
                                                                <v-icon :icon="mdiDeleteOutline" size="16" />
                                                                <v-tooltip activator="parent">{{ tt('Delete') }}</v-tooltip>
                                                            </v-btn>
                                                        </div>
                                                        <div class="budget-amounts d-flex align-center justify-end ms-auto" style="min-width: 150px;">
                                                            <span class="budget-spent text-body-2"
                                                                  :class="{ 'text-error font-weight-bold': budget.isOverBudget }">
                                                                {{ formatAmount(budget.spentAmountInYuan) }}
                                                            </span>
                                                            <span class="budget-separator text-body-2 text-medium-emphasis mx-1">/</span>
                                                            <span class="budget-total text-body-2 text-medium-emphasis">
                                                                {{ formatAmount(budget.amountInYuan) }}
                                                            </span>
                                                        </div>
                                                    </div>
                                                    <div class="budget-progress-container cursor-pointer"
                                                         @click.stop="navigateToTransactions(budget.category, null, budget)"
                                                         :title="tt('Click to view transactions')">
                                                        <v-progress-linear
                                                            :model-value="Math.min(budget.executionRate, 100)"
                                                            :color="getBudgetProgressColor(budget)"
                                                            :bg-color="isDarkMode ? '#444444' : '#f0f0f0'"
                                                            :bg-opacity="1"
                                                            :height="6"
                                                            rounded
                                                        />
                                                    </div>
                                                </div>
                                            </div>
                                        </td>
                                    </tr>
                                </template>
                            </template>
                            </tbody>
                        </v-table>
                    </v-window-item>

                    <v-window-item value="history">
                        <v-card-text class="pt-4">
                            <div v-if="loading && (!isHistoricalHistoryReady || historicalLegendGroups.length === 0)" class="py-10 d-flex flex-column align-center justify-center budget-history-loading-placeholder">
                                <v-progress-linear indeterminate color="primary" class="budget-history-loading-bar mb-2" />
                                <span class="text-caption text-medium-emphasis">{{ tt('Loading') }}...</span>
                            </div>

                            <div v-else-if="historicalLegendGroups.length > 0" class="budget-history-panel">
                                <div class="budget-history-chart-shell">
                                    <v-chart
                                        v-if="historicalChartModel.primaryBands.length > 0"
                                        :key="historicalChartRenderKey"
                                        ref="historicalChartRef"
                                        autoresize
                                        class="budget-history-chart"
                                        :option="historicalChartOptions"
                                        :update-options="historicalChartUpdateOptions"
                                    />

                                    <div v-else class="d-flex align-center justify-center budget-history-chart budget-history-empty-state">
                                        <span class="text-medium-emphasis">{{ tt('All categories hidden') }}</span>
                                    </div>

                                </div>

                                <div v-if="historicalLegendGroups.length > 0" class="budget-history-legend">
                                    <div v-for="group in historicalLegendGroups" :key="group.primaryKey" class="budget-history-legend-group">
                                        <button
                                            type="button"
                                            class="budget-history-legend-item budget-history-legend-item--primary"
                                            :class="{
                                                'is-inactive': group.state === 'none',
                                                'is-partial': group.state === 'partial'
                                            }"
                                            :aria-pressed="group.state !== 'none'"
                                            @click="toggleHistoricalPrimaryLegend(group.primaryKey)"
                                        >
                                            <span class="budget-history-legend-swatch" :style="{ backgroundColor: group.color }"></span>
                                            <span class="budget-history-legend-label">{{ group.primaryLabel }}</span>
                                            <span v-if="historicalBudgetLevel === 'secondary'" class="budget-history-legend-count">
                                                {{ group.secondaryItems.filter(item => item.selected).length }}/{{ group.secondaryItems.length }}
                                            </span>
                                        </button>

                                        <div v-if="historicalBudgetLevel === 'secondary'" class="budget-history-legend-secondary-list">
                                            <button
                                                v-for="item in group.secondaryItems"
                                                :key="item.key"
                                                type="button"
                                                class="budget-history-legend-item budget-history-legend-item--secondary"
                                                :class="{ 'is-inactive': !item.selected }"
                                                :aria-pressed="item.selected"
                                                @click="toggleHistoricalSecondaryLegend(item.key)"
                                            >
                                                <span class="budget-history-legend-swatch" :style="{ backgroundColor: item.color }"></span>
                                                <span class="budget-history-legend-label">{{ item.label }}</span>
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            <div v-else class="d-flex flex-column align-center justify-center py-16">
                                <v-icon :icon="mdiChartBoxOutline" size="48" color="grey-lighten-1" class="mb-3"/>
                                <span class="text-body-1 text-medium-emphasis">{{ tt('No historical budget data') }}</span>
                                <span class="text-caption text-disabled mt-1">{{ tt('Create budgets and wait for execution data') }}</span>
                            </div>
                        </v-card-text>
                    </v-window-item>

                    <!-- 周期预计视图 -->
                    <v-window-item value="forecast">
                        <!-- 周期预计表格 -->
                        <v-table class="forecast-table table-striped" :hover="!forecastLoading">
                            <thead>
                            <tr>
                                <th style="width: 20%;">{{ tt('Category') }}</th>
                                <th style="width: 15%;">{{ tt('Historical Average') }}</th>
                                <th style="width: 15%;">{{ tt('Current Spent') }}</th>
                                <th style="width: 15%;">{{ tt('Projected Total') }}</th>
                                <th style="width: 15%;">{{ tt('Budget') }}</th>
                                <th style="width: 10%;">{{ tt('Trend') }}</th>
                                <th style="width: 10%;">{{ tt('Status') }}</th>
                            </tr>
                            </thead>

                            <tbody v-if="forecastLoading && (!currentForecast || currentForecast.forecasts.length === 0)">
                            <tr :key="itemIdx" v-for="itemIdx in [1, 2, 3, 4, 5]">
                                <td class="px-0" colspan="7">
                                    <v-skeleton-loader type="text" :loading="true"></v-skeleton-loader>
                                </td>
                            </tr>
                            </tbody>

                            <tbody v-if="!forecastLoading && (!currentForecast || currentForecast.forecasts.length === 0)">
                            <tr>
                                <td colspan="7">
                                    <div class="d-flex flex-column align-center justify-center py-12">
                                        <v-icon :icon="mdiChartTimelineVariant" size="48" color="grey-lighten-1" class="mb-3"/>
                                        <span class="text-body-1 text-medium-emphasis">{{ tt('No forecast data') }}</span>
                                    </div>
                                </td>
                            </tr>
                            </tbody>

                            <tbody v-if="displayForecasts.length > 0">
                            <tr v-for="forecast in displayForecasts" :key="forecast.categoryId" class="text-sm">
                                <td>
                                    <div>{{ forecast.categoryName }}</div>
                                    <div class="text-caption text-medium-emphasis" v-if="forecast.strategyExplanation">
                                        {{ forecast.strategyExplanation }}
                                    </div>
                                    <div class="text-caption text-medium-emphasis" v-if="forecast.samplePeriods !== null && forecast.samplePeriods !== undefined">
                                        {{ tt('Sample Periods') }}: {{ forecast.samplePeriods }}
                                    </div>
                                </td>
                                <td>{{ formatAmount(forecast.historicalAverage / 100) }}</td>
                                <td>{{ formatAmount(forecast.currentSpent / 100) }}</td>
                                <td :class="{ 'text-error': forecast.projectedOverBudget }">
                                    {{ formatAmount(forecast.projectedTotal / 100) }}
                                </td>
                                <td>{{ formatAmount(forecast.budgetAmount / 100) }}</td>
                                <td>
                                    <v-icon v-if="forecast.trend === 'up'" :icon="mdiTrendingUp" color="error" size="20" />
                                    <v-icon v-else-if="forecast.trend === 'down'" :icon="mdiTrendingDown" color="success" size="20" />
                                    <v-icon v-else :icon="mdiTrendingNeutral" color="grey" size="20" />
                                </td>
                                <td>
                                    <div class="d-flex flex-column ga-1">
                                        <v-chip v-if="forecast.projectedOverBudget" color="error" size="small">
                                            {{ tt('Over Budget') }}
                                        </v-chip>
                                        <v-chip v-else color="success" size="small">
                                            {{ tt('On Track') }}
                                        </v-chip>
                                        <span class="text-caption text-medium-emphasis" v-if="forecast.backtestMape !== null && forecast.backtestMape !== undefined">
                                            {{ tt('Backtest MAPE') }}: {{ forecast.backtestMape.toFixed(2) }}%
                                            <v-icon class="ms-1" :icon="mdiInformationOutline" size="14" />
                                            <v-tooltip activator="parent" location="top">
                                                {{ tt('Backtest MAPE Hint') }}
                                            </v-tooltip>
                                        </span>
                                        <span class="text-caption" :class="getForecastConfidenceClass(forecast.confidence)" v-if="forecast.confidence">
                                            {{ tt('Confidence') }}: {{ tt(getForecastConfidenceLabel(forecast.confidence)) }}
                                            <v-icon class="ms-1" :icon="mdiInformationOutline" size="14" />
                                            <v-tooltip activator="parent" location="top">
                                                {{ tt('Forecast Confidence Hint') }}
                                            </v-tooltip>
                                        </span>
                                    </div>
                                </td>
                            </tr>
                            </tbody>
                        </v-table>

                        <!-- 周期信息 -->
                        <v-card-text v-if="currentForecast" class="border-t">
                            <div class="budget-forecast-summary-row d-flex align-center flex-wrap ga-6 text-subtitle-2">
                                <div>
                                    <span>{{ tt('Period') }}: </span>
                                    <span>{{ currentForecast.periodStart }} - {{ currentForecast.periodEnd }}</span>
                                </div>
                                <div>
                                    <span>{{ tt('Forecast Strategy') }}: </span>
                                    <span>{{ tt(currentForecast.forecastStrategy === 'moving_average' ? 'Moving Average Strategy' : 'Historical Average Strategy') }}</span>
                                </div>
                                <div>
                                    <span>{{ tt('History Periods') }}: </span>
                                    <span>{{ currentForecast.historyPeriods || forecastMonthsHistory }}</span>
                                </div>
                                <div>
                                    <span>{{ tt('Low Confidence Count', { count: forecastRiskSummary.lowConfidenceCount }) }}</span>
                                </div>
                                <div>
                                    <span>{{ tt('Over Budget Count', { count: forecastRiskSummary.overBudgetCount }) }}</span>
                                </div>
                                <div v-if="currentForecast.avgBacktestMape !== null && currentForecast.avgBacktestMape !== undefined">
                                    <span>{{ tt('Average Backtest MAPE') }}: </span>
                                    <span>{{ currentForecast.avgBacktestMape.toFixed(2) }}%</span>
                                    <v-icon class="ms-1" :icon="mdiInformationOutline" size="14" />
                                    <v-tooltip activator="parent" location="top">
                                        {{ tt('Average Backtest MAPE Hint') }}
                                    </v-tooltip>
                                </div>
                            </div>
                        </v-card-text>
                                </v-window-item>
                            </v-window>
                        </v-card>
                    </v-main>
                </v-layout>
            </v-card>
        </v-col>
    </v-row>

    <!-- 编辑对话框 -->
    <edit-dialog ref="editDialog" @budget:saved="onBudgetSaved" />

    <!-- 确认对话框 -->
    <confirm-dialog ref="confirmDialog"/>
    <snack-bar ref="snackbar" />

    <!-- 筛选账户对话框 -->
    <v-dialog width="800" v-model="showFilterAccountDialog">
        <account-filter-settings-card type="statisticsCurrent" :dialog-mode="true"
            @settings:change="setAccountFilter" />
    </v-dialog>

    <!-- 筛选标签对话框 -->
    <v-dialog width="800" v-model="showFilterTagDialog">
        <transaction-tag-filter-settings-card type="statisticsCurrent" :dialog-mode="true"
            @settings:change="setTagFilter" />
    </v-dialog>

    <!-- 筛选分类对话框 -->
    <v-dialog width="800" v-model="showFilterCategoryDialog">
        <category-filter-settings-card type="statisticsCurrent" :dialog-mode="true"
            :category-types="allowedCategoryTypes"
            @settings:change="onCategoryFilterDialogChange" />
    </v-dialog>

    <!-- 自定义日期范围对话框 - 使用日期范围选择对话框组件 -->
    <date-range-selection-dialog
        :title="tt('Select Custom Date Range')"
        :min-time="customMinDatetime"
        :max-time="customMaxDatetime"
        v-model:show="showCustomDateDialog"
        @dateRange:change="onCustomDateRangeChange"
        @error="onDateRangeError"
    />

    <date-range-selection-dialog
        :title="tt('Select Custom Date Range')"
        :min-time="historicalMinDatetime"
        :max-time="historicalMaxDatetime"
        v-model:show="showHistoricalDateDialog"
        @dateRange:change="onHistoricalDateRangeChange"
        @error="onDateRangeError"
    />

    <!-- 保存筛选预设对话框 -->
    <v-dialog v-model="showSavePresetDialog" max-width="400">
        <v-card>
            <v-card-title>{{ tt('Save Filter Preset') }}</v-card-title>
            <v-card-text>
                <v-text-field
                    v-model="presetName"
                    :label="tt('Preset Name')"
                    variant="outlined"
                    autofocus
                    @keyup.enter="savePreset"
                />
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn @click="showSavePresetDialog = false">{{ tt('Cancel') }}</v-btn>
                <v-btn color="primary" @click="savePreset" :disabled="!presetName.trim()">{{ tt('Save') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <v-dialog v-model="showForecastSettingsDialog" max-width="640">
        <v-card class="pa-2 pa-sm-4 pa-md-8">
            <template #title>
                <div class="d-flex align-center justify-center">
                    <div class="d-flex w-100 align-center justify-center">
                        <h4 class="text-h4">{{ tt('Forecast Settings') }}</h4>
                    </div>
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                           :icon="true" @click="showForecastSettingsDialog = false">
                        <v-icon :icon="mdiClose" size="24" />
                    </v-btn>
                </div>
            </template>
            <v-card-text class="mt-md-4 pt-0">
                <v-row>
                    <v-col cols="12">
                        <v-select class="budget-forecast-setting-control"
                                  density="compact"
                                  hide-details
                                  variant="outlined"
                                  :disabled="loading || forecastLoading"
                                  :label="tt('Forecast Sort')"
                                  :aria-label="tt('Forecast Sort')"
                                  :items="forecastSortOptions"
                                  item-title="name"
                                  item-value="value"
                                  v-model="forecastSortBy" />
                    </v-col>

                    <v-col cols="12">
                        <v-select class="budget-forecast-setting-control"
                                  density="compact"
                                  hide-details
                                  variant="outlined"
                                  :disabled="loading || forecastLoading"
                                  :label="tt('Forecast Strategy')"
                                  :aria-label="tt('Forecast Strategy')"
                                  :items="forecastStrategies"
                                  item-title="name"
                                  item-value="value"
                                  v-model="forecastStrategy" />
                    </v-col>

                    <v-col cols="12">
                        <v-select class="budget-forecast-setting-control"
                                  density="compact"
                                  hide-details
                                  variant="outlined"
                                  :disabled="loading || forecastLoading"
                                  :label="tt('History Periods')"
                                  :aria-label="tt('History Periods')"
                                  :items="historyPeriodOptions"
                                  v-model="forecastMonthsHistory" />
                    </v-col>
                </v-row>
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn @click="showForecastSettingsDialog = false">{{ tt('Close') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
</template>

<script setup lang="ts">
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import BtnHorizontalGroup from '@/components/desktop/BtnHorizontalGroup.vue';
import BtnVerticalGroup from '@/components/desktop/BtnVerticalGroup.vue';
import ItemIcon from '@/components/desktop/ItemIcon.vue';
import AmountInput from '@/components/desktop/AmountInput.vue';
import DateRangeSelectionDialog from '@/components/desktop/DateRangeSelectionDialog.vue';
import AccountFilterSettingsCard from '@/views/desktop/common/cards/AccountFilterSettingsCard.vue';
import TransactionTagFilterSettingsCard from '@/views/desktop/common/cards/TransactionTagFilterSettingsCard.vue';
import CategoryFilterSettingsCard from '@/views/desktop/common/cards/CategoryFilterSettingsCard.vue';
import EditDialog from './list/dialogs/EditDialog.vue';
import { filterAndSortForecasts, summarizeForecastRisks } from './forecastDisplay.ts';
import { buildBudgetForecastLoadRequest } from './forecastRequest.ts';
import {
    buildHistoricalPolarChartModel,
    buildHistoricalPolarChartOption,
    createHistoricalLabelAnimationState,
    resetHistoricalCategoryAnimationState,
    resetHistoricalLabelAnimationState,
    syncHistoricalLegendSelection,
    toggleHistoricalPrimarySelection,
    toggleHistoricalSecondarySelection,
    type HistoricalCategoryChartPoint,
    type HistoricalLegendSelection
} from './historyPolarChart.ts';

import { ref, computed, useTemplateRef, watch, onMounted, nextTick } from 'vue';
import { useDisplay, useTheme } from 'vuetify';
import { useRouter } from 'vue-router';

import { useI18n } from '@/locales/helpers.ts';
import { useSettingsStore } from '@/stores/setting.ts';
import { useBudgetStore } from '@/stores/budget.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { useUserStore } from '@/stores/user.ts';
import { CategoryType } from '@/core/category.ts';
import { DateRange, DateRangeScene } from '@/core/datetime.ts';
import { FiscalYearStart } from '@/core/fiscalyear.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';
import { AmountFilterType } from '@/core/numeral.ts';
import { ThemeType } from '@/core/theme.ts';
import {
    getCurrentUnixTime,
    getTodayFirstUnixTime,
    getDateRangeByDateType,
    getShiftedDateRangeAndDateType,
    getDateTypeByDateRange
} from '@/lib/datetime.ts';
import logger from '@/lib/logger.ts';

import {
    Budget,
    BudgetType,
    BudgetPeriodType,
    BudgetForecastStrategy,
    type BudgetExecutionResponse,
    type BudgetForecastResponse,
    type BudgetHistoryResponse,
    type BudgetHistoryItem,
    type BudgetHistoryRequest
} from '@/models/budget.ts';

import {
    mdiRefresh,
    mdiPencilOutline,
    mdiDeleteOutline,
    mdiMagnify,
    mdiAlertCircle,
    mdiAlertOctagon,
    mdiTrendingUp,
    mdiTrendingDown,
    mdiTrendingNeutral,
    mdiCheck,
    mdiDotsVertical,
    mdiFilterRemoveOutline,
    mdiMenu,
    mdiChevronRight,
    mdiChevronDown,
    mdiPlusCircleOutline,
    mdiUnfoldLessHorizontal,
    mdiUnfoldMoreHorizontal,
    mdiShapeOutline,
    mdiWalletOutline,
    mdiTagOutline,
    mdiChartDonut,
    mdiCashMinus,
    mdiCashPlus,
    mdiBookmarkOutline,
    mdiBookmark,
    mdiContentSaveOutline,
    mdiClose,
    mdiInformationOutline,
    mdiArrowLeft,
    mdiArrowRight,
    mdiCalendarRangeOutline,
    mdiChartBoxOutline,
    mdiChartTimelineVariant
} from '@mdi/js';

// ============================================================================
// 类型定义
// ============================================================================

type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;
type EditDialogType = InstanceType<typeof EditDialog>;
interface ResizableChartComponent {
    resize?: () => void;
    chart?: {
        resize?: () => void;
    };
}

interface PeriodFilter {
    name: string;
    value: string;
}

interface RangeFilter {
    label: string;
    value: string;
    min?: number;
    max?: number;
}

type BudgetViewMode = 'budget' | 'forecast' | 'history';
type HistoricalBudgetLevel = 'primary' | 'secondary';

function isBudgetViewMode(value: string): value is BudgetViewMode {
    return value === 'budget' || value === 'forecast' || value === 'history';
}

const CATEGORY_CHART_PALETTE = [
    '#5470c6', '#91cc75', '#fac858', '#ee6666', '#73c0de',
    '#3ba272', '#fc8452', '#9a60b4', '#ea7ccc', '#48b8d0'
];

// ============================================================================
// 属性
// ============================================================================

const props = defineProps<{
    initType?: string;
    initPeriodType?: string;
    initViewMode?: string;
}>();

// ============================================================================
// 组件引用
// ============================================================================

const { tt, getAllDateRanges, formatDateRange } = useI18n();
const display = useDisplay();
const theme = useTheme();
const router = useRouter();
const budgetStore = useBudgetStore();
const transactionCategoriesStore = useTransactionCategoriesStore();
const accountsStore = useAccountsStore();
const transactionTagsStore = useTransactionTagsStore();
const settingsStore = useSettingsStore();
const userStore = useUserStore();

// 默认货币
const defaultCurrency = computed(() => String(settingsStore.appSettings['currency'] || 'CNY'));

// 暗色模式检测
const isDarkMode = computed<boolean>(() => theme.global.name.value === ThemeType.Dark);

// 是否始终显示导航栏（桌面端）
const alwaysShowNav = computed(() => display.mdAndUp.value);
const showNav = ref<boolean>(true);

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const editDialog = useTemplateRef<EditDialogType>('editDialog');
const fileInput = useTemplateRef<HTMLInputElement>('fileInput');
const historicalChartRef = useTemplateRef<ResizableChartComponent>('historicalChartRef');

// ============================================================================
// 响应式状态
// ============================================================================

const loading = ref<boolean>(false);
const updating = ref<boolean>(false);
const searchKeyword = ref<string>('');
const filterKeyword = ref<string>('');
const reloadRequestId = ref<number>(0);

// 视图模式：'budget' 或 'forecast'
const activeViewMode = ref<BudgetViewMode>('budget');
const forecastStrategy = ref<BudgetForecastStrategy>(BudgetForecastStrategy.HistoricalAverage);
const forecastMonthsHistory = ref<number>(6);
const forecastSortBy = ref<string>('backtest');
const forecastOnlyLowConfidence = ref<boolean>(false);
const forecastOnlyOverBudget = ref<boolean>(false);
const historicalBudgetLevel = ref<HistoricalBudgetLevel>('secondary');
const historicalLegendSelection = ref<HistoricalLegendSelection>({});
const historicalDateType = ref<number>(DateRange.RecentTwelveMonths.type);
const showHistoricalDateDialog = ref<boolean>(false);
const historicalMinDatetime = ref<number>(0);
const historicalMaxDatetime = ref<number>(0);

// 预算类型：支出或投资
const activeBudgetType = ref<BudgetType>(BudgetType.Expense);

// 周期筛选器
const activePeriodFilter = ref<string>('thisMonth');

// 自定义日期范围 - 使用日期范围选择对话框组件
const showCustomDateDialog = ref<boolean>(false);
const customMinDatetime = ref<number>(getTodayFirstUnixTime());  // Unix时间戳（秒）
const customMaxDatetime = ref<number>(getCurrentUnixTime());      // Unix时间戳（秒）
const customStartDate = ref<string>('');  // 保留用于reload()计算
const customEndDate = ref<string>('');    // 保留用于reload()计算

// 删除状态追踪
const budgetRemoving = ref<Record<string, boolean>>({});

// 筛选器值
const categoryFilter = ref<string | null>(null);
const accountFilter = ref<string[]>([]);
const tagFilter = ref<string[]>([]);
const executionRateFilter = ref<RangeFilter | null>(null);
const spentAmountFilter = ref<string>('');  // 格式: 'filterType:value1:value2'
const budgetAmountFilter = ref<string>('');  // 格式: 'filterType:value1:value2'

// 筛选对话框
const showFilterAccountDialog = ref<boolean>(false);
const showFilterTagDialog = ref<boolean>(false);
const showFilterCategoryDialog = ref<boolean>(false);
const showForecastSettingsDialog = ref<boolean>(false);

// 筛选预设
const showSavePresetDialog = ref<boolean>(false);
const presetName = ref<string>('');

interface FilterPreset {
    id: string;
    name: string;
    categoryFilter: string | null;
    accountFilter: string[];
    tagFilter: string[];
    executionRateFilter: RangeFilter | null;
    spentAmountFilter: string;
    budgetAmountFilter: string;
}

const filterPresets = ref<FilterPreset[]>([]);

// 从本地存储加载预设
function loadPresetsFromStorage(): void {
    const stored = localStorage.getItem('budgetFilterPresets');
    if (stored) {
        try {
            filterPresets.value = JSON.parse(stored);
        } catch {
            filterPresets.value = [];
        }
    }
}

// 将预设保存到本地存储
function savePresetsToStorage(): void {
    localStorage.setItem('budgetFilterPresets', JSON.stringify(filterPresets.value));
}

// 金额筛选临时值 (已花费)
const currentSpentFilterType = ref<string>('');
const currentSpentFilterValue1 = ref<number>(0);
const currentSpentFilterValue2 = ref<number>(0);

// 金额筛选临时值 (总预算)
const currentBudgetFilterType = ref<string>('');
const currentBudgetFilterValue1 = ref<number>(0);
const currentBudgetFilterValue2 = ref<number>(0);

// 排序
const sortBy = ref<string>('category');
const sortDesc = ref<boolean>(false);

// 折叠状态：记录哪些一级分类是折叠的
const collapsedCategories = ref<Set<string>>(new Set());

// ============================================================================
// 计算属性
// ============================================================================

const quickPeriodFilters = computed<PeriodFilter[]>(() => [
    { name: tt('This Month'), value: 'thisMonth' },
    { name: tt('This Quarter'), value: 'thisQuarter' },
    { name: tt('This Year'), value: 'thisYear' }
]);

const previousPeriodFilters = computed<PeriodFilter[]>(() => [
    { name: tt('Last Month'), value: 'lastMonth' },
    { name: tt('Last Quarter'), value: 'lastQuarter' },
    { name: tt('Last Year'), value: 'lastYear' }
]);

// 所有周期筛选选项（用于侧边栏列表）
const allPeriodFilters = computed<PeriodFilter[]>(() => [
    ...quickPeriodFilters.value,
    ...previousPeriodFilters.value,
    { name: tt('Expired Budgets'), value: 'expired' },
    { name: tt('Custom Range'), value: 'custom' }
]);

const forecastPeriodFilters = computed<PeriodFilter[]>(() => [
    { name: tt('Monthly Forecast'), value: 'thisMonth' },
    { name: tt('Quarterly Forecast'), value: 'thisQuarter' },
    { name: tt('Yearly Forecast'), value: 'thisYear' }
]);

const visiblePeriodFilters = computed<PeriodFilter[]>(() => {
    return activeViewMode.value === 'forecast'
        ? forecastPeriodFilters.value
        : allPeriodFilters.value;
});

// 当前选中的周期筛选索引（用于标签页）
const activePeriodFilterIndex = computed<number>(() => {
    return visiblePeriodFilters.value.findIndex(f => f.value === activePeriodFilter.value);
});

const allBudgets = computed<Budget[]>(() => budgetStore.allBudgets);

const filteredBudgets = computed<Budget[]>(() => {
    let budgets = allBudgets.value;

    // 按类型筛选
    budgets = budgets.filter(b => b.type === activeBudgetType.value);

    // 按周期筛选
    budgets = filterByPeriod(budgets);

    // 按关键词筛选
    if (searchKeyword.value) {
        const keyword = searchKeyword.value.toLowerCase();
        budgets = budgets.filter(b =>
            b.name.toLowerCase().includes(keyword) ||
            b.category.toLowerCase().includes(keyword) ||
            b.subCategory.toLowerCase().includes(keyword)
        );
    }

    // 按分类筛选（使用分类ID）
    if (categoryFilter.value) {
        const selectedCategory = allCategoriesMap.value[categoryFilter.value];
        if (selectedCategory) {
            budgets = budgets.filter(b => {
                // 如果选中的是一级分类，匹配该分类及其所有子分类
                if (!selectedCategory.parentId) {
                    return b.categoryId === categoryFilter.value ||
                           (selectedCategory.subCategories &&
                            selectedCategory.subCategories.some(sub => sub.id === b.categoryId));
                }
                // 如果选中的是二级分类，精确匹配
                return b.categoryId === categoryFilter.value;
            });
        }
    }

    // 按执行度筛选
    if (executionRateFilter.value) {
        const { min, max } = executionRateFilter.value;
        budgets = budgets.filter(b => {
            if (min !== undefined && b.executionRate < min) return false;
            if (max !== undefined && b.executionRate > max) return false;
            return true;
        });
    }

    // 按已花费筛选（使用新的字符串格式）
    if (spentAmountFilter.value) {
        const parsed = parseAmountFilter(spentAmountFilter.value);
        if (parsed) {
            budgets = budgets.filter(b => matchAmountFilter(b.spentAmountInYuan, parsed));
        }
    }

    // 按预算金额筛选（使用新的字符串格式）
    if (budgetAmountFilter.value) {
        const parsed = parseAmountFilter(budgetAmountFilter.value);
        if (parsed) {
            budgets = budgets.filter(b => matchAmountFilter(b.amountInYuan, parsed));
        }
    }

    return budgets;
});

const sortedBudgets = computed<Budget[]>(() => {
    const budgets = [...filteredBudgets.value];

    budgets.sort((a, b) => {
        let comparison = 0;

        switch (sortBy.value) {
            case 'category':
                comparison = (a.fullCategoryName || a.name).localeCompare(b.fullCategoryName || b.name);
                break;
            case 'executionRate':
                comparison = a.executionRate - b.executionRate;
                break;
            case 'spent':
                comparison = a.spentAmountInYuan - b.spentAmountInYuan;
                break;
            case 'budget':
                comparison = a.amountInYuan - b.amountInYuan;
                break;
        }

        return sortDesc.value ? -comparison : comparison;
    });

    return budgets;
});

/**
 * 层级预算组：按一级分类分组
 */
interface BudgetGroup {
    category: string;          // 一级分类名称
    categoryIcon: string;      // 分类图标
    categoryColor: string;     // 分类颜色
    primaryBudgets: Budget[];  // 一级分类预算列表（可能存在多条）
    subBudgets: Budget[];      // 二级分类预算列表
    totalAmount: number;       // 显示的总预算金额（分）- 优先使用一级分类预算，否则使用二级之和
    totalSpent: number;        // 显示的总已花费（分）- 优先使用一级分类已花费，否则使用二级之和
    subTotalAmount: number;    // 二级分类预算金额之和（分）
    subTotalSpent: number;     // 二级分类已花费之和（分）
    primaryAmount: number;     // 一级分类预算金额（分）
    primarySpent: number;      // 一级分类已花费（分）
    isCollapsed: boolean;      // 是否折叠
}

/**
 * 分组后的预算列表（层级显示用）
 */
const groupedBudgets = computed<BudgetGroup[]>(() => {
    const groups: Map<string, BudgetGroup> = new Map();

    // 构建一级分类名称到分类对象的映射（用于获取正确的图标和颜色）
    const primaryCategoryByName: Record<string, TransactionCategory> = {};
    for (const cat of budgetPrimaryCategories.value) {
        primaryCategoryByName[cat.name] = cat;
    }

    for (const budget of sortedBudgets.value) {
        const categoryKey = budget.category || budget.name;

        if (!groups.has(categoryKey)) {
            // 从分类配置中获取正确的图标和颜色（而非从预算数据中获取）
            const primaryCategory = primaryCategoryByName[categoryKey];
            const categoryIcon = primaryCategory?.icon || '';
            const categoryColor = primaryCategory?.color || '';

            groups.set(categoryKey, {
                category: categoryKey,
                categoryIcon: categoryIcon,
                categoryColor: categoryColor,
                primaryBudgets: [],
                subBudgets: [],
                totalAmount: 0,
                totalSpent: 0,
                subTotalAmount: 0,
                subTotalSpent: 0,
                primaryAmount: 0,
                primarySpent: 0,
                isCollapsed: collapsedCategories.value.has(categoryKey)
            });
        }

        const group = groups.get(categoryKey)!;

        // 判断是一级分类预算还是二级分类预算
        if (!budget.subCategory) {
            group.primaryBudgets.push(budget);
            group.primaryAmount += budget.amount;
            group.primarySpent += budget.spentAmount;
        } else {
            // 二级分类预算
            group.subBudgets.push(budget);
            // 累加二级分类的金额和花费
            group.subTotalAmount += budget.amount;
            group.subTotalSpent += budget.spentAmount;
        }
    }

    // 计算显示的总金额和总花费
    // 规则：
    // 1. 如果有一级分类预算，使用一级分类的预算金额（用户设置的总预算）
    // 2. 如果没有一级分类预算，使用二级分类之和
    // 3. 已花费金额：优先使用一级分类的已花费，否则使用二级之和
    for (const group of groups.values()) {
        if (group.primaryBudgets.length > 1) {
            group.totalAmount = group.primaryAmount + group.subTotalAmount;
            group.totalSpent = group.primarySpent + group.subTotalSpent;
        } else if (group.primaryBudgets.length > 0) {
            // 有一级分类预算：使用一级分类的预算金额
            group.totalAmount = group.primaryAmount;
            // 已花费：如果一级分类有值就用一级的，否则用二级之和
            group.totalSpent = group.primarySpent > 0 ? group.primarySpent : group.subTotalSpent;
        } else if (group.subBudgets.length > 0) {
            // 只有二级分类预算：使用二级之和
            group.totalAmount = group.subTotalAmount;
            group.totalSpent = group.subTotalSpent;
        } else {
            // 既没有一级也没有二级：金额为0
            group.totalAmount = 0;
            group.totalSpent = 0;
        }
    }

    // 转换为数组并按分类名称排序
    return Array.from(groups.values()).sort((a, b) => a.category.localeCompare(b.category));
});

function getPrimaryBudgetForHeader(group: BudgetGroup): Budget | null {
    return group.primaryBudgets[0] || null;
}

function getExpandedPrimaryBudgets(group: BudgetGroup): Budget[] {
    return group.primaryBudgets.length > 1 ? group.primaryBudgets : [];
}

function groupHasExpandedRows(group: BudgetGroup): boolean {
    return getExpandedPrimaryBudgets(group).length > 0 || group.subBudgets.length > 0;
}

/**
 * 切换分类折叠状态
 */
function toggleCategoryCollapse(category: string): void {
    if (collapsedCategories.value.has(category)) {
        collapsedCategories.value.delete(category);
    } else {
        collapsedCategories.value.add(category);
    }
    // 强制更新
    collapsedCategories.value = new Set(collapsedCategories.value);
}

/**
 * 检查是否全部展开
 */
const isAllExpanded = computed<boolean>(() => {
    if (groupedBudgets.value.length === 0) return true;
    return collapsedCategories.value.size === 0;
});

/**
 * 切换全部展开/折叠（带动画）
 */
function toggleAllCategories(): void {
    if (isAllExpanded.value) {
        // 全部折叠
        const allCategories = groupedBudgets.value.map(g => g.category);
        collapsedCategories.value = new Set(allCategories);
    } else {
        // 全部展开
        collapsedCategories.value = new Set();
    }
}

// 根据预算类型获取对应的分类类型
const currentCategoryType = computed<number>(() => {
    return activeBudgetType.value === BudgetType.Expense
        ? CategoryType.Expense
        : CategoryType.Investment;
});

// 获取当前允许的分类类型字符串（用于分类筛选对话框）
const allowedCategoryTypes = computed<string>(() => {
    return String(currentCategoryType.value);
});

// 获取当前预算类型的一级分类列表
const budgetPrimaryCategories = computed<TransactionCategory[]>(() => {
    const categories = transactionCategoriesStore.allTransactionCategories[currentCategoryType.value];
    return categories || [];
});

// 分类ID到分类对象的映射
const allCategoriesMap = computed<Record<string, TransactionCategory>>(() => {
    const map: Record<string, TransactionCategory> = {};

    for (const primaryCategory of budgetPrimaryCategories.value) {
        map[primaryCategory.id] = primaryCategory;
        if (primaryCategory.subCategories) {
            for (const subCategory of primaryCategory.subCategories) {
                map[subCategory.id] = subCategory;
            }
        }
    }

    return map;
});

const executionRateOptions = computed<RangeFilter[]>(() => [
    { label: tt('Under 50%'), value: 'under50', min: 0, max: 50 },
    { label: tt('50% - 80%'), value: '50to80', min: 50, max: 80 },
    { label: tt('80% - 100%'), value: '80to100', min: 80, max: 100 },
    { label: tt('Over Budget'), value: 'over100', min: 100, max: undefined }
]);

// 所有账户列表
const allAccounts = computed(() => accountsStore.allAccounts);

// 所有标签列表
const allTransactionTags = computed(() => transactionTagsStore.allTransactionTags);

const hasActiveFilters = computed<boolean>(() => {
    return !!(categoryFilter.value || executionRateFilter.value ||
              spentAmountFilter.value || budgetAmountFilter.value ||
              accountFilter.value.length > 0 || tagFilter.value.length > 0);
});

const currentExecution = computed<BudgetExecutionResponse | null>(() => budgetStore.currentExecution);
const currentForecast = computed<BudgetForecastResponse | null>(() => budgetStore.currentForecast);
const currentHistory = computed<BudgetHistoryResponse | null>(() => budgetStore.currentHistory);
const currentHistoryRequestSignature = computed<string>(() => budgetStore.currentHistoryRequestSignature);
const forecastLoading = computed<boolean>(() => budgetStore.forecastLoading);
const firstDayOfWeek = computed(() => userStore.currentUserFirstDayOfWeek);
const viewModeButtons = computed(() => [
    { name: tt('Budget Management'), value: 'budget' },
    { name: tt('Period Forecast'), value: 'forecast' },
    { name: tt('Expired Budgets'), value: 'history' }
]);
const currentViewTitle = computed(() => {
    if (activeViewMode.value === 'forecast') {
        return tt('Period Forecast');
    }
    if (activeViewMode.value === 'history') {
        return tt('Historical Budget Execution');
    }
    return tt('Budget Management');
});
const historicalLevelButtons = computed(() => [
    { name: tt('Primary Category Budget'), value: 'primary' },
    { name: tt('Secondary Category Budget'), value: 'secondary' }
]);
const allHistoricalDateRanges = computed(() => getAllDateRanges(DateRangeScene.AssetTrends, true, false));
const historicalDateRangeName = computed(() => {
    if (!historicalMinDatetime.value || !historicalMaxDatetime.value) {
        return tt('Recent 12 months');
    }
    return formatDateRange(historicalDateType.value, historicalMinDatetime.value, historicalMaxDatetime.value);
});
const historicalDateStartText = computed(() => historicalMinDatetime.value ? formatDateOnly(new Date(historicalMinDatetime.value * 1000)) : '');
const historicalDateEndText = computed(() => historicalMaxDatetime.value ? formatDateOnly(new Date(historicalMaxDatetime.value * 1000)) : '');
const canShiftHistoricalDateRange = computed(() => {
    return historicalDateType.value !== DateRange.All.type && !!historicalMinDatetime.value && !!historicalMaxDatetime.value;
});
const forecastStrategies = computed(() => [
    { name: tt('Historical Average Strategy'), value: BudgetForecastStrategy.HistoricalAverage },
    { name: tt('Moving Average Strategy'), value: BudgetForecastStrategy.MovingAverage }
]);
const historyPeriodOptions = computed<number[]>(() => [3, 6, 9, 12]);
const forecastSortOptions = computed(() => [
    { name: tt('Sort by Backtest MAPE'), value: 'backtest' },
    { name: tt('Sort by Confidence'), value: 'confidence' },
    { name: tt('Sort by Projected Total'), value: 'projected_total' },
    { name: tt('Sort by Category'), value: 'category' }
]);

const forecastRiskSummary = computed(() => {
    return summarizeForecastRisks(currentForecast.value?.forecasts || [], displayForecasts.value.length);
});

const displayForecasts = computed(() => {
    return filterAndSortForecasts(currentForecast.value?.forecasts || [], {
        sortBy: forecastSortBy.value as 'backtest' | 'confidence' | 'projected_total' | 'category',
        onlyLowConfidence: forecastOnlyLowConfidence.value,
        onlyOverBudget: forecastOnlyOverBudget.value
    });
});

/**
 * 抬头汇总：按当前筛选后的可见分组聚合，避免全量预算总和
 */
const filteredSummary = computed(() => {
    let totalBudget = 0;
    let totalSpent = 0;

    for (const group of groupedBudgets.value) {
        totalBudget += group.totalAmount;
        totalSpent += group.totalSpent;
    }

    const totalExecutionRate = totalBudget > 0 ? (totalSpent / totalBudget) * 100 : 0;

    return {
        totalBudget,
        totalSpent,
        totalExecutionRate
    };
});

type HistoricalAggregationType = BudgetPeriodType | 'fiscal_year';

interface HistoricalAggregationOption {
    name: string;
    value: HistoricalAggregationType;
}

interface HistoricalPeriodRange {
    key: string;
    label: string;
    startDate: string;
    endDate: string;
}

/**
 * 往期预算趋势数据（一级分类）
 */
const HISTORICAL_FISCAL_YEAR = 'fiscal_year' as const;
const historicalAggregationType = ref<HistoricalAggregationType>(BudgetPeriodType.Monthly);

const fiscalYearStartValue = computed<number>(() => userStore.currentUserFiscalYearStart);

const fiscalYearStartInfo = computed(() => {
    return FiscalYearStart.valueOf(fiscalYearStartValue.value) || FiscalYearStart.Default;
});

const historicalAggregationOptions = computed<HistoricalAggregationOption[]>(() => [
    { name: tt('Monthly'), value: BudgetPeriodType.Monthly },
    { name: tt('Quarterly'), value: BudgetPeriodType.Quarterly },
    { name: tt('Yearly'), value: BudgetPeriodType.Yearly },
    { name: tt('FiscalYearly'), value: HISTORICAL_FISCAL_YEAR }
]);

function parseDateOnly(text: string): Date | null {
    if (!text) {
        return null;
    }

    const parsed = new Date(`${text}T00:00:00`);
    return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function normalizeHistoryAmountCents(value: number): number {
    const amount = Number(value);
    return Number.isFinite(amount) ? Math.max(0, amount) : 0;
}

function addMonths(sourceDate: Date, months: number): Date {
    return new Date(sourceDate.getFullYear(), sourceDate.getMonth() + months, 1);
}

const historicalAggregationLabel = computed(() => {
    const matched = historicalAggregationOptions.value.find(item => item.value === historicalAggregationType.value);
    return matched?.name || tt('Monthly');
});

function isValidHistoricalUnixTime(value: number): boolean {
    return Number.isFinite(value) && value > 0;
}

function getDefaultHistoricalDateRange(): { dateType: number; minTime: number; maxTime: number } | null {
    return getDateRangeByDateType(
        historicalDateType.value,
        firstDayOfWeek.value,
        fiscalYearStartValue.value
    ) ?? getDateRangeByDateType(
        DateRange.RecentTwelveMonths.type,
        firstDayOfWeek.value,
        fiscalYearStartValue.value
    );
}

function getHistoricalDateRangeSnapshot(): { dateType: number; minTime: number; maxTime: number } {
    if (
        isValidHistoricalUnixTime(historicalMinDatetime.value)
        && isValidHistoricalUnixTime(historicalMaxDatetime.value)
        && historicalMinDatetime.value <= historicalMaxDatetime.value
    ) {
        return {
            dateType: historicalDateType.value,
            minTime: historicalMinDatetime.value,
            maxTime: historicalMaxDatetime.value
        };
    }

    const fallbackRange = getDefaultHistoricalDateRange();
    if (fallbackRange) {
        return fallbackRange;
    }

    const now = getCurrentUnixTime();
    return {
        dateType: DateRange.RecentTwelveMonths.type,
        minTime: now,
        maxTime: now
    };
}

function getHistoricalBudgetQueryRange(): { startDate: string; endDate: string } {
    const range = getHistoricalDateRangeSnapshot();

    return {
        startDate: formatDateOnly(new Date(range.minTime * 1000)),
        endDate: formatDateOnly(new Date(range.maxTime * 1000))
    };
}

function buildBudgetHistoryRequestSignature(req: BudgetHistoryRequest): string {
    return JSON.stringify({
        type: req.type ?? null,
        periodType: req.periodType ?? null,
        year: req.year ?? null,
        month: req.month ?? null,
        quarter: req.quarter ?? null,
        startDate: req.startDate ?? null,
        endDate: req.endDate ?? null,
        categoryId: req.categoryId ?? null,
        accountIds: req.accountIds ?? [],
        tagIds: req.tagIds ?? []
    });
}

const activeHistoricalHistoryRequestSignature = computed<string>(() => {
    const historyRange = getHistoricalBudgetQueryRange();

    return buildBudgetHistoryRequestSignature({
        type: activeBudgetType.value,
        periodType: BudgetPeriodType.Monthly,
        startDate: historyRange.startDate,
        endDate: historyRange.endDate,
        categoryId: categoryFilter.value || undefined,
        accountIds: accountFilter.value.length ? [...accountFilter.value] : undefined,
        tagIds: tagFilter.value.length ? [...tagFilter.value] : undefined
    });
});

const isHistoricalHistoryReady = computed<boolean>(() => (
    !!currentHistory.value
    && currentHistoryRequestSignature.value === activeHistoricalHistoryRequestSignature.value
));

function buildFiscalYearPeriod(targetDate: Date): HistoricalPeriodRange {
    const fiscalStart = fiscalYearStartInfo.value;
    const currentYearFiscalStart = new Date(targetDate.getFullYear(), fiscalStart.month - 1, fiscalStart.day);
    const fiscalStartYear = targetDate >= currentYearFiscalStart
        ? targetDate.getFullYear()
        : targetDate.getFullYear() - 1;
    const startDate = new Date(fiscalStartYear, fiscalStart.month - 1, fiscalStart.day);
    const endDate = new Date(fiscalStartYear + 1, fiscalStart.month - 1, fiscalStart.day - 1);
    const endYear = endDate.getFullYear();

    return {
        key: `FY${endYear}`,
        label: `FY${endYear}`,
        startDate: formatDateOnly(startDate),
        endDate: formatDateOnly(endDate)
    };
}

function buildHistoricalAggregationPeriods(
    aggregationType: HistoricalAggregationType,
    startDate: string,
    endDate: string
): HistoricalPeriodRange[] {
    const start = parseDateOnly(startDate);
    const end = parseDateOnly(endDate);

    if (!start || !end || start > end) {
        return [];
    }

    const periods: HistoricalPeriodRange[] = [];

    if (aggregationType === BudgetPeriodType.Monthly) {
        let cursor = new Date(start.getFullYear(), start.getMonth(), 1);
        while (cursor <= end) {
            const periodEnd = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 0);
            const key = `${cursor.getFullYear()}-${String(cursor.getMonth() + 1).padStart(2, '0')}`;
            periods.push({
                key,
                label: key,
                startDate: formatDateOnly(cursor),
                endDate: formatDateOnly(periodEnd)
            });
            cursor = addMonths(cursor, 1);
        }
        return periods;
    }

    if (aggregationType === BudgetPeriodType.Quarterly) {
        let cursor = new Date(start.getFullYear(), Math.floor(start.getMonth() / 3) * 3, 1);
        while (cursor <= end) {
            const quarter = Math.floor(cursor.getMonth() / 3) + 1;
            const periodEnd = new Date(cursor.getFullYear(), cursor.getMonth() + 3, 0);
            const key = `${cursor.getFullYear()}-Q${quarter}`;
            periods.push({
                key,
                label: key,
                startDate: formatDateOnly(cursor),
                endDate: formatDateOnly(periodEnd)
            });
            cursor = addMonths(cursor, 3);
        }
        return periods;
    }

    if (aggregationType === BudgetPeriodType.Yearly) {
        let cursor = new Date(start.getFullYear(), 0, 1);
        while (cursor <= end) {
            const periodEnd = new Date(cursor.getFullYear(), 11, 31);
            const key = `${cursor.getFullYear()}`;
            periods.push({
                key,
                label: key,
                startDate: formatDateOnly(cursor),
                endDate: formatDateOnly(periodEnd)
            });
            cursor = new Date(cursor.getFullYear() + 1, 0, 1);
        }
        return periods;
    }

    let cursor = parseDateOnly(buildFiscalYearPeriod(start).startDate);
    while (cursor && cursor <= end) {
        const fiscalPeriod = buildFiscalYearPeriod(cursor);
        periods.push(fiscalPeriod);
        cursor = new Date(cursor.getFullYear() + 1, cursor.getMonth(), cursor.getDate());
    }

    return periods;
}

function resolveHistoricalPeriodByDate(
    aggregationType: HistoricalAggregationType,
    targetDate: Date
): HistoricalPeriodRange {
    if (aggregationType === BudgetPeriodType.Monthly) {
        const key = `${targetDate.getFullYear()}-${String(targetDate.getMonth() + 1).padStart(2, '0')}`;
        return {
            key,
            label: key,
            startDate: formatDateOnly(new Date(targetDate.getFullYear(), targetDate.getMonth(), 1)),
            endDate: formatDateOnly(new Date(targetDate.getFullYear(), targetDate.getMonth() + 1, 0))
        };
    }

    if (aggregationType === BudgetPeriodType.Quarterly) {
        const quarterStartMonth = Math.floor(targetDate.getMonth() / 3) * 3;
        const quarter = Math.floor(targetDate.getMonth() / 3) + 1;
        return {
            key: `${targetDate.getFullYear()}-Q${quarter}`,
            label: `${targetDate.getFullYear()}-Q${quarter}`,
            startDate: formatDateOnly(new Date(targetDate.getFullYear(), quarterStartMonth, 1)),
            endDate: formatDateOnly(new Date(targetDate.getFullYear(), quarterStartMonth + 3, 0))
        };
    }

    if (aggregationType === BudgetPeriodType.Yearly) {
        return {
            key: `${targetDate.getFullYear()}`,
            label: `${targetDate.getFullYear()}`,
            startDate: `${targetDate.getFullYear()}-01-01`,
            endDate: `${targetDate.getFullYear()}-12-31`
        };
    }

    return buildFiscalYearPeriod(targetDate);
}

const filteredHistoricalItems = computed<BudgetHistoryItem[]>(() => {
    if (!isHistoricalHistoryReady.value || !currentHistory.value?.items?.length) {
        return [];
    }

    const selectedCategory = categoryFilter.value ? allCategoriesMap.value[categoryFilter.value] : null;
    const selectedPrimaryCategory = selectedCategory?.parentId
        ? allCategoriesMap.value[selectedCategory.parentId]
        : selectedCategory;

    return currentHistory.value.items.filter((item: BudgetHistoryItem) => {
        if (!selectedCategory) {
            return true;
        }

        const selectedName = selectedCategory.name;
        const selectedPrimaryName = selectedPrimaryCategory?.name || selectedName;

        if (selectedCategory.parentId) {
            return item.category === selectedPrimaryName && item.subCategory === selectedName;
        }

        return item.category === selectedPrimaryName;
    });
});

const historicalPeriods = computed<HistoricalPeriodRange[]>(() => {
    const range = getHistoricalBudgetQueryRange();
    return buildHistoricalAggregationPeriods(
        historicalAggregationType.value,
        range.startDate,
        range.endDate
    );
});

const historicalCategoryChartData = computed<{ categories: string[]; points: HistoricalCategoryChartPoint[] }>(() => {
    const periods = historicalPeriods.value;
    const items = filteredHistoricalItems.value;

    if (!periods.length || !items.length) {
        return { categories: [], points: [] };
    }

    const periodMap = new Map(periods.map(period => [period.key, period]));

    const categoryColorMap: Record<string, string> = {};
    const categoryOrderMap: Record<string, number> = {};
    const subCategoryColorMap: Record<string, string> = {};
    const subCategoryOrderMap: Record<string, number> = {};
    for (const cat of budgetPrimaryCategories.value) {
        if (cat.color) {
            categoryColorMap[cat.name] = `#${cat.color}`;
        }
        categoryOrderMap[cat.name] = cat.displayOrder ?? 0;

        for (const subCategory of cat.subCategories || []) {
            if (subCategory.color) {
                subCategoryColorMap[`${cat.name}::${subCategory.name}`] = `#${subCategory.color}`;
            }
            subCategoryOrderMap[`${cat.name}::${subCategory.name}`] = subCategory.displayOrder ?? 0;
        }
    }

    const groupedByPeriodAndPrimary = new Map<string, {
        primaryCategory: string;
        groupOrder: number;
        primaryBudgetAmount: number;
        primarySpentAmount: number;
        secondaryBudgetAmount: number;
        secondarySpentAmount: number;
        secondaryItems: Map<string, {
            displayCategory: string;
            secondaryCategory: string;
            budgetAmount: number;
            spentAmount: number;
            itemOrder: number;
        }>;
    }>();

    for (const item of items) {
        const itemDate = parseDateOnly(item.periodStart);
        if (!itemDate) continue;

        const resolvedPeriod = resolveHistoricalPeriodByDate(historicalAggregationType.value, itemDate);
        if (!periodMap.has(resolvedPeriod.key)) continue;

        const primaryCategory = String(item.category || '').trim() || tt('Uncategorized');
        const secondaryCategory = String(item.subCategory || '').trim();
        const budgetAmount = normalizeHistoryAmountCents(item.budgetAmount);
        const spentAmount = normalizeHistoryAmountCents(item.spentAmount);
        const periodPrimaryKey = `${resolvedPeriod.key}::${primaryCategory}`;

        if (!groupedByPeriodAndPrimary.has(periodPrimaryKey)) {
            groupedByPeriodAndPrimary.set(periodPrimaryKey, {
                primaryCategory,
                groupOrder: categoryOrderMap[primaryCategory] ?? Number.MAX_SAFE_INTEGER,
                primaryBudgetAmount: 0,
                primarySpentAmount: 0,
                secondaryBudgetAmount: 0,
                secondarySpentAmount: 0,
                secondaryItems: new Map()
            });
        }

        const summary = groupedByPeriodAndPrimary.get(periodPrimaryKey)!;

        if (!secondaryCategory) {
            summary.primaryBudgetAmount += budgetAmount;
            summary.primarySpentAmount += spentAmount;
            continue;
        }

        const secondaryKey = `${primaryCategory}::${secondaryCategory}`;
        if (!summary.secondaryItems.has(secondaryKey)) {
            summary.secondaryItems.set(secondaryKey, {
                displayCategory: secondaryCategory,
                secondaryCategory,
                budgetAmount: 0,
                spentAmount: 0,
                itemOrder: subCategoryOrderMap[secondaryKey] ?? 0
            });
        }

        const secondarySummary = summary.secondaryItems.get(secondaryKey)!;
        secondarySummary.budgetAmount += budgetAmount;
        secondarySummary.spentAmount += spentAmount;
        summary.secondaryBudgetAmount += budgetAmount;
        summary.secondarySpentAmount += spentAmount;
    }

    const grouped = new Map<string, {
        displayCategory: string;
        primaryCategory: string;
        secondaryCategory: string;
        budgetAmount: number;
        spentAmount: number;
        groupOrder: number;
        itemOrder: number;
    }>();

    for (const summary of groupedByPeriodAndPrimary.values()) {
        if (historicalBudgetLevel.value === 'primary') {
            const groupKey = summary.primaryCategory;
            const hasPrimaryBudget = summary.primaryBudgetAmount > 0 || summary.primarySpentAmount > 0;
            const budgetAmount = hasPrimaryBudget ? summary.primaryBudgetAmount : summary.secondaryBudgetAmount;
            const spentAmount = hasPrimaryBudget
                ? (summary.primarySpentAmount > 0 ? summary.primarySpentAmount : summary.secondarySpentAmount)
                : summary.secondarySpentAmount;

            if (!grouped.has(groupKey)) {
                grouped.set(groupKey, {
                    displayCategory: summary.primaryCategory,
                    primaryCategory: summary.primaryCategory,
                    secondaryCategory: '',
                    budgetAmount: 0,
                    spentAmount: 0,
                    groupOrder: summary.groupOrder,
                    itemOrder: 0
                });
            }

            const primarySummary = grouped.get(groupKey)!;
            primarySummary.budgetAmount += budgetAmount;
            primarySummary.spentAmount += spentAmount;
            continue;
        }

        const visibleSecondaryEntries = Array.from(summary.secondaryItems.entries())
            .filter(([, secondarySummary]) => secondarySummary.budgetAmount > 0 || secondarySummary.spentAmount > 0);
        const secondaryEntries: Array<[string, {
            displayCategory: string;
            secondaryCategory: string;
            budgetAmount: number;
            spentAmount: number;
            itemOrder: number;
        }]> = visibleSecondaryEntries.length > 0
            ? visibleSecondaryEntries
            : [[
                `${summary.primaryCategory}::${summary.primaryCategory}`,
                {
                    displayCategory: summary.primaryCategory,
                    secondaryCategory: summary.primaryCategory,
                    budgetAmount: summary.primaryBudgetAmount,
                    spentAmount: summary.primarySpentAmount,
                    itemOrder: 0
                }
            ]];

        for (const [secondaryKey, secondarySummary] of secondaryEntries) {
            if (secondarySummary.budgetAmount <= 0 && secondarySummary.spentAmount <= 0) {
                continue;
            }

            if (!grouped.has(secondaryKey)) {
                grouped.set(secondaryKey, {
                    displayCategory: secondarySummary.displayCategory,
                    primaryCategory: summary.primaryCategory,
                    secondaryCategory: secondarySummary.secondaryCategory,
                    budgetAmount: 0,
                    spentAmount: 0,
                    groupOrder: summary.groupOrder,
                    itemOrder: secondarySummary.itemOrder
                });
            }

            const groupedSecondarySummary = grouped.get(secondaryKey)!;
            groupedSecondarySummary.budgetAmount += secondarySummary.budgetAmount;
            groupedSecondarySummary.spentAmount += secondarySummary.spentAmount;
        }
    }

    const points: HistoricalCategoryChartPoint[] = [];
    let colorIdx = 0;

    for (const [, summary] of grouped) {
        const budgetAmount = summary.budgetAmount / 100;
        const spentAmount = summary.spentAmount / 100;
        const executionRate = summary.budgetAmount > 0
            ? (summary.spentAmount / summary.budgetAmount) * 100
            : 0;
        const subCategoryKey = `${summary.primaryCategory}::${summary.secondaryCategory}`;
        const color: string = (historicalBudgetLevel.value === 'secondary'
            ? subCategoryColorMap[subCategoryKey]
            : undefined)
            ?? categoryColorMap[summary.primaryCategory]
            ?? CATEGORY_CHART_PALETTE[colorIdx % CATEGORY_CHART_PALETTE.length] ?? '#5470c6';
        colorIdx++;

        points.push({
            category: summary.displayCategory,
            primaryCategory: summary.primaryCategory,
            secondaryCategory: summary.secondaryCategory,
            budgetAmount,
            spentAmount,
            executionRate: Number(executionRate.toFixed(1)),
            color,
            groupOrder: summary.groupOrder,
            itemOrder: summary.itemOrder
        });
    }

    points.sort((a, b) => {
        if (a.groupOrder !== b.groupOrder) {
            return a.groupOrder - b.groupOrder;
        }
        if (historicalBudgetLevel.value === 'secondary' && a.itemOrder !== b.itemOrder) {
            return a.itemOrder - b.itemOrder;
        }
        if (a.primaryCategory !== b.primaryCategory) {
            return a.primaryCategory.localeCompare(b.primaryCategory, 'zh-CN');
        }
        return a.category.localeCompare(b.category, 'zh-CN');
    });

    return { categories: points.map(p => p.category), points };
});

watch(
    () => historicalCategoryChartData.value.points.map(point => `${point.primaryCategory}::${point.secondaryCategory || point.category}`).join('|'),
    () => {
        historicalLegendSelection.value = syncHistoricalLegendSelection(
            historicalCategoryChartData.value.points,
            historicalLegendSelection.value
        );
        if (activeViewMode.value === 'history') {
            scheduleHistoricalChartResize();
        }
    },
    { immediate: true }
);

watch(activeBudgetType, () => {
    historicalLegendSelection.value = {};
    resetHistoricalLabelAnimationState(historicalLabelAnimationState);
    if (activeViewMode.value === 'history') {
        scheduleHistoricalChartResize();
    }
});

watch(historicalBudgetLevel, () => {
    historicalLegendSelection.value = {};
    resetHistoricalCategoryAnimationState(historicalLabelAnimationState);
    if (activeViewMode.value === 'history') {
        scheduleHistoricalChartResize();
    }
});

const historicalChartModel = computed(() => {
    return buildHistoricalPolarChartModel(
        historicalCategoryChartData.value.points,
        historicalLegendSelection.value
    );
});
const historicalLabelAnimationState = createHistoricalLabelAnimationState();

const historicalLegendGroups = computed(() => historicalChartModel.value.legendGroups);
const historicalChartUpdateOptions = {
    notMerge: false,
    lazyUpdate: false,
    replaceMerge: ['series']
};
// Bump this when the historical chart's graphic/custom transition contract changes.
// It forces one component remount so old ECharts instances cannot keep stale leaveTo internals.
const HISTORICAL_CHART_RENDER_REVISION = 'history-animation-restore-v6';
const historicalChartRenderKey = computed(() => `${HISTORICAL_CHART_RENDER_REVISION}:${activeBudgetType.value}`);

const historicalChartOptions = computed(() => {
    if (!historicalChartModel.value.primaryBands.length) {
        return {};
    }

    const accentColor = activeBudgetType.value === BudgetType.Investment ? '#ffb300' : '#5c6bc0';
    const animationScope = `${activeBudgetType.value}-${historicalBudgetLevel.value}`;
    return buildHistoricalPolarChartOption(historicalChartModel.value, {
        isDarkMode: isDarkMode.value,
        accentColor,
        budgetAmountLabel: tt('Budget Amount'),
        spentAmountLabel: tt('Spent Amount'),
        executionRateLabel: tt('Execution Rate'),
        formatAmount,
        showPrimaryRing: historicalBudgetLevel.value === 'secondary',
        categoryAnimationScope: animationScope,
        amountAxisRenderScope: animationScope,
        labelAnimationState: historicalLabelAnimationState
    });
});

watch(activeViewMode, (mode) => {
    if (mode === 'history') {
        scheduleHistoricalChartResize();
    }
}, { flush: 'post' });

watch(historicalChartOptions, () => {
    if (activeViewMode.value === 'history') {
        scheduleHistoricalChartResize();
    }
}, { flush: 'post' });

function scheduleHistoricalChartResize(): void {
    if (typeof window === 'undefined') {
        return;
    }

    void nextTick(() => {
        window.requestAnimationFrame(() => {
            const chartComponent = historicalChartRef.value;
            chartComponent?.resize?.();
            chartComponent?.chart?.resize?.();
        });
    });
}

function toggleHistoricalPrimaryLegend(primaryKey: string): void {
    historicalLegendSelection.value = toggleHistoricalPrimarySelection(
        historicalLegendSelection.value,
        historicalChartModel.value,
        primaryKey
    );
}

function toggleHistoricalSecondaryLegend(secondaryKey: string): void {
    historicalLegendSelection.value = toggleHistoricalSecondarySelection(
        historicalLegendSelection.value,
        secondaryKey
    );
}

function ensureHistoricalDateRangeInitialized(): void {
    if (
        isValidHistoricalUnixTime(historicalMinDatetime.value)
        && isValidHistoricalUnixTime(historicalMaxDatetime.value)
        && historicalMinDatetime.value <= historicalMaxDatetime.value
    ) {
        return;
    }

    const range = getDefaultHistoricalDateRange();

    if (!range) {
        return;
    }

    historicalDateType.value = range.dateType;
    historicalMinDatetime.value = range.minTime;
    historicalMaxDatetime.value = range.maxTime;
}

function canShowHistoricalCustomDateRange(dateType: number): boolean {
    return historicalDateType.value === DateRange.Custom.type && dateType === DateRange.Custom.type;
}

function setHistoricalDateFilter(dateType: number): void {
    if (dateType === DateRange.Custom.type) {
        ensureHistoricalDateRangeInitialized();
        showHistoricalDateDialog.value = true;
        return;
    }

    const range = getDateRangeByDateType(dateType, firstDayOfWeek.value, fiscalYearStartValue.value);
    if (!range) {
        return;
    }

    historicalDateType.value = range.dateType;
    historicalMinDatetime.value = range.minTime;
    historicalMaxDatetime.value = range.maxTime;
    reload(false);
}

function onHistoricalDateRangeChange(minUnixTime: number, maxUnixTime: number): void {
    historicalDateType.value = getDateTypeByDateRange(
        minUnixTime,
        maxUnixTime,
        firstDayOfWeek.value,
        fiscalYearStartValue.value,
        DateRangeScene.AssetTrends
    );
    historicalMinDatetime.value = minUnixTime;
    historicalMaxDatetime.value = maxUnixTime;
    showHistoricalDateDialog.value = false;
    reload(false);
}

function shiftHistoricalDateRange(scale: number): void {
    if (!canShiftHistoricalDateRange.value) {
        return;
    }

    const range = getShiftedDateRangeAndDateType(
        historicalMinDatetime.value,
        historicalMaxDatetime.value,
        scale,
        firstDayOfWeek.value,
        fiscalYearStartValue.value,
        DateRangeScene.AssetTrends
    );

    historicalDateType.value = range.dateType;
    historicalMinDatetime.value = range.minTime;
    historicalMaxDatetime.value = range.maxTime;
    reload(false);
}

async function loadHistoricalBudgetView(requestId: number): Promise<void> {
    const historyRange = getHistoricalBudgetQueryRange();
    const historyRequest: BudgetHistoryRequest = {
        type: activeBudgetType.value,
        periodType: BudgetPeriodType.Monthly,
        startDate: historyRange.startDate,
        endDate: historyRange.endDate,
        categoryId: categoryFilter.value || undefined,
        accountIds: accountFilter.value.length ? [...accountFilter.value] : undefined,
        tagIds: tagFilter.value.length ? [...tagFilter.value] : undefined
    };

    try {
        await budgetStore.createBudgetHistorySnapshot({
            ...historyRequest,
            type: activeBudgetType.value
        });
    } catch (snapshotError) {
        logger.warn('[Budget List] Failed to create history snapshot during historical view load', snapshotError);
    }

    if (requestId !== reloadRequestId.value) {
        return;
    }

    await budgetStore.loadBudgetHistory(historyRequest);
    if (activeViewMode.value === 'history') {
        scheduleHistoricalChartResize();
    }
}

async function loadBudgetHistoryForExpired(requestId: number): Promise<void> {
    await loadHistoricalBudgetView(requestId);
}

// ============================================================================
// 方法
// ============================================================================

/**
 * 根据周期筛选预算
 */
function filterByPeriod(budgets: Budget[]): Budget[] {
    const now = new Date();
    const currentYear = now.getFullYear();
    const currentMonth = now.getMonth();
    const currentQuarter = Math.floor(currentMonth / 3);

    switch (activePeriodFilter.value) {
        case 'thisMonth': {
            const startDate = new Date(currentYear, currentMonth, 1);
            const endDate = new Date(currentYear, currentMonth + 1, 0);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Monthly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'lastMonth': {
            const startDate = new Date(currentYear, currentMonth - 1, 1);
            const endDate = new Date(currentYear, currentMonth, 0);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Monthly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'thisQuarter': {
            const startDate = new Date(currentYear, currentQuarter * 3, 1);
            const endDate = new Date(currentYear, currentQuarter * 3 + 3, 0);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Quarterly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'lastQuarter': {
            const prevQuarter = currentQuarter === 0 ? 3 : currentQuarter - 1;
            const year = currentQuarter === 0 ? currentYear - 1 : currentYear;
            const startDate = new Date(year, prevQuarter * 3, 1);
            const endDate = new Date(year, prevQuarter * 3 + 3, 0);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Quarterly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'thisYear': {
            const startDate = new Date(currentYear, 0, 1);
            const endDate = new Date(currentYear, 11, 31);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Yearly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'lastYear': {
            const startDate = new Date(currentYear - 1, 0, 1);
            const endDate = new Date(currentYear - 1, 11, 31);
            return budgets.filter(b => b.periodType === BudgetPeriodType.Yearly &&
                isDateInRange(b, startDate, endDate));
        }
        case 'expired': {
            const today = new Date();
            today.setHours(0, 0, 0, 0);
            return budgets.filter(b => {
                if (!b.endDate) return false;
                const endDate = new Date(b.endDate);
                return endDate < today;
            });
        }
        case 'custom': {
            if (!customStartDate.value || !customEndDate.value) return budgets;
            const startDate = new Date(customStartDate.value);
            const endDate = new Date(customEndDate.value);
            return budgets.filter(b => isDateInRange(b, startDate, endDate));
        }
        default:
            return budgets;
    }
}

/**
 * 检查预算是否在日期范围内
 */
function isDateInRange(budget: Budget, startDate: Date, endDate: Date): boolean {
    if (!budget.startDate || !budget.endDate) return true;
    const budgetStart = new Date(budget.startDate);
    const budgetEnd = new Date(budget.endDate);
    // 预算周期与筛选范围有交集即可
    return budgetStart <= endDate && budgetEnd >= startDate;
}

/**
 * 解析金额筛选字符串
 * 格式: 'filterType:value1' 或 'filterType:value1:value2'
 */
function parseAmountFilter(filter: string): { type: string; value1: number; value2?: number } | null {
    if (!filter) return null;
    const parts = filter.split(':');
    if (parts.length < 2) return null;

    const type = parts[0] || '';
    const value1Str = parts[1] || '';
    const value1 = parseFloat(value1Str);

    if (isNaN(value1)) return null;

    if (parts.length >= 3) {
        const value2Str = parts[2] || '';
        const value2 = parseFloat(value2Str);
        if (isNaN(value2)) return null;
        return { type, value1, value2 };
    }

    return { type, value1 };
}

/**
 * 匹配金额筛选条件
 */
function matchAmountFilter(amount: number, filter: { type: string; value1: number; value2?: number }): boolean {
    switch (filter.type) {
        case 'gt': // Greater than
            return amount > filter.value1;
        case 'lt': // Less than
            return amount < filter.value1;
        case 'eq': // Equal to
            return Math.abs(amount - filter.value1) < 0.01;
        case 'ne': // Not equal to
            return Math.abs(amount - filter.value1) >= 0.01;
        case 'bt': // Between
            return filter.value2 !== undefined && amount >= filter.value1 && amount <= filter.value2;
        case 'nb': // Not between
            return filter.value2 !== undefined && (amount < filter.value1 || amount > filter.value2);
        default:
            return true;
    }
}

/**
 * 获取金额筛选参数个数
 */
function getAmountFilterParameterCount(filterType: string): number {
    const filterTypeObj = AmountFilterType.valueOf(filterType);
    return filterTypeObj ? filterTypeObj.paramCount : 0;
}

/**
 * 设置周期筛选器
 */
function setPeriodFilter(filter: string): void {
    // 如果点击自定义范围，弹出日期选择对话框
    if (filter === 'custom') {
        showCustomDateDialog.value = true;
        return;
    }
    activePeriodFilter.value = filter;
    reload(false);
}

/**
 * 自定义日期范围变化处理
 */
function onCustomDateRangeChange(minUnixTime: number, maxUnixTime: number): void {
    // 更新时间戳
    customMinDatetime.value = minUnixTime;
    customMaxDatetime.value = maxUnixTime;

    // 转换为日期字符串（四位年-两位月-两位日）
    // 时间戳单位是秒，需要乘以 1000 转换为毫秒
    const minDate = new Date(minUnixTime * 1000);
    const maxDate = new Date(maxUnixTime * 1000);
    customStartDate.value = `${minDate.getFullYear()}-${String(minDate.getMonth() + 1).padStart(2, '0')}-${String(minDate.getDate()).padStart(2, '0')}`;
    customEndDate.value = `${maxDate.getFullYear()}-${String(maxDate.getMonth() + 1).padStart(2, '0')}-${String(maxDate.getDate()).padStart(2, '0')}`;

    // 设置为自定义模式并关闭对话框
    activePeriodFilter.value = 'custom';
    showCustomDateDialog.value = false;
    reload(false);
}

/**
 * 日期范围选择错误处理
 */
function onDateRangeError(message: string): void {
    snackbar.value?.showMessage(message);
}

/**
 * 获取账户筛选器显示名称
 */
function getAccountFilterDisplayName(): string {
    if (accountFilter.value.length === 0) {
        return tt('All Accounts');
    }
    if (accountFilter.value.length === 1) {
        const account = allAccounts.value.find(a => a.id === accountFilter.value[0]);
        return account ? account.name : tt('Selected Account');
    }
    return `${accountFilter.value.length} ${tt('Accounts')}`;
}

/**
 * 获取标签筛选器显示名称
 */
function getTagFilterDisplayName(): string {
    if (tagFilter.value.length === 0) {
        return tt('All Tags');
    }
    if (tagFilter.value.length === 1) {
        const tag = allTransactionTags.value.find(t => t.id === tagFilter.value[0]);
        return tag ? tag.name : tt('Selected Tag');
    }
    return `${tagFilter.value.length} ${tt('Tags')}`;
}

/**
 * 处理账户筛选变化
 */
function setAccountFilter(changed: boolean): void {
    showFilterAccountDialog.value = false;

    if (changed) {
        reload(false);
    }
}

/**
 * 处理标签筛选变化
 */
function setTagFilter(changed: boolean): void {
    showFilterTagDialog.value = false;

    if (changed) {
        reload(false);
    }
}

/**
 * 处理分类筛选对话框变化
 */
function onCategoryFilterDialogChange(changed: boolean): void {
    showFilterCategoryDialog.value = false;

    if (changed) {
        reload(false);
    }
}

/**
 * 设置关键词筛选
 */
function setKeywordFilter(keyword: string): void {
    searchKeyword.value = keyword;
}

// ============================================================================
// 已花费筛选相关函数
// ============================================================================

/**
 * 点击已花费筛选类型
 */
function onSpentFilterTypeClick(filterType: string): void {
    if (currentSpentFilterType.value === filterType) {
        currentSpentFilterType.value = '';
    } else {
        currentSpentFilterType.value = filterType;
    }
}

/**
 * 改变已花费筛选
 */
function changeSpentFilter(filterType: string): void {
    currentSpentFilterType.value = '';

    if (!filterType) {
        spentAmountFilter.value = '';
        return;
    }

    const amountCount = getAmountFilterParameterCount(filterType);
    if (!amountCount) return;

    let amountFilter = filterType;

    if (amountCount === 1) {
        amountFilter += ':' + currentSpentFilterValue1.value;
    } else if (amountCount === 2) {
        if (currentSpentFilterValue2.value < currentSpentFilterValue1.value) {
            snackbar.value?.showMessage(tt('Incorrect amount range'));
            return;
        }
        amountFilter += ':' + currentSpentFilterValue1.value + ':' + currentSpentFilterValue2.value;
    } else {
        return;
    }

    spentAmountFilter.value = amountFilter;
}

/**
 * 获取已花费筛选器的显示名称
 */
function getSpentFilterDisplayName(): string {
    if (!spentAmountFilter.value) return tt('Spent');

    const parsed = parseAmountFilter(spentAmountFilter.value);
    if (!parsed) return tt('Spent');

    const filterType = AmountFilterType.valueOf(parsed.type);
    if (!filterType) return tt('Spent');

    const typeName = tt(filterType.name);
    if (parsed.value2 !== undefined) {
        return `${typeName} ¥${parsed.value1}~¥${parsed.value2}`;
    }
    return `${typeName} ¥${parsed.value1}`;
}

/**
 * 获取已花费筛选器的简短标签（用于更多设置菜单）
 */
function getSpentFilterLabel(): string {
    if (!spentAmountFilter.value) return '';
    const parsed = parseAmountFilter(spentAmountFilter.value);
    if (!parsed) return '';
    const filterType = AmountFilterType.valueOf(parsed.type);
    if (!filterType) return '';
    if (parsed.value2 !== undefined) {
        return `¥${parsed.value1}~¥${parsed.value2}`;
    }
    return `${tt(filterType.name)} ¥${parsed.value1}`;
}

// ============================================================================
// 总预算筛选相关函数
// ============================================================================

/**
 * 点击总预算筛选类型
 */
function onBudgetFilterTypeClick(filterType: string): void {
    if (currentBudgetFilterType.value === filterType) {
        currentBudgetFilterType.value = '';
    } else {
        currentBudgetFilterType.value = filterType;
    }
}

/**
 * 改变总预算筛选
 */
function changeBudgetFilter(filterType: string): void {
    currentBudgetFilterType.value = '';

    if (!filterType) {
        budgetAmountFilter.value = '';
        return;
    }

    const amountCount = getAmountFilterParameterCount(filterType);
    if (!amountCount) return;

    let amountFilter = filterType;

    if (amountCount === 1) {
        amountFilter += ':' + currentBudgetFilterValue1.value;
    } else if (amountCount === 2) {
        if (currentBudgetFilterValue2.value < currentBudgetFilterValue1.value) {
            snackbar.value?.showMessage(tt('Incorrect amount range'));
            return;
        }
        amountFilter += ':' + currentBudgetFilterValue1.value + ':' + currentBudgetFilterValue2.value;
    } else {
        return;
    }

    budgetAmountFilter.value = amountFilter;
}

/**
 * 获取总预算筛选器的显示名称
 */
function getBudgetFilterDisplayName(): string {
    if (!budgetAmountFilter.value) return tt('Budget');

    const parsed = parseAmountFilter(budgetAmountFilter.value);
    if (!parsed) return tt('Budget');

    const filterType = AmountFilterType.valueOf(parsed.type);
    if (!filterType) return tt('Budget');

    const typeName = tt(filterType.name);
    if (parsed.value2 !== undefined) {
        return `${typeName} ¥${parsed.value1}~¥${parsed.value2}`;
    }
    return `${typeName} ¥${parsed.value1}`;
}

/**
 * 获取总预算筛选器的简短标签（用于更多设置菜单）
 */
function getBudgetFilterLabel(): string {
    if (!budgetAmountFilter.value) return '';
    const parsed = parseAmountFilter(budgetAmountFilter.value);
    if (!parsed) return '';
    const filterType = AmountFilterType.valueOf(parsed.type);
    if (!filterType) return '';
    if (parsed.value2 !== undefined) {
        return `¥${parsed.value1}~¥${parsed.value2}`;
    }
    return `${tt(filterType.name)} ¥${parsed.value1}`;
}

/**
 * 清除所有筛选器
 */
function clearAllFilters(): void {
    categoryFilter.value = null;
    executionRateFilter.value = null;
    spentAmountFilter.value = '';
    budgetAmountFilter.value = '';
    currentSpentFilterType.value = '';
    currentSpentFilterValue1.value = 0;
    currentSpentFilterValue2.value = 0;
    currentBudgetFilterType.value = '';
    currentBudgetFilterValue1.value = 0;
    currentBudgetFilterValue2.value = 0;
}

// ============================================================================
// 筛选预设相关函数
// ============================================================================

/**
 * 保存当前筛选为预设
 */
function savePreset(): void {
    if (!presetName.value.trim()) {
        snackbar.value?.showMessage(tt('Please enter a preset name'));
        return;
    }

    const preset: FilterPreset = {
        id: Date.now().toString(),
        name: presetName.value.trim(),
        categoryFilter: categoryFilter.value,
        accountFilter: [...accountFilter.value],
        tagFilter: [...tagFilter.value],
        executionRateFilter: executionRateFilter.value,
        spentAmountFilter: spentAmountFilter.value,
        budgetAmountFilter: budgetAmountFilter.value
    };

    filterPresets.value.push(preset);
    savePresetsToStorage();
    showSavePresetDialog.value = false;
    presetName.value = '';
    snackbar.value?.showMessage(tt('Preset saved successfully'));
}

/**
 * 加载筛选预设
 */
function loadPreset(preset: FilterPreset): void {
    categoryFilter.value = preset.categoryFilter;
    accountFilter.value = [...preset.accountFilter];
    tagFilter.value = [...preset.tagFilter];
    executionRateFilter.value = preset.executionRateFilter;
    spentAmountFilter.value = preset.spentAmountFilter;
    budgetAmountFilter.value = preset.budgetAmountFilter;
    snackbar.value?.showMessage(tt('Preset loaded'));
}

/**
 * 删除筛选预设
 */
function deletePreset(presetId: string): void {
    filterPresets.value = filterPresets.value.filter(p => p.id !== presetId);
    savePresetsToStorage();
    snackbar.value?.showMessage(tt('Preset deleted'));
}

/**
 * 获取分类筛选显示名称
 */
function getCategoryFilterDisplayName(): string {
    if (!categoryFilter.value) {
        return '';
    }

    const category = allCategoriesMap.value[categoryFilter.value];
    if (!category) {
        return categoryFilter.value;
    }

    // 如果是子分类，显示"父分类 > 子分类"
    if (category.parentId) {
        const parentCategory = allCategoriesMap.value[category.parentId];
        if (parentCategory) {
            return `${parentCategory.name} > ${category.name}`;
        }
    }

    return category.name;
}

/**
 * 设置执行度筛选
 */
function setExecutionRateFilter(option: RangeFilter | null): void {
    executionRateFilter.value = option;
}

/**
 * 获取执行率颜色
 */
function getExecutionRateColor(rate: number): string {
    if (rate >= 100) return 'error';
    if (rate >= 80) return 'warning';
    if (rate >= 50) return 'info';
    return 'success';
}

/**
 * 获取预算进度条颜色（优先使用分类颜色，否则使用执行率颜色）
 */
function getBudgetProgressColor(budget: Budget): string {
    // 优先使用分类颜色
    if (budget.categoryColor) {
        // 将十六进制颜色转换为样式颜色值（添加 # 前缀）
        return `#${budget.categoryColor}`;
    }
    // 降级使用执行率颜色
    return getExecutionRateColor(budget.executionRate);
}

/**
 * 获取分组进度条颜色（用于一级分类行）
 */
function getGroupProgressColor(group: BudgetGroup): string {
    // 优先使用分类颜色
    if (group.categoryColor) {
        return `#${group.categoryColor}`;
    }
    // 如果有一级分类预算，使用其分类颜色
    const primaryBudget = getPrimaryBudgetForHeader(group);
    if (primaryBudget && primaryBudget.categoryColor) {
        return `#${primaryBudget.categoryColor}`;
    }
    // 降级使用执行率颜色
    return getExecutionRateColor(getGroupExecutionRate(group));
}

/**
 * 获取执行率文本样式类
 */
function getExecutionRateTextClass(rate: number): string {
    // 使用黑色显示百分比，超支时显示红色
    if (rate >= 100) return 'text-error';
    return 'text-default';  // 黑色/默认色
}

/**
 * 获取执行率颜色类（用于汇总行，按阈值着色）
 * 50以下绿色，50-80黄色，80-100橙色，100以上红色
 */
function getExecutionRateColorClass(rate: number): string {
    if (rate >= 100) return 'text-error';          // 红色 - 超支
    if (rate >= 80) return 'text-warning';         // 橙色 - 接近超支
    if (rate >= 50) return 'text-orange';          // 黄色 - 中等使用
    return 'text-success';                         // 绿色 - 低使用
}

/**
 * 获取分组执行率（一级分类汇总）
 *
 * 计算规则：
 * 1. 如果有一级分类预算且有已花费金额，使用一级分类的执行率
 * 2. 如果有一级分类预算但没有已花费，则按“总已花费 / 一级预算金额”计算
 * 3. 如果没有一级分类预算，则按“总已花费 / 总预算金额”计算（二级分类之和）
 */
function getGroupExecutionRate(group: BudgetGroup): number {
    // 如果有一级分类预算
    const primaryBudget = getPrimaryBudgetForHeader(group);
    if (group.primaryBudgets.length > 1 && group.totalAmount > 0) {
        return (group.totalSpent / group.totalAmount) * 100;
    }
    if (primaryBudget) {
        // 如果一级分类预算有执行率（后端已计算），使用它
        if (group.primaryBudgets.length === 1 && primaryBudget.executionRate > 0) {
            return primaryBudget.executionRate;
        }
        // 否则按“总已花费 / 一级预算金额”计算
        if (group.primaryAmount > 0) {
            return (group.totalSpent / group.primaryAmount) * 100;
        }
    }

    // 没有一级分类预算，使用二级分类汇总计算
    if (group.totalAmount > 0) {
        return (group.totalSpent / group.totalAmount) * 100;
    }

    return 0;
}

/**
 * 获取分组执行率文本
 */
function getGroupExecutionRateText(group: BudgetGroup): string {
    const rate = getGroupExecutionRate(group);
    return rate.toFixed(1) + '%';
}

/**
 * 获取预测置信等级文案键
 */
function getForecastConfidenceLabel(confidence?: 'high' | 'medium' | 'low' | null): string {
    switch (confidence) {
        case 'high':
            return 'High Confidence';
        case 'medium':
            return 'Medium Confidence';
        default:
            return 'Low Confidence';
    }
}

/**
 * 获取预测置信等级颜色
 */
function getForecastConfidenceClass(confidence?: 'high' | 'medium' | 'low' | null): string {
    switch (confidence) {
        case 'high':
            return 'text-success';
        case 'medium':
            return 'text-warning';
        default:
            return 'text-error';
    }
}

/**
 * 格式化金额
 */
function formatAmount(amount: number): string {
    return '¥' + amount.toFixed(2);
}

/**
 * 点击进度条跳转到对应分类和时间的账单列表
 * @param category 主分类名称
 * @param subCategory 子分类名称（可选）
 * @param budget 预算对象（用于获取日期范围）
 */
function navigateToTransactions(category: string, subCategory: string | null, budget: Budget | null): void {
    // 构建查询参数
    const query: Record<string, string> = {};

    // 设置分类筛选
    if (category) {
        query['category'] = category;
        if (subCategory) {
            query['subCategory'] = subCategory;
        }
    }

    // 设置日期范围（从预算对象获取）
    if (budget) {
        if (budget.startDate) {
            query['startDate'] = budget.startDate;
        }
        if (budget.endDate) {
            query['endDate'] = budget.endDate;
        }
    } else {
        // 如果没有预算对象，使用当前选择的周期
        const now = new Date();
        const currentYear = now.getFullYear();
        const currentMonth = now.getMonth();

        switch (activePeriodFilter.value) {
            case 'thisMonth':
                query['startDate'] = `${currentYear}-${String(currentMonth + 1).padStart(2, '0')}-01`;
                query['endDate'] = `${currentYear}-${String(currentMonth + 1).padStart(2, '0')}-${new Date(currentYear, currentMonth + 1, 0).getDate()}`;
                break;
            case 'lastMonth': {
                const lastMonthDate = new Date(currentYear, currentMonth - 1, 1);
                query['startDate'] = `${lastMonthDate.getFullYear()}-${String(lastMonthDate.getMonth() + 1).padStart(2, '0')}-01`;
                query['endDate'] = `${lastMonthDate.getFullYear()}-${String(lastMonthDate.getMonth() + 1).padStart(2, '0')}-${new Date(lastMonthDate.getFullYear(), lastMonthDate.getMonth() + 1, 0).getDate()}`;
                break;
            }
            case 'thisYear':
                query['startDate'] = `${currentYear}-01-01`;
                query['endDate'] = `${currentYear}-12-31`;
                break;
            case 'custom':
                if (customStartDate.value) query['startDate'] = customStartDate.value;
                if (customEndDate.value) query['endDate'] = customEndDate.value;
                break;
        }
    }

    // 设置交易类型（支出或投资）
    query['type'] = activeBudgetType.value === BudgetType.Expense ? '3' : '5';

    console.log(`[Budget] 跳转到账单列表: category=${category}, subCategory=${subCategory}, query=`, query);

    // 跳转到账单列表页面
    router.push({
        path: '/transaction/list',
        query: query
    });
}

/**
 * 切换视图模式
 */
function switchViewMode(mode: unknown): void {
    activeViewMode.value = mode as BudgetViewMode;
    if (mode === 'forecast') {
        if (!['thisMonth', 'thisQuarter', 'thisYear'].includes(activePeriodFilter.value)) {
            activePeriodFilter.value = 'thisMonth';
        }
        loadForecast();
    } else {
        reload(false);
        if (mode === 'history') {
            scheduleHistoricalChartResize();
        }
    }
}

/**
 * 切换预算类型
 */
function switchBudgetType(type: unknown): void {
    activeBudgetType.value = type as BudgetType;
    reload(false);
}

/**
 * 获取当前周期类型
 */
function getCurrentPeriodType(): BudgetPeriodType {
    if (['thisMonth', 'lastMonth'].includes(activePeriodFilter.value)) {
        return BudgetPeriodType.Monthly;
    } else if (['thisQuarter', 'lastQuarter'].includes(activePeriodFilter.value)) {
        return BudgetPeriodType.Quarterly;
    } else if (['thisYear', 'lastYear'].includes(activePeriodFilter.value)) {
        return BudgetPeriodType.Yearly;
    }
    return BudgetPeriodType.Monthly;
}

function formatDateOnly(date: Date): string {
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

function getCurrentPeriodRequest(): BudgetHistoryRequest {
    const now = new Date();
    const currentYear = now.getFullYear();
    const currentMonth = now.getMonth();
    const currentQuarter = Math.floor(currentMonth / 3) + 1;

    switch (activePeriodFilter.value) {
        case 'thisMonth':
            return {
                periodType: BudgetPeriodType.Monthly,
                year: currentYear,
                month: currentMonth + 1
            };
        case 'lastMonth': {
            const target = new Date(currentYear, currentMonth - 1, 1);
            return {
                periodType: BudgetPeriodType.Monthly,
                year: target.getFullYear(),
                month: target.getMonth() + 1
            };
        }
        case 'thisQuarter':
            return {
                periodType: BudgetPeriodType.Quarterly,
                year: currentYear,
                quarter: currentQuarter
            };
        case 'lastQuarter': {
            const target = new Date(currentYear, currentMonth - 3, 1);
            return {
                periodType: BudgetPeriodType.Quarterly,
                year: target.getFullYear(),
                quarter: Math.floor(target.getMonth() / 3) + 1
            };
        }
        case 'thisYear':
            return {
                periodType: BudgetPeriodType.Yearly,
                year: currentYear
            };
        case 'lastYear':
            return {
                periodType: BudgetPeriodType.Yearly,
                year: currentYear - 1
            };
        case 'custom':
            return {
                periodType: getCurrentPeriodType(),
                startDate: customStartDate.value || undefined,
                endDate: customEndDate.value || undefined
            };
        default:
            return {
                periodType: getCurrentPeriodType()
            };
    }
}

/**
 * 加载预算列表和执行数据
 */
async function reload(force: boolean): Promise<void> {
    const requestId = ++reloadRequestId.value;
    loading.value = true;

    try {
        if (activeViewMode.value === 'history') {
            await loadHistoricalBudgetView(requestId);
            return;
        }

        const periodRequest = getCurrentPeriodRequest();

        await budgetStore.loadAllBudgets({
            force,
            type: activeBudgetType.value,
            periodType: periodRequest.periodType
        });
        await budgetStore.loadBudgetExecution({
            type: activeBudgetType.value,
            periodType: periodRequest.periodType,
            year: periodRequest.year,
            month: periodRequest.month,
            quarter: periodRequest.quarter,
            startDate: periodRequest.startDate,
            endDate: periodRequest.endDate
        });

        if (requestId !== reloadRequestId.value) {
            return;
        }

        loading.value = false;

        if (activePeriodFilter.value === 'expired') {
            void loadBudgetHistoryForExpired(requestId);
        }

        if (activeViewMode.value === 'forecast') {
            void loadForecast();
        }
    } catch (error: unknown) {
        const err = error as { isUpToDate?: boolean; message?: string };
        if (!err.isUpToDate) {
            snackbar.value?.showError(err.message || tt('Failed to load budgets'));
        }
    } finally {
        if (requestId === reloadRequestId.value) {
            loading.value = false;
        }
    }
}

/**
 * 加载周期预计
 */
async function loadForecast(): Promise<void> {
    try {
        const periodRequest = getCurrentPeriodRequest();

        await budgetStore.loadBudgetForecast(
            buildBudgetForecastLoadRequest({
                budgetType: activeBudgetType.value,
                periodRequest,
                monthsHistory: forecastMonthsHistory.value,
                forecastStrategy: forecastStrategy.value
            })
        );
    } catch (error: unknown) {
        const err = error as { message?: string };
        snackbar.value?.showError(err.message || tt('Failed to load forecast'));
    }
}

/**
 * 添加预算
 */
function add(): void {
    const newBudget = Budget.createNew(activeBudgetType.value);
    newBudget.periodType = getCurrentPeriodType();
    editDialog.value?.open({ budget: newBudget, type: activeBudgetType.value });
}

/**
 * 添加一级分类预算（为已有子分类的分类组添加一级分类预算）
 */
function addPrimaryBudget(group: BudgetGroup): void {
    const newBudget = Budget.createNew(activeBudgetType.value);
    newBudget.periodType = getCurrentPeriodType();
    newBudget.category = group.category;
    newBudget.subCategory = '';  // 一级分类预算的子分类为空
    newBudget.amount = group.totalAmount;  // 初始金额设为当前汇总金额
    newBudget.categoryIcon = group.categoryIcon;
    newBudget.categoryColor = group.categoryColor;
    editDialog.value?.open({ budget: newBudget, type: activeBudgetType.value, usePrimaryCategoryOnly: true });
}

/**
 * 编辑预算
 */
function edit(budget: Budget): void {
    editDialog.value?.open({ budget, type: activeBudgetType.value });
}

/**
 * 删除预算
 */
function remove(budget: Budget): void {
    const budgetName = budget.name || budget.fullCategoryName;
    confirmDialog.value?.open(
        tt('Delete Budget'),
        tt('Are you sure you want to delete this budget "{name}"?', { name: budgetName }),
        { color: 'warning' }  // 橙色，与其他删除弹窗一致
    ).then(result => {
        if (result) {
            budgetRemoving.value[budget.id] = true;

            budgetStore.deleteBudget({ budgetId: budget.id }).then(() => {
                snackbar.value?.showMessage(tt('Budget deleted successfully'));
            }).catch((error: unknown) => {
                const err = error as { message?: string };
                snackbar.value?.showError(err.message || tt('Failed to delete budget'));
            }).finally(() => {
                budgetRemoving.value[budget.id] = false;
            });
        }
    });
}

/**
 * 预算保存回调
 */
function onBudgetSaved(): void {
    reload(true);
}

/**
 * 导出预算
 */
async function exportBudgets(): Promise<void> {
    try {
        const result = await budgetStore.exportBudgets();

        // 下载导出的预算文件
        const dataStr = JSON.stringify(result.budgets, null, 2);
        const blob = new Blob([dataStr], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.href = url;
        link.download = `budgets_export_${new Date().toISOString().split('T')[0]}.json`;
        link.click();
        URL.revokeObjectURL(url);

        snackbar.value?.showMessage(tt('Budgets exported successfully'));
    } catch (error: unknown) {
        const err = error as { message?: string };
        snackbar.value?.showError(err.message || tt('Failed to export budgets'));
    }
}

/**
 * 导入预算
 */
function importBudgets(): void {
    fileInput.value?.click();
}

/**
 * 文件选择处理
 */
async function onFileSelected(event: Event): Promise<void> {
    const target = event.target as HTMLInputElement;
    const file = target.files?.[0];

    if (!file) return;

    try {
        const text = await file.text();
        const data = JSON.parse(text);

        // 验证数据格式
        if (!Array.isArray(data)) {
            throw new Error('Invalid file format');
        }

        // 确认导入
        confirmDialog.value?.open(
            tt('Import Budgets'),
            tt('Are you sure you want to import {count} budgets?', { count: data.length })
        ).then(async (result) => {
            if (result) {
                try {
                    const importResult = await budgetStore.importBudgets({ budgets: data, overwriteExisting: true });
                    snackbar.value?.showMessage(
                        tt('Imported {imported} budgets, updated {updated} budgets', {
                            imported: importResult.importedCount,
                            updated: importResult.updatedCount
                        })
                    );
                    reload(true);
                } catch (error: unknown) {
                    const err = error as { message?: string };
                    snackbar.value?.showError(err.message || tt('Failed to import budgets'));
                }
            }
        });
    } catch {
        snackbar.value?.showError(tt('Failed to parse file'));
    } finally {
        // 重置文件输入
        target.value = '';
    }
}

// ============================================================================
// 生命周期
// ============================================================================

onMounted(() => {
    // 应用初始参数
    if (props.initType) {
        activeBudgetType.value = parseInt(props.initType) as BudgetType;
    }
    if (props.initPeriodType) {
        // 映射旧的周期类型到新的筛选器
        const periodMap: Record<string, string> = {
            [BudgetPeriodType.Monthly]: 'thisMonth',
            [BudgetPeriodType.Quarterly]: 'thisQuarter',
            [BudgetPeriodType.Yearly]: 'thisYear'
        };
        activePeriodFilter.value = periodMap[props.initPeriodType] || 'thisMonth';
    }
    if (props.initViewMode) {
        if (isBudgetViewMode(props.initViewMode)) {
            activeViewMode.value = props.initViewMode;
        }
    }

    // 加载筛选预设
    loadPresetsFromStorage();

    ensureHistoricalDateRangeInitialized();

    // 初始加载
    reload(false);
});

watch([forecastStrategy, forecastMonthsHistory], () => {
    if (activeViewMode.value === 'forecast') {
        loadForecast();
    }
});

// 监听筛选关键词变化
watch(filterKeyword, (newVal) => {
    // 实时搜索
    searchKeyword.value = newVal;
});
</script>

<style scoped>
.forecast-table .text-sm {
    font-size: 0.875rem;
}

.budget-keyword-filter {
    min-width: 200px;
    max-width: 300px;
    flex: 0 1 300px;
}

.budget-right-tools {
    flex: 0 0 auto;
    margin-left: auto;
}

.budget-forecast-title-actions {
    flex: 0 0 auto;
}

.budget-forecast-title-button {
    flex: 0 0 112px;
    width: 112px;
    max-width: 112px;
    height: 38px;
    min-height: 38px;
    padding-inline: 10px;
    text-transform: none;
    letter-spacing: normal;
    white-space: nowrap;
}

.budget-forecast-settings-content {
    display: flex;
    flex-direction: column;
    gap: 14px;
}

.budget-forecast-setting-control {
    width: 100%;
}

.budget-forecast-setting-control :deep(.v-field) {
    min-height: 48px;
}

.budget-forecast-summary-row {
    column-gap: 28px;
    row-gap: 8px;
}

.tab-text-truncate {
    justify-content: flex-start;
    padding-inline: 12px;
}

.tab-text-truncate .text-truncate {
    width: 100%;
    text-align: left;
}

.cursor-pointer {
    cursor: pointer;
}

/* 表头筛选菜单样式 */
.budget-table th {
    position: relative;
}

.list-item-selected {
    background-color: rgba(var(--v-theme-primary), 0.1);
}

/* 导航栏按钮宽度统一（适应默认宽度256px） */
.budget-nav-buttons {
    width: 100%;
}

/* 使用深层穿透选择器作用到子组件 */
.budget-nav-buttons:deep(.v-btn) {
    width: 100%;
}

.budget-level-tabs :deep(.v-tab) {
    justify-content: flex-start;
}

/* 水平按钮组样式 - 总宽度100%，每个按钮各占一半 */
.budget-nav-buttons.btn-horizontal-group {
    width: 100%;
    display: flex;
}

.budget-nav-buttons.btn-horizontal-group:deep(.v-btn) {
    flex: 1 1 0;
    min-width: 0;
    max-width: 50%;
}

/* 预算列表项样式 - 类似统计分析的条形图样式 */
.budget-list-row {
    cursor: pointer;
    transition: background-color 0.2s ease;
}

.budget-list-row:hover {
    background-color: rgba(var(--v-theme-primary), 0.04);
}

.budget-list-row:focus {
    outline: none;
    background-color: rgba(var(--v-theme-primary), 0.08);
}

.budget-item {
    width: 100%;
}

.budget-category-name {
    color: rgba(var(--v-theme-on-surface), 0.87);
}

.budget-percent {
    font-size: 0.875rem;
}

.budget-amounts {
    flex-shrink: 0;
    font-family: 'Roboto Mono', monospace;
}

.budget-spent {
    color: rgba(var(--v-theme-on-surface), 0.87);
}

.budget-progress-container {
    width: 100%;
}

.budget-progress-container :deep(.v-progress-linear) {
    transition: all 0.3s ease;
}

/* 操作按钮悬浮显示 */
.budget-list-row:hover .budget-row-actions,
.budget-list-row:focus .budget-row-actions {
    opacity: 1;
}

.budget-row-actions {
    opacity: 0;
    transition: opacity 0.2s ease;
}

/* 预算表格背景，自适应深色/浅色主题 */
.budget-table {
    background-color: rgb(var(--v-theme-surface)) !important;
}

.budget-table:deep(tbody tr) {
    background-color: transparent !important;
}

.budget-table:deep(tbody tr td) {
    border-bottom: none !important;
}

/* 一级分类预算背景 - 使用主题变量 */
.budget-primary {
    background-color: rgb(var(--v-theme-surface)) !important;
}

/* 二级分类背景 - 使用主题变量 */
.budget-secondary {
    background-color: rgb(var(--v-theme-surface)) !important;
}

/* 分组间分隔线（在最后一行底部添加左右不封完的边框） */
.budget-group-last-row td {
    position: relative;
}
.budget-group-last-row td::after {
    content: '';
    position: absolute;
    left: 32px;
    right: 32px;
    bottom: 0;
    height: 1px;
    background-color: rgba(var(--v-theme-on-surface), 0.15);
}

/* 折叠/展开图标动画 */
.toggle-icon {
    transition: transform 0.3s ease-in-out;
}

/* 金额数字使用等宽字体 */
.budget-amounts {
    flex-shrink: 0;
    font-family: 'Roboto Mono', 'Consolas', monospace;
    text-align: right;
}

/* 筛选菜单固定宽度 */
.budget-filter-menu {
    width: 320px;
    min-width: 320px;
    max-width: 320px;
}

/* 汇总行样式（参考统计分析页面） */
.budget-summary-row {
    background-color: transparent;
}

.budget-history-chart {
    width: 100%;
    height: 520px;
}

.budget-history-chart-shell {
    position: relative;
}

.budget-history-legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 10px;
}

.budget-history-legend-group {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    max-width: 100%;
}

.budget-history-legend-secondary-list {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 3px;
}

.budget-history-legend-item {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
    border-radius: 999px;
    background-color: rgba(var(--v-theme-surface), 0.82);
    color: rgba(var(--v-theme-on-surface), 0.88);
    cursor: pointer;
    padding: 3px 8px;
    font-size: 0.78rem;
    line-height: 1.1;
    transition: all 0.18s ease;
}

.budget-history-legend-item:hover {
    border-color: rgba(var(--v-theme-primary), 0.32);
    transform: translateY(-1px);
}

.budget-history-legend-item--primary {
    align-self: flex-start;
    font-weight: 600;
    background-color: rgba(var(--v-theme-primary), 0.07);
}

.budget-history-legend-item--secondary {
    padding: 2px 7px;
    font-size: 0.74rem;
    border-color: rgba(var(--v-theme-on-surface), 0.09);
}

.budget-history-legend-item--primary.is-partial {
    border-style: dashed;
}

.budget-history-legend-item.is-inactive {
    opacity: 0.46;
}

.budget-history-legend-swatch {
    width: 7px;
    height: 7px;
    border-radius: 999px;
    flex-shrink: 0;
}

.budget-history-legend-label {
    line-height: 1.2;
}

.budget-history-legend-count {
    color: rgba(var(--v-theme-on-surface), 0.6);
    font-size: 0.75rem;
}

.budget-history-loading-placeholder {
    min-height: 440px;
}

.budget-history-loading-bar {
    max-width: 480px;
}

.budget-history-detail-chart {
    width: 100%;
    height: 360px;
}

.budget-history-panel {
    background-color: transparent;
}

.budget-history-aggregation-select {
    min-width: 190px;
    max-width: 190px;
}

.budget-summary-label {
    font-size: 1rem;
    color: rgba(var(--v-theme-on-surface), 0.7);
}

.budget-summary-amount {
    font-size: 1.5rem;
    font-weight: bold;
    overflow: hidden;
    text-overflow: ellipsis;
}

/* 执行率颜色 - 橙黄色(50-80%) */
.text-orange {
    color: rgb(226, 182, 10) !important;
}
</style>
