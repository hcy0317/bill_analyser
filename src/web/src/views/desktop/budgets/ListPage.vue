<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-layout>
                    <!-- 左侧导航抽屉 -->
                    <v-navigation-drawer :permanent="alwaysShowNav" v-model="showNav">
                        <div class="mx-4 my-4">
                            <!-- 视图切换：预算管理 / 周期预计（上下排列）-->
                            <btn-vertical-group class="mb-4 budget-nav-buttons" :disabled="loading" :buttons="[
                                { name: tt('Budget Management'), value: 'budget' },
                                { name: tt('Period Forecast'), value: 'forecast' }
                            ]" v-model="activeViewMode" @update:model-value="switchViewMode" />
                        </div>
                        <v-divider />
                        <div class="mx-4 mt-4">
                            <!-- 类型切换：支出 / 投资（横向排列，宽度与上方按钮一致）-->
                            <btn-horizontal-group class="budget-nav-buttons" :disabled="loading" :buttons="[
                                { name: tt('Expense'), value: BudgetType.Expense },
                                { name: tt('Investment'), value: BudgetType.Investment }
                            ]" v-model="activeBudgetType" @update:model-value="switchBudgetType" />
                        </div>
                        <!-- 时间筛选列表（类似交易列表的月份选择）-->
                        <v-tabs show-arrows class="my-4" direction="vertical"
                                :disabled="loading" v-model="activePeriodFilterIndex">
                            <v-tab class="tab-text-truncate" :key="idx" :value="idx"
                                   v-for="(filter, idx) in allPeriodFilters"
                                   @click="setPeriodFilter(filter.value)">
                                <span class="text-truncate">{{ filter.name }}</span>
                            </v-tab>
                        </v-tabs>
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
                                    <span>{{ activeViewMode === 'budget' ? tt('Budget Management') : tt('Period Forecast') }}</span>
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
                                    <v-btn density="compact" color="default" variant="text" size="24"
                                           class="ms-2" :icon="true" :loading="loading || updating" @click="reload(true)">
                                        <template #loader>
                                            <v-progress-circular indeterminate size="20"/>
                                        </template>
                                        <v-icon :icon="mdiRefresh" size="24" />
                                        <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                                    </v-btn>

                                    <v-spacer/>

                                    <!-- 搜索框 -->
                                    <div class="budget-keyword-filter ms-2">
                                        <v-text-field density="compact" :disabled="loading"
                                                      :prepend-inner-icon="mdiMagnify"
                                                      :append-inner-icon="filterKeyword !== searchKeyword ? mdiCheck : undefined"
                                                      :placeholder="tt('Filter budget description')"
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
                            </template>

                            <!-- 汇总信息行（在工具栏下方，参考统计分析页面的样式） -->
                            <v-card-text v-if="currentExecution && activeViewMode === 'budget'" class="py-3 border-b budget-summary-row">
                                <div class="d-flex align-center flex-wrap ga-6">
                                    <div class="d-flex align-center">
                                        <span class="budget-summary-label me-2">{{ tt('Total Budget') }}:</span>
                                        <span class="budget-summary-amount text-expense">{{ formatAmount(currentExecution.totalBudget / 100) }}</span>
                                    </div>
                                    <div class="d-flex align-center">
                                        <span class="budget-summary-label me-2">{{ tt('Total Spent') }}:</span>
                                        <span class="budget-summary-amount text-income">{{ formatAmount(currentExecution.totalSpent / 100) }}</span>
                                    </div>
                                    <div class="d-flex align-center">
                                        <span class="budget-summary-label me-2">{{ tt('Overall Execution Rate') }}:</span>
                                        <span class="budget-summary-amount" :class="getExecutionRateColorClass(currentExecution.totalExecutionRate)">
                                            {{ currentExecution.totalExecutionRate.toFixed(1) }}%
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
                                    :class="{ 'budget-group-last-row': gIdx < groupedBudgets.length - 1 && (group.isCollapsed || group.subBudgets.length === 0) }"
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
                                                    <!-- 一级分类预算操作按钮（hover时显示） -->
                                                    <div class="budget-row-actions d-flex align-center">
                                                        <!-- 如果没有一级分类预算，显示添加按钮 -->
                                                        <v-btn v-if="!group.primaryBudget"
                                                               density="compact" color="default" variant="text" size="x-small"
                                                               :icon="mdiPlusCircleOutline"
                                                               :disabled="loading || updating"
                                                               @click.stop="addPrimaryBudget(group)">
                                                            <v-icon :icon="mdiPlusCircleOutline" size="16" />
                                                            <v-tooltip activator="parent">{{ tt('Add Primary Budget') }}</v-tooltip>
                                                        </v-btn>
                                                        <!-- 如果有一级分类预算，显示编辑和删除按钮 -->
                                                        <v-btn v-if="group.primaryBudget"
                                                               density="compact" color="default" variant="text" size="x-small"
                                                               :icon="mdiPencilOutline"
                                                               :disabled="loading || updating"
                                                               @click.stop="edit(group.primaryBudget)">
                                                            <v-icon :icon="mdiPencilOutline" size="16" />
                                                            <v-tooltip activator="parent">{{ tt('Edit') }}</v-tooltip>
                                                        </v-btn>
                                                        <v-btn v-if="group.primaryBudget"
                                                               density="compact" color="default" variant="text" size="x-small"
                                                               :icon="mdiDeleteOutline"
                                                               :loading="budgetRemoving[group.primaryBudget.id]"
                                                               :disabled="loading || updating"
                                                               @click.stop="remove(group.primaryBudget)">
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
                                                     @click.stop="navigateToTransactions(group.category, null, group.primaryBudget)"
                                                     :title="tt('Click to view transactions')">
                                                    <v-progress-linear
                                                        :model-value="Math.min(getGroupExecutionRate(group), 100)"
                                                        :color="getGroupProgressColor(group)"
                                                        :bg-color="isDarkMode ? '#444444' : '#f0f0f0'"
                                                        :bg-opacity="1"
                                                        :height="6"
                                                        :rounded="false"
                                                    />
                                                </div>
                                            </div>
                                        </div>
                                    </td>
                                </tr>

                                <!-- 二级分类预算列表（展开时显示） -->
                                <template v-if="!group.isCollapsed">
                                    <tr v-for="(budget, bIdx) in group.subBudgets" :key="budget.id"
                                        class="budget-list-row budget-sub-row"
                                        :class="{ 'budget-group-last-row': gIdx < groupedBudgets.length - 1 && bIdx === group.subBudgets.length - 1 }"
                                        @dblclick="edit(budget)" tabindex="0"
                                        @keydown.delete="remove(budget)" @keydown.enter="edit(budget)">
                                        <td colspan="4" class="pa-0">
                                            <div class="budget-item budget-secondary d-flex px-4 py-2"
                                                 style="padding-left: 56px !important;">
                                                <!-- 子分类图标（放大，与文字+进度条等高） -->
                                                <item-icon
                                                    v-if="budget.categoryIcon"
                                                    class="me-3 flex-shrink-0"
                                                    icon-type="category"
                                                    :icon-id="budget.categoryIcon"
                                                    :color="budget.categoryColor"
                                                    :size="28"
                                                />
                                                <!-- 右侧内容区域 -->
                                                <div class="d-flex flex-column flex-grow-1">
                                                    <!-- 第一行：子分类名称 + 执行度 + 操作按钮 + 金额 -->
                                                    <div class="d-flex align-center justify-space-between mb-1">
                                                        <div class="d-flex align-center flex-grow-1">
                                                            <span class="budget-category-name text-body-2">
                                                                {{ budget.subCategory }}
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
                                                        <!-- 操作按钮 -->
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
                                                        <!-- 金额显示（左对齐在最右边） -->
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
                                                    <!-- 子分类进度条（与子分类名称左对齐，点击跳转到账单列表） -->
                                                    <div class="budget-progress-container cursor-pointer"
                                                         @click.stop="navigateToTransactions(budget.category, budget.subCategory, budget)"
                                                         :title="tt('Click to view transactions')">
                                                        <v-progress-linear
                                                            :model-value="Math.min(budget.executionRate, 100)"
                                                            :color="getBudgetProgressColor(budget)"
                                                            :bg-color="isDarkMode ? '#444444' : '#f0f0f0'"
                                                            :bg-opacity="1"
                                                            :height="4"
                                                            :rounded="false"
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
                                    <div class="d-flex align-center justify-center py-8">
                                        <span class="text-grey">{{ tt('No forecast data') }}</span>
                                    </div>
                                </td>
                            </tr>
                            </tbody>

                            <tbody v-if="currentForecast && currentForecast.forecasts.length > 0">
                            <tr v-for="forecast in currentForecast.forecasts" :key="forecast.categoryId" class="text-sm">
                                <td>{{ forecast.categoryName }}</td>
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
                                    <v-chip v-if="forecast.projectedOverBudget" color="error" size="small">
                                        {{ tt('Over Budget') }}
                                    </v-chip>
                                    <v-chip v-else color="success" size="small">
                                        {{ tt('On Track') }}
                                    </v-chip>
                                </td>
                            </tr>
                            </tbody>
                        </v-table>

                        <!-- 周期信息 -->
                        <v-card-text v-if="currentForecast" class="border-t">
                            <div class="d-flex justify-space-between align-center flex-wrap ga-3">
                                <div>
                                    <span class="text-subtitle-2">{{ tt('Period') }}: </span>
                                    <span>{{ currentForecast.periodStart }} - {{ currentForecast.periodEnd }}</span>
                                </div>
                                <div>
                                    <span class="text-subtitle-2">{{ tt('Days Elapsed') }}: </span>
                                    <span>{{ currentForecast.daysElapsed }}</span>
                                </div>
                                <div>
                                    <span class="text-subtitle-2">{{ tt('Days Remaining') }}: </span>
                                    <span>{{ currentForecast.daysRemaining }}</span>
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

    <!-- 自定义日期范围对话框 - 使用DateRangeSelectionDialog组件 -->
    <date-range-selection-dialog
        :title="tt('Select Custom Date Range')"
        :min-time="customMinDatetime"
        :max-time="customMaxDatetime"
        v-model:show="showCustomDateDialog"
        @dateRange:change="onCustomDateRangeChange"
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

import { ref, computed, useTemplateRef, watch, onMounted } from 'vue';
import { useDisplay, useTheme } from 'vuetify';
import { useRouter } from 'vue-router';

import { useI18n } from '@/locales/helpers.ts';
import { useSettingsStore } from '@/stores/setting.ts';
import { useBudgetStore } from '@/stores/budget.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { CategoryType } from '@/core/category.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';
import { AmountFilterType } from '@/core/numeral.ts';
import { ThemeType } from '@/core/theme.ts';
import { getCurrentUnixTime, getTodayFirstUnixTime } from '@/lib/datetime.ts';

import {
    Budget,
    BudgetType,
    BudgetPeriodType,
    type BudgetExecutionResponse,
    type BudgetForecastResponse
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
    mdiInformationOutline
} from '@mdi/js';

// ============================================================================
// 类型定义
// ============================================================================

type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;
type EditDialogType = InstanceType<typeof EditDialog>;

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

// ============================================================================
// Props
// ============================================================================

const props = defineProps<{
    initType?: string;
    initPeriodType?: string;
    initViewMode?: string;
}>();

// ============================================================================
// 组件引用
// ============================================================================

const { tt } = useI18n();
const display = useDisplay();
const theme = useTheme();
const router = useRouter();
const budgetStore = useBudgetStore();
const transactionCategoriesStore = useTransactionCategoriesStore();
const accountsStore = useAccountsStore();
const transactionTagsStore = useTransactionTagsStore();
const settingsStore = useSettingsStore();

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

// ============================================================================
// 响应式状态
// ============================================================================

const loading = ref<boolean>(false);
const updating = ref<boolean>(false);
const searchKeyword = ref<string>('');
const filterKeyword = ref<string>('');

// 视图模式：'budget' 或 'forecast'
const activeViewMode = ref<string>('budget');

// 预算类型：支出或投资
const activeBudgetType = ref<BudgetType>(BudgetType.Expense);

// 周期筛选器
const activePeriodFilter = ref<string>('thisMonth');

// 自定义日期范围 - 使用DateRangeSelectionDialog组件
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

// 从localStorage加载预设
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

// 保存预设到localStorage
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

// 当前选中的周期筛选索引（用于 v-tabs）
const activePeriodFilterIndex = computed<number>(() => {
    return allPeriodFilters.value.findIndex(f => f.value === activePeriodFilter.value);
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
    primaryBudget: Budget | null;  // 一级分类预算（可能不存在）
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
                primaryBudget: null,
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
            // 一级分类预算
            group.primaryBudget = budget;
            group.primaryAmount = budget.amount;
            group.primarySpent = budget.spentAmount;
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
        if (group.primaryBudget) {
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
const forecastLoading = computed<boolean>(() => budgetStore.forecastLoading);

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
    // 更新Unix时间戳
    customMinDatetime.value = minUnixTime;
    customMaxDatetime.value = maxUnixTime;

    // 转换为日期字符串 (YYYY-MM-DD)
    // Unix时间戳是秒，需要乘以1000转换为毫秒
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
        // 将hex颜色转换为CSS颜色值（添加#前缀）
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
    if (group.primaryBudget && group.primaryBudget.categoryColor) {
        return `#${group.primaryBudget.categoryColor}`;
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
 * 2. 如果有一级分类预算但没有已花费，用 totalSpent / primaryAmount 计算
 * 3. 如果没有一级分类预算，用 totalSpent / totalAmount 计算（二级之和）
 */
function getGroupExecutionRate(group: BudgetGroup): number {
    // 如果有一级分类预算
    if (group.primaryBudget) {
        // 如果一级分类预算有执行率（后端已计算），使用它
        if (group.primaryBudget.executionRate > 0) {
            return group.primaryBudget.executionRate;
        }
        // 否则用 totalSpent / primaryAmount 计算
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
    activeViewMode.value = mode as string;
    if (mode === 'forecast') {
        loadForecast();
    } else {
        reload(false);
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

/**
 * 加载预算列表和执行数据
 */
async function reload(force: boolean): Promise<void> {
    loading.value = true;

    try {
        await budgetStore.loadAllBudgets({
            force,
            type: activeBudgetType.value,
            periodType: getCurrentPeriodType()
        });

        // 同时加载执行数据
        await budgetStore.loadBudgetExecution({
            type: activeBudgetType.value,
            periodType: getCurrentPeriodType()
        });
    } catch (error: unknown) {
        const err = error as { isUpToDate?: boolean; message?: string };
        if (!err.isUpToDate) {
            snackbar.value?.showError(err.message || tt('Failed to load budgets'));
        }
    } finally {
        loading.value = false;
    }
}

/**
 * 加载周期预计
 */
async function loadForecast(): Promise<void> {
    try {
        await budgetStore.loadBudgetForecast({
            type: activeBudgetType.value,
            periodType: getCurrentPeriodType()
        });
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

        // 下载JSON文件
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
        activeViewMode.value = props.initViewMode;
    }

    // 加载筛选预设
    loadPresetsFromStorage();

    // 初始加载
    reload(false);
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

/* 使用 :deep() 穿透到子组件 */
.budget-nav-buttons:deep(.v-btn) {
    width: 100%;
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

