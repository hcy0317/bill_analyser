<template>
    <v-data-table
        fixed-header
        fixed-footer
        show-select
        multi-sort
        density="compact"
        item-value="index"
        :class="{ 'import-transaction-table': true, 'disabled': !!disabled }"
        :height="importTransactionsTableHeight"
        :headers="importTransactionHeaders"
        :items="importTransactions"
        :search="JSON.stringify(filters)"
        :custom-filter="importTransactionsFilter"
        :no-data-text="tt('No data to import')"
        v-model:items-per-page="countPerPage"
        v-model:page="currentPage"
    >
        <template #header.data-table-select>
            <v-checkbox readonly class="always-cursor-pointer"
                        density="compact" width="28"
                        :disabled="!!disabled"
                        :indeterminate="anyButNotAllTransactionSelected"
                        v-model="allTransactionSelected"
            >
                <v-menu activator="parent" location="bottom">
                    <v-list>
                        <v-list-item :prepend-icon="mdiSelectAll"
                                     :title="tt('Select All Valid Items')"
                                     :disabled="!!disabled"
                                     @click="selectAllValid"></v-list-item>
                        <v-list-item :prepend-icon="mdiSelectAll"
                                     :title="tt('Select All Invalid Items')"
                                     :disabled="!!disabled"
                                     @click="selectAllInvalid"></v-list-item>
                        <v-list-item :prepend-icon="mdiMessageAlertOutline"
                                     :title="getSelectAllAnnotationText()"
                                     :disabled="!!disabled"
                                     @click="selectAllNeedsAnnotation"></v-list-item>
                        <v-divider class="my-2"/>
                        <v-list-item :prepend-icon="mdiSelectAll"
                                     :title="tt('Select All')"
                                     :disabled="!!disabled"
                                     @click="selectAll"></v-list-item>
                        <v-list-item :prepend-icon="mdiSelect"
                                     :title="tt('Select None')"
                                     :disabled="!!disabled"
                                     @click="selectNone"></v-list-item>
                        <v-list-item :prepend-icon="mdiSelectInverse"
                                     :title="tt('Invert Selection')"
                                     :disabled="!!disabled"
                                     @click="selectInvert"></v-list-item>
                        <v-divider class="my-2"/>
                        <v-list-item :prepend-icon="mdiSelectAll"
                                     :title="tt('Select All in This Page')"
                                     :disabled="!!disabled"
                                     @click="selectAllInThisPage"></v-list-item>
                        <v-list-item :prepend-icon="mdiSelect"
                                     :title="tt('Select None in This Page')"
                                     :disabled="!!disabled"
                                     @click="selectNoneInThisPage"></v-list-item>
                        <v-list-item :prepend-icon="mdiSelectInverse"
                                     :title="tt('Invert Selection in This Page')"
                                     :disabled="!!disabled"
                                     @click="selectInvertInThisPage"></v-list-item>
                    </v-list>
                </v-menu>
            </v-checkbox>
        </template>
        <template #item.data-table-select="{ item }">
            <v-checkbox density="compact"
                        :color="!item.valid ? 'error' : 'primary'"
                        :disabled="!!disabled"
                        v-model="item.selected"></v-checkbox>
        </template>
        <template #item.valid="{ item }">
            <div class="d-flex align-center ga-1">
            <v-icon size="small" :class="{ 'text-error': !item.valid }"
                :disabled="!!disabled"
                :icon="editingTransaction === item ? mdiCheck : mdiPencilOutline"
                @click="editTransaction(item)">
            </v-icon>
            <v-icon v-if="needsAnnotation(item)"
                size="small"
                color="warning"
                :icon="mdiMessageAlertOutline"
                :title="getAnnotationSummary(item)">
            </v-icon>
            </div>
        </template>
        <template #item.time="{ item }">
            <span>{{ getDisplayDateTime(item) }}</span>
            <v-chip class="ms-1" variant="flat" color="grey" size="x-small"
                    v-if="item.utcOffset !== currentTimezoneOffsetMinutes">{{ getDisplayTimezone(item) }}</v-chip>
        </template>
        <!-- v6.77: 类型列 - 支持编辑模式切换 -->
        <template #item.type="{ item }">
            <!-- 非编辑状态：显示类型标签 -->
            <div v-if="editingTransaction !== item" :key="`type-view-${item.index}`">
                <v-chip label color="secondary" variant="outlined" size="x-small" v-if="item.type === TransactionType.ModifyBalance">{{ tt('Modify Balance') }}</v-chip>
                <v-chip label class="text-income" variant="outlined" size="x-small" v-else-if="item.type === TransactionType.Income">{{ tt('Income') }}</v-chip>
                <v-chip label class="text-expense" variant="outlined" size="x-small" v-else-if="item.type === TransactionType.Expense">{{ tt('Expense') }}</v-chip>
                <v-chip label color="primary" variant="outlined" size="x-small" v-else-if="item.type === TransactionType.Transfer">{{ tt('Transfer') }}</v-chip>
                <v-chip label color="warning" variant="outlined" size="x-small" v-else-if="item.type === TransactionType.Investment">{{ tt('Investment') }}</v-chip>
                <v-chip label color="default" variant="outlined" size="x-small" v-else>{{ tt('Unknown') }}</v-chip>
                <div class="mt-1" v-if="item.hasTransferSuggestion()">
                    <v-chip
                        color="warning"
                        variant="tonal"
                        size="x-small"
                        :prepend-icon="mdiLightbulbOutline"
                        :title="item.transferSuggestionReason"
                        @click.stop="applySuggestedType(item)">
                        {{ tt('Likely Transfer') }}
                    </v-chip>
                </div>
                <div class="mt-1" v-if="item.hasInvestmentSignal()">
                    <v-chip
                        color="info"
                        variant="tonal"
                        size="x-small"
                        :prepend-icon="mdiChartLine"
                        :title="item.investmentSignalReason">
                        {{ tt('Investment Signal') }}
                    </v-chip>
                    <div class="text-caption text-medium-emphasis ms-1 mt-1"
                         v-if="item.getInvestmentProfileText()">
                        {{ item.getInvestmentProfileText() }}
                    </div>
                </div>
                <div class="mt-1" v-if="item.hasLearningRecommendation()">
                    <v-chip
                        color="secondary"
                        variant="tonal"
                        size="x-small"
                        :prepend-icon="mdiSchoolOutline"
                        :title="item.learningRecommendationReason">
                        {{ tt('Learning Suggestion') }}
                    </v-chip>
                    <div class="text-caption text-medium-emphasis ms-1 mt-1"
                         v-if="item.learningRecommendationSummary">
                        {{ item.learningRecommendationSummary }}
                    </div>
                </div>
                <div class="mt-1" v-if="item.hasRecurringMatch() || item.recurringCandidateCount > 0">
                    <v-chip
                        v-if="item.hasRecurringMatch()"
                        color="success"
                        variant="tonal"
                        size="x-small"
                        :title="getRecurringMatchSummary(item)">
                        {{ tt('Scheduled Match') }}
                    </v-chip>
                    <v-chip
                        v-if="item.recurringCandidateCount > 0"
                        class="ms-1"
                        color="info"
                        variant="outlined"
                        size="x-small">
                        {{ tt('Scheduled Candidates') }} {{ getDisplayCount(item.recurringCandidateCount) }}
                    </v-chip>
                    <v-chip
                        v-if="item.recurringCandidateCount > 0 && getPrimaryRecurringReason(item)"
                        class="ms-1"
                        color="amber"
                        variant="tonal"
                        size="x-small"
                        :prepend-icon="mdiStar">
                        {{ tt('Best Candidate') }} · {{ getPrimaryRecurringReason(item) }}
                    </v-chip>
                    <v-btn
                        class="mt-1"
                        variant="text"
                        color="success"
                        size="x-small"
                        :disabled="!!disabled || !props.sessionId"
                        @click.stop="openRecurringCandidateDialog(item)">
                        {{ tt('Choose Scheduled Match') }}
                    </v-btn>
                </div>
            </div>
            <!-- 编辑状态：类型选择器（余额调整类型不可编辑） -->
            <div style="width: 120px" v-else :key="`type-edit-${item.index}`">
                <v-select
                    density="compact"
                    variant="plain"
                    hide-details
                    :disabled="!!disabled || item.type === TransactionType.ModifyBalance"
                    :items="transactionTypeOptions"
                    item-title="text"
                    item-value="value"
                    v-model="item.type"
                    @update:model-value="onTransactionTypeChange(item)"
                ></v-select>
                <v-btn
                    v-if="item.hasTransferSuggestion()"
                    class="mt-1"
                    variant="text"
                    color="warning"
                    size="x-small"
                    :prepend-icon="mdiLightbulbOutline"
                    @click.stop="applySuggestedType(item)">
                    {{ tt('Apply Suggestion') }}
                </v-btn>
                <v-chip
                    v-if="item.hasInvestmentSignal()"
                    class="mt-1"
                    color="info"
                    variant="tonal"
                    size="x-small"
                    :prepend-icon="mdiChartLine"
                    :title="item.investmentSignalReason">
                    {{ tt('Investment Signal') }}
                </v-chip>
                <div class="text-caption text-medium-emphasis mt-1"
                     v-if="item.hasInvestmentSignal() && item.getInvestmentProfileText()">
                    {{ item.getInvestmentProfileText() }}
                </div>
                <div class="text-caption text-medium-emphasis mt-1"
                     v-if="item.hasLearningRecommendation() && item.learningRecommendationSummary">
                    {{ item.learningRecommendationSummary }}
                </div>
                <div class="mt-1" v-if="item.hasRecurringMatch() || item.recurringCandidateCount > 0">
                    <v-chip
                        v-if="item.hasRecurringMatch()"
                        color="success"
                        variant="tonal"
                        size="x-small"
                        :title="getRecurringMatchSummary(item)">
                        {{ tt('Scheduled Match') }}
                    </v-chip>
                    <v-btn
                        class="mt-1"
                        variant="text"
                        color="success"
                        size="x-small"
                        :disabled="!!disabled || !props.sessionId"
                        @click.stop="openRecurringCandidateDialog(item)">
                        {{ tt('Choose Scheduled Match') }}
                    </v-btn>
                    <v-btn
                        class="mt-1"
                        variant="text"
                        color="warning"
                        size="x-small"
                        :disabled="!item.hasRecurringMatch()"
                        @click.stop="clearRecurringMatch(item)">
                        {{ tt('Clear Scheduled Match') }}
                    </v-btn>
                </div>
            </div>
        </template>
        <template #item.actualCategoryName="{ item }">
            <!-- 非编辑状态或余额调整类型：显示分类名称 -->
            <!-- 使用 v-if 避免创建不必要的复杂组件实例 -->
            <div class="d-flex align-center" v-if="editingTransaction !== item || item.type === TransactionType.ModifyBalance" :key="`cat-view-${item.index}`">
                <span v-if="item.type === TransactionType.ModifyBalance">-</span>
                <ItemIcon size="24px" icon-type="category"
                          :icon-id="allCategoriesMap[item.categoryId]?.icon ?? ''"
                          :color="allCategoriesMap[item.categoryId]?.color ?? ''"
                          v-if="item.type !== TransactionType.ModifyBalance && item.categoryId && item.categoryId !== '0' && allCategoriesMap[item.categoryId]"></ItemIcon>
                <span class="ms-2" v-if="item.type !== TransactionType.ModifyBalance && item.categoryId && item.categoryId !== '0' && allCategoriesMap[item.categoryId]">
                                    {{ allCategoriesMap[item.categoryId]?.name }}
                                </span>
                <div class="text-error font-italic" v-else-if="item.type !== TransactionType.ModifyBalance && (!item.categoryId || item.categoryId === '0' || !allCategoriesMap[item.categoryId])">
                    <v-icon class="me-1" :icon="mdiAlertOutline"/>
                    <span>{{ item.originalCategoryName }}</span>
                </div>
            </div>
            <!-- 编辑状态：统一分类选择器（根据交易类型动态选择分类列表） -->
            <!-- 使用 v-if 确保只有当前编辑行才创建选择器组件，避免同时创建大量组件实例 -->
            <div style="width: 260px" v-if="editingTransaction === item && item.type !== TransactionType.ModifyBalance" :key="`cat-edit-${item.index}`">
                <two-column-select density="compact" variant="plain"
                                   primary-key-field="id" primary-value-field="id" primary-title-field="name"
                                   primary-icon-field="icon" primary-icon-type="category" primary-color-field="color"
                                   primary-hidden-field="hidden" primary-sub-items-field="subCategories"
                                   secondary-key-field="id" secondary-value-field="id" secondary-title-field="name"
                                   secondary-icon-field="icon" secondary-icon-type="category" secondary-color-field="color"
                                   secondary-hidden-field="hidden"
                                   :disabled="!!disabled || !hasAvailableCategoriesForType(item.type)"
                                   :enable-filter="true" :filter-placeholder="tt('Find category')" :filter-no-items-text="tt('No available category')"
                                   :show-selection-primary-text="true"
                                   :custom-selection-primary-text="getCategoryPrimaryText(item)"
                                   :custom-selection-secondary-text="getCategorySecondaryText(item)"
                                   :placeholder="tt('Category')"
                                   :items="getCategoriesForType(item.type)"
                                   v-model="item.categoryId">
                </two-column-select>
            </div>
        </template>
        <template #item.sourceAmount="{ item }">
            <!-- 非编辑状态：显示金额 -->
            <div class="d-flex align-center" v-if="editingTransaction !== item" :key="`amount-view-${item.index}`">
                <span>{{ getTransactionDisplayAmount(item) }}</span>
                <v-icon class="icon-with-direction mx-1" size="13" :icon="mdiArrowRight" v-if="requiresDestinationAccount(item) && item.sourceAccountId !== item.destinationAccountId"></v-icon>
                <span v-if="requiresDestinationAccount(item) && item.sourceAccountId !== item.destinationAccountId">{{ getTransactionDisplayDestinationAmount(item) }}</span>
            </div>
            <!-- 编辑状态：金额输入框 -->
            <div class="d-flex align-center" :style="`width: ${requiresDestinationAccount(item) && item.sourceAccountId !== item.destinationAccountId ? 250 : 100}px`" v-else :key="`amount-edit-${item.index}`">
                <amount-input density="compact" variant="plain"
                              persistent-placeholder
                              :currency="item.originalSourceAccountCurrency || defaultCurrency"
                              :show-currency="true"
                              :disabled="!!disabled"
                              :placeholder="tt('Amount')"
                              v-model="item.sourceAmount"/>
                <v-icon class="icon-with-direction mx-1" size="13" :icon="mdiArrowRight" v-if="requiresDestinationAccount(item) && item.sourceAccountId !== item.destinationAccountId"></v-icon>
                <amount-input density="compact" variant="plain"
                              persistent-placeholder
                              :currency="item.originalDestinationAccountCurrency || defaultCurrency"
                              :show-currency="true"
                              :disabled="!!disabled"
                              :placeholder="tt('Destination Amount')"
                              v-model="item.destinationAmount"
                              v-if="requiresDestinationAccount(item) && item.sourceAccountId !== item.destinationAccountId"/>
            </div>
        </template>
        <template #item.actualSourceAccountName="{ item }">
            <!-- 非编辑状态：显示账户名称 -->
            <div class="d-flex align-center" v-if="editingTransaction !== item" :key="`account-view-${item.index}`">
                <span v-if="item.sourceAccountId && item.sourceAccountId !== '0' && allAccountsMap[item.sourceAccountId]">{{ allAccountsMap[item.sourceAccountId]?.name }}</span>
                <div class="text-error font-italic" v-else>
                    <v-icon class="me-1" :icon="mdiAlertOutline"/>
                    <span>{{ item.originalSourceAccountName }}</span>
                </div>
                <v-icon class="icon-with-direction mx-1" size="13" :icon="mdiArrowRight" v-if="requiresDestinationAccount(item)"></v-icon>
                <span v-if="requiresDestinationAccount(item) && item.destinationAccountId && item.destinationAccountId !== '0' && allAccountsMap[item.destinationAccountId]">{{allAccountsMap[item.destinationAccountId]?.name }}</span>
                <div class="text-error font-italic" v-else-if="requiresDestinationAccount(item) && (!item.destinationAccountId || item.destinationAccountId === '0' || !allAccountsMap[item.destinationAccountId])">
                    <v-icon class="me-1" :icon="mdiAlertOutline"/>
                    <span>{{ item.originalDestinationAccountName }}</span>
                </div>
            </div>
            <!-- 编辑状态：账户选择器 -->
            <div class="d-flex align-center" :style="`width: ${requiresDestinationAccount(item) ? 450 : 200}px`" v-else :key="`account-edit-${item.index}`">
                <two-column-select density="compact" variant="plain"
                                   primary-key-field="id" primary-value-field="category"
                                   primary-title-field="name" primary-footer-field="displayBalance"
                                   primary-icon-field="icon" primary-icon-type="account"
                                   primary-sub-items-field="accounts"
                                   :primary-title-i18n="true"
                                   secondary-key-field="id" secondary-value-field="id"
                                   secondary-title-field="name" secondary-footer-field="displayBalance"
                                   secondary-icon-field="icon" secondary-icon-type="account" secondary-color-field="color"
                                   :disabled="!!disabled || !allVisibleAccounts.length"
                                   :enable-filter="true" :filter-placeholder="tt('Find account')" :filter-no-items-text="tt('No available account')"
                                   :custom-selection-primary-text="getSourceAccountDisplayName(item)"
                                   :placeholder="getSourceAccountTitle(item)"
                                   :items="allVisibleCategorizedAccounts"
                                   v-model="item.sourceAccountId">
                </two-column-select>
                <v-icon class="icon-with-direction mx-1" size="13" :icon="mdiArrowRight" v-if="requiresDestinationAccount(item)"></v-icon>
                <two-column-select density="compact" variant="plain"
                                   primary-key-field="id" primary-value-field="category"
                                   primary-title-field="name" primary-footer-field="displayBalance"
                                   primary-icon-field="icon" primary-icon-type="account"
                                   primary-sub-items-field="accounts"
                                   :primary-title-i18n="true"
                                   secondary-key-field="id" secondary-value-field="id"
                                   secondary-title-field="name" secondary-footer-field="displayBalance"
                                   secondary-icon-field="icon" secondary-icon-type="account" secondary-color-field="color"
                                   :disabled="!!disabled || !allVisibleAccounts.length"
                                   :enable-filter="true" :filter-placeholder="tt('Find account')" :filter-no-items-text="tt('No available account')"
                                   :custom-selection-primary-text="getDestinationAccountDisplayName(item)"
                                   :placeholder="getDestinationAccountTitle(item)"
                                   :items="allVisibleCategorizedAccounts"
                                   v-model="item.destinationAccountId"
                                   v-if="requiresDestinationAccount(item)">
                </two-column-select>
            </div>
        </template>
        <template #item.geoLocation="{ item }">
            <span v-if="item.geoLocation">{{ `(${formatCoordinate(item.geoLocation, coordinateDisplayType)})` }}</span>
            <span v-else-if="!item.geoLocation">{{ tt('None') }}</span>
        </template>
        <template #item.tagIds="{ item }">
            <!-- 非编辑状态：显示标签 -->
            <div v-if="editingTransaction !== item" :key="`tags-view-${item.index}`">
                <v-chip class="transaction-tag" size="small"
                        :class="{ 'font-italic': !tagId || tagId === '0' || !allTagsMap[tagId] }"
                        :prepend-icon="tagId && tagId !== '0' && allTagsMap[tagId] ? mdiPound : mdiAlertOutline"
                        :color="tagId && tagId !== '0' && allTagsMap[tagId] ? 'default' : 'error'"
                        :text="tagId && tagId !== '0' && allTagsMap[tagId] ? allTagsMap[tagId].name : item.originalTagNames[index]"
                        :key="tagId"
                        v-for="(tagId, index) in item.tagIds"/>
                <v-chip class="transaction-tag" size="small"
                        :text="tt('None')"
                        v-if="!item.tagIds || !item.tagIds.length"/>
            </div>
            <!-- 编辑状态：标签选择器 -->
            <div style="width: 200px" v-else :key="`tags-edit-${item.index}`">
                <v-autocomplete
                    item-title="name"
                    item-value="id"
                    auto-select-first
                    persistent-placeholder
                    multiple
                    chips
                    closable-chips
                    density="compact" variant="plain"
                    :disabled="!!disabled"
                    :placeholder="tt('None')"
                    :items="allTags"
                    :no-data-text="tt('No available tag')"
                    v-model="editingTags"
                >
                    <template #chip="{ props, index }">
                        <v-chip :class="{ 'font-italic': !isTagValid(editingTags, index) }"
                                :prepend-icon="isTagValid(editingTags, index) ? mdiPound : mdiAlertOutline"
                                :color="isTagValid(editingTags, index) ? 'default' : 'error'"
                                :text="isTagValid(editingTags, index) ? allTagsMap[editingTags[index] as string]?.name : item.originalTagNames[index]"
                                v-bind="props"/>
                    </template>

                    <template #item="{ props, item }">
                        <v-list-item :value="item.value" v-bind="props" v-if="!item.raw.hidden">
                            <template #title>
                                <v-list-item-title>
                                    <div class="d-flex align-center">
                                        <v-icon size="20" start :icon="mdiPound"/>
                                        <span>{{ item.title }}</span>
                                    </div>
                                </v-list-item-title>
                            </template>
                        </v-list-item>
                    </template>
                </v-autocomplete>
            </div>
        </template>
        <!-- v6.32新增: 交易对方列 -->
        <template #item.counterparty="{ item }">
            <span v-if="editingTransaction !== item" :key="`counterparty-view-${item.index}`">{{ item.counterparty || '-' }}</span>
            <div v-else :key="`counterparty-edit-${item.index}`">
                <v-text-field style="width: 150px" type="text"
                              density="compact" variant="plain"
                              persistent-placeholder
                              :placeholder="tt('Counterparty')"
                              :disabled="!!disabled"
                              v-model="item.counterparty" />
            </div>
        </template>
        <!-- v6.32新增: 支付方式列 -->
        <template #item.paymentMethod="{ item }">
            <span v-if="editingTransaction !== item" :key="`paymentMethod-view-${item.index}`">{{ item.paymentMethod || '-' }}</span>
            <div v-else :key="`paymentMethod-edit-${item.index}`">
                <v-text-field style="width: 150px" type="text"
                              density="compact" variant="plain"
                              persistent-placeholder
                              :placeholder="tt('Payment Method')"
                              :disabled="!!disabled"
                              v-model="item.paymentMethod" />
            </div>
        </template>
        <template #item.parserSource="{ item }">
            <v-chip v-if="item.parserSource" size="x-small" :color="getParserColor(item.parserSource)">
                {{ getParserLabel(item.parserSource) }}
            </v-chip>
            <span v-else>-</span>
        </template>
        <template #item.comment="{ item }">
            <!-- 非编辑状态：显示备注 -->
            <span v-if="editingTransaction !== item" :key="`comment-view-${item.index}`">{{ item.comment || '' }}</span>
            <!-- 编辑状态：备注输入框 -->
            <div v-else :key="`comment-edit-${item.index}`">
                <v-text-field style="width: 200px" type="text"
                              density="compact" variant="plain"
                              persistent-placeholder
                              :placeholder="tt('Description')"
                              :disabled="!!disabled"
                              v-model="item.comment" />
            </div>
        </template>
        <template #bottom>
            <div class="title-and-toolbar d-flex align-center text-no-wrap mt-2" v-if="importTransactions">
                <span :class="{ 'text-error': selectedInvalidTransactionCount > 0 }">
                    {{ tt('format.misc.selectedCount', { count: getDisplayCount(selectedImportTransactionCount), totalCount: getDisplayCount(importTransactions.length) }) }}
                </span>
                <v-chip class="ms-3"
                        color="warning"
                        variant="tonal"
                        size="small"
                        v-if="annotationTransactionCount > 0">
                    {{ getNeedsAnnotationText() }} {{ getDisplayCount(annotationTransactionCount) }}
                </v-chip>
                <v-btn class="ms-2"
                       v-if="aiAnnotationEnabled"
                       density="compact"
                       variant="tonal"
                       color="warning"
                       :disabled="!!disabled || selectedAnnotationTransactionCount < 1"
                       :prepend-icon="mdiMessageAlertOutline"
                       @click="openAnnotationDialog">
                    {{ getAnnotationActionText() }}
                </v-btn>

                <!-- 快速编辑按钮组 -->
                <v-btn-group class="ms-4" density="compact" variant="outlined" color="primary">
                    <!-- v6.56: 移除选择限制，重新分类按钮始终可点击 -->
                    <v-btn :disabled="!!disabled"
                           :prepend-icon="mdiAutoFix"
                           @click="reclassifySelected">
                        {{ tt('Reclassify') }}
                    </v-btn>
                              <v-btn :disabled="!!disabled || selectedImportTransactionCount < 1 || !props.sessionId"
                                    :prepend-icon="mdiSchoolOutline"
                                    @click="promoteSelectedToLongTermLearning">
                                {{ tt('Save as Long-term Learning') }}
                          </v-btn>
                    <!-- v6.40: 分类按钮直接打开管理分类对话框 -->
                    <v-btn :disabled="!!disabled"
                           :prepend-icon="mdiTagMultiple"
                           @click="openCategoryManagement">
                        {{ tt('Manage Categories') }}
                    </v-btn>
                    <!-- v6.40: 账户按钮直接打开管理账户对话框 -->
                    <v-btn :disabled="!!disabled"
                           :prepend-icon="mdiWallet"
                           @click="openAccountManagement">
                        {{ tt('Manage Accounts') }}
                    </v-btn>
                </v-btn-group>

                <v-spacer v-if="importTransactions.length > 10"/>
                <span v-if="importTransactions.length > 10">{{ tt('Transactions Per Page') }}</span>
                <v-select class="ms-2" density="compact" max-width="100"
                          item-title="name"
                          item-value="value"
                          :disabled="!!disabled"
                          :items="importTransactionsTablePageOptions"
                          v-model="countPerPage"
                          v-if="importTransactions.length > 10"
                />
                <pagination-buttons density="compact"
                                    :disabled="!!disabled"
                                    :totalPageCount="totalPageCount"
                                    v-model="currentPage"
                                    v-if="importTransactions.length > 10"></pagination-buttons>
            </div>
        </template>
    </v-data-table>

    <!-- 批量编辑分类对话框 -->
    <v-dialog width="640" v-model="showBatchCategoryDialog">
        <v-card class="pa-4">
            <v-card-title class="text-center">
                <h4 class="text-h5">{{ tt('Edit Category for Selected') }}</h4>
            </v-card-title>
            <v-card-text>
                <p class="text-body-2 text-medium-emphasis mb-4">
                    {{ tt('format.misc.selectedCount', { count: selectedImportTransactionCount, totalCount: importTransactions?.length || 0 }) }}
                </p>
                <!-- 分类类型选择 -->
                <v-select
                    :label="tt('Transaction Type')"
                    :items="batchCategoryTypeOptions"
                    item-title="name"
                    item-value="value"
                    v-model="batchCategoryType"
                    class="mb-4"
                />
                <!-- 分类选择器 -->
                <two-column-select
                    :label="tt('Category')"
                    :placeholder="tt('Select Category')"
                    :items="getBatchCategoryItems()"
                    primary-key-field="id"
                    primary-value-field="id"
                    primary-title-field="name"
                    primary-icon-field="icon"
                    primary-icon-type="category"
                    primary-color-field="color"
                    primary-hidden-field="hidden"
                    primary-sub-items-field="subCategories"
                    secondary-key-field="id"
                    secondary-value-field="id"
                    secondary-title-field="name"
                    secondary-icon-field="icon"
                    secondary-icon-type="category"
                    secondary-color-field="color"
                    secondary-hidden-field="hidden"
                    :enable-filter="true"
                    :filter-placeholder="tt('Find category')"
                    v-model="batchCategoryId"
                />
            </v-card-text>
            <v-card-actions class="justify-center gap-4">
                <v-btn color="primary" :disabled="!batchCategoryId" @click="applyBatchCategory">
                    {{ tt('Apply') }}
                </v-btn>
                <v-btn color="secondary" variant="tonal" @click="showBatchCategoryDialog = false">
                    {{ tt('Cancel') }}
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <!-- 批量编辑账户对话框 -->
    <v-dialog width="640" v-model="showBatchAccountDialog">
        <v-card class="pa-4">
            <v-card-title class="text-center">
                <h4 class="text-h5">{{ tt('Edit Account for Selected') }}</h4>
            </v-card-title>
            <v-card-text>
                <p class="text-body-2 text-medium-emphasis mb-4">
                    {{ tt('format.misc.selectedCount', { count: selectedImportTransactionCount, totalCount: importTransactions?.length || 0 }) }}
                </p>
                <!-- 账户选择器 -->
                <icon-select
                    :label="tt('Account')"
                    :placeholder="tt('Select Account')"
                    :items="availableAccounts"
                    item-value-field="id"
                    item-title-field="name"
                    item-icon-field="icon"
                    item-icon-type="account"
                    item-color-field="color"
                    item-hidden-field="hidden"
                    :enable-filter="true"
                    :filter-placeholder="tt('Find account')"
                    v-model="batchAccountId"
                />
            </v-card-text>
            <v-card-actions class="justify-center gap-4">
                <v-btn color="primary" :disabled="!batchAccountId" @click="applyBatchAccount">
                    {{ tt('Apply') }}
                </v-btn>
                <v-btn color="secondary" variant="tonal" @click="showBatchAccountDialog = false">
                    {{ tt('Cancel') }}
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <v-dialog width="640" v-model="showCustomDescriptionDialog">
        <v-card class="pa-2 pa-sm-4 pa-md-4">
            <template #title>
                <div class="d-flex align-center justify-center">
                    <h4 class="text-h4">{{ tt('Filter Description') }}</h4>
                </div>
            </template>
            <v-card-text class="mb-md-4 w-100 d-flex justify-center">
                <v-text-field
                    type="text"
                    persistent-placeholder
                    :label="tt('Description')"
                    :placeholder="tt('Description')"
                    v-model="currentDescriptionFilterValue"
                />
            </v-card-text>
            <v-card-text class="overflow-y-visible">
                <div class="w-100 d-flex justify-center gap-4">
                    <v-btn :disabled="!currentDescriptionFilterValue" @click="showCustomDescriptionDialog = false; filters.description = currentDescriptionFilterValue">{{ tt('OK') }}</v-btn>
                    <v-btn color="secondary" variant="tonal" @click="showCustomDescriptionDialog = false; currentDescriptionFilterValue = ''">{{ tt('Cancel') }}</v-btn>
                </div>
            </v-card-text>
        </v-card>
    </v-dialog>

    <date-range-selection-dialog :title="tt('Custom Date Range')"
                                 :min-time="filters.minDatetime"
                                 :max-time="filters.maxDatetime"
                                 v-model:show="showCustomDateRangeDialog"
                                 @dateRange:change="changeCustomDateFilter"
                                 @error="onShowDateRangeError" />
    <batch-replace-dialog ref="batchReplaceDialog" />
    <batch-replace-all-types-dialog ref="batchReplaceAllTypesDialog" />
    <batch-create-dialog ref="batchCreateDialog" />
    <snack-bar ref="snackbar" />

    <!-- v6.34: 分类管理选择对话框 -->
    <v-dialog width="700" v-model="showCategorySelectDialog">
        <v-card class="pa-4">
            <v-card-title class="text-center">
                <h4 class="text-h5">{{ tt('Select Category to Edit') }}</h4>
            </v-card-title>
            <v-card-text>
                <p class="text-body-2 text-medium-emphasis mb-4">
                    {{ tt('Select a category to edit its keywords and settings') }}
                </p>
                <!-- 分类类型切换 -->
                <v-tabs v-model="manageCategoryType" class="mb-4">
                    <v-tab :value="CategoryType.Expense">{{ tt('Expense') }}</v-tab>
                    <v-tab :value="CategoryType.Income">{{ tt('Income') }}</v-tab>
                    <v-tab :value="CategoryType.Transfer">{{ tt('Transfer') }}</v-tab>
                    <v-tab :value="CategoryType.Investment">{{ tt('Investment') }}</v-tab>
                </v-tabs>
                <!-- 分类选择器 -->
                <two-column-select
                    :label="tt('Category')"
                    :placeholder="tt('Select Category')"
                    :items="getManageCategoryItems()"
                    primary-key-field="id"
                    primary-value-field="id"
                    primary-title-field="name"
                    primary-icon-field="icon"
                    primary-icon-type="category"
                    primary-color-field="color"
                    primary-hidden-field="hidden"
                    primary-sub-items-field="subCategories"
                    secondary-key-field="id"
                    secondary-value-field="id"
                    secondary-title-field="name"
                    secondary-icon-field="icon"
                    secondary-icon-type="category"
                    secondary-color-field="color"
                    secondary-hidden-field="hidden"
                    :enable-filter="true"
                    :filter-placeholder="tt('Find category')"
                    v-model="manageCategoryId"
                />
            </v-card-text>
            <v-card-actions class="justify-center gap-4">
                <v-btn color="primary" :disabled="!manageCategoryId" @click="openSelectedCategoryEditDialog">
                    {{ tt('Edit') }}
                </v-btn>
                <v-btn color="secondary" variant="tonal" @click="showCategorySelectDialog = false">
                    {{ tt('Cancel') }}
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <!-- v6.34: 分类编辑对话框 -->
    <category-edit-dialog ref="categoryEditDialog" />

    <!-- v6.34: 账户管理选择对话框 -->
    <v-dialog width="600" v-model="showAccountSelectDialog">
        <v-card class="pa-4">
            <v-card-title class="text-center">
                <h4 class="text-h5">{{ tt('Select Account to Edit') }}</h4>
            </v-card-title>
            <v-card-text>
                <p class="text-body-2 text-medium-emphasis mb-4">
                    {{ tt('Select an account to edit its aliases and settings') }}
                </p>
                <!-- 账户选择器 -->
                <v-select
                    :label="tt('Account')"
                    :placeholder="tt('Select Account')"
                    :items="allDisplayAccounts"
                    item-title="name"
                    item-value="id"
                    :no-data-text="tt('No available account')"
                    v-model="manageAccountId"
                >
                    <template #item="{ props, item }">
                        <v-list-item v-bind="props">
                            <template #prepend>
                                <ItemIcon class="me-2" icon-type="account"
                                          :icon-id="item.raw.icon"
                                          :color="item.raw.color" />
                            </template>
                        </v-list-item>
                    </template>
                </v-select>
            </v-card-text>
            <v-card-actions class="justify-center gap-4">
                <v-btn color="primary" :disabled="!manageAccountId" @click="openSelectedAccountEditDialog">
                    {{ tt('Edit') }}
                </v-btn>
                <v-btn color="secondary" variant="tonal" @click="showAccountSelectDialog = false">
                    {{ tt('Cancel') }}
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <!-- v6.34: 账户编辑对话框 -->
    <account-edit-dialog ref="accountEditDialog" />

    <v-dialog width="760" v-model="showRecurringCandidateDialog">
        <v-card class="pa-4">
            <v-card-title class="text-center">
                <h4 class="text-h5">{{ tt('Choose Scheduled Match') }}</h4>
            </v-card-title>
            <v-card-text>
                <p class="text-body-2 text-medium-emphasis mb-4">
                    {{ tt('Pick a scheduled transaction to link with this imported bill') }}
                </p>
                <div class="text-body-2 mb-4" v-if="recurringCandidateTarget">
                    {{ getAnnotationListTitle(recurringCandidateTarget) }}
                </div>
                <div class="d-flex justify-center py-8" v-if="recurringCandidateLoading">
                    <v-progress-circular indeterminate color="primary" />
                </div>
                <v-list class="border rounded" lines="three" max-height="360" v-else-if="recurringCandidates.length > 0">
                    <v-list-item
                        v-for="candidate in recurringCandidates"
                        :key="String(candidate.id)"
                        :active="selectedRecurringCandidateId === String(candidate.id)"
                        @click="selectedRecurringCandidateId = String(candidate.id)">
                        <template #prepend>
                            <v-icon :icon="selectedRecurringCandidateId === String(candidate.id) ? mdiCheck : mdiPencilOutline" />
                        </template>
                        <v-list-item-title>
                            <div class="d-flex align-center ga-2 flex-wrap">
                                <span>{{ candidate.name || tt('Unnamed Template') }}</span>
                                <v-chip v-if="isBestRecurringCandidate(candidate)"
                                        color="amber"
                                        variant="tonal"
                                        size="x-small"
                                        :prepend-icon="mdiStar">
                                    {{ tt('Best Candidate') }}
                                </v-chip>
                                <v-chip color="success" variant="tonal" size="x-small">
                                    {{ tt('Match Score') }} {{ candidate.matchScore || 0 }}
                                </v-chip>
                            </div>
                        </v-list-item-title>
                        <v-list-item-subtitle>
                            {{ formatRecurringCandidateSubtitle(candidate) }}
                        </v-list-item-subtitle>
                        <div class="text-caption text-medium-emphasis mt-1"
                             v-if="isBestRecurringCandidate(candidate) && getRecurringCandidatePrimaryReason(candidate)">
                            {{ tt('Best Candidate Reason') }}: {{ getRecurringCandidatePrimaryReason(candidate) }}
                        </div>
                    </v-list-item>
                </v-list>
                <v-alert type="info" variant="tonal" v-else>
                    {{ tt('No Scheduled Candidates') }}
                </v-alert>
            </v-card-text>
            <v-card-actions class="justify-center gap-4 flex-wrap">
                <v-btn color="primary"
                       :disabled="!selectedRecurringCandidateId"
                       @click="applySelectedRecurringCandidate">
                    {{ tt('Apply') }}
                </v-btn>
                <v-btn color="warning"
                       variant="tonal"
                       :disabled="!recurringCandidateTarget || !recurringCandidateTarget.hasRecurringMatch()"
                       @click="clearRecurringMatchFromDialog">
                    {{ tt('Clear Scheduled Match') }}
                </v-btn>
                <v-btn color="secondary" variant="tonal" @click="closeRecurringCandidateDialog">
                    {{ tt('Cancel') }}
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <!-- v6.89: 待标注人工处理弹窗 -->
    <v-dialog width="760" v-model="showAnnotationDialog">
        <v-card class="pa-4">
            <v-card-title class="text-center">
                <h4 class="text-h5">{{ getAnnotationDialogTitle() }}</h4>
            </v-card-title>
            <v-card-text>
                <p class="text-body-2 text-medium-emphasis mb-4">
                    {{ getAnnotationDialogDescription() }}
                </p>
                <div class="d-flex flex-wrap ga-2 mb-4" v-if="annotationReasonSummaries.length > 0">
                    <v-chip v-for="reason in annotationReasonSummaries"
                            :key="reason.key"
                            color="warning"
                            variant="outlined"
                            size="small">
                        {{ reason.label }} × {{ getDisplayCount(reason.count) }}
                    </v-chip>
                </div>
                <v-list class="border rounded" lines="two" max-height="360">
                    <v-list-item v-for="transaction in selectedAnnotationTransactions"
                                 :key="transaction.index"
                                 :title="getAnnotationListTitle(transaction)"
                                 :subtitle="getAnnotationSummary(transaction)">
                        <template #prepend>
                            <v-icon color="warning" :icon="mdiMessageAlertOutline" />
                        </template>
                    </v-list-item>
                </v-list>
            </v-card-text>
            <v-card-actions class="justify-center gap-4 flex-wrap">
                <v-btn color="warning" variant="tonal" @click="openBatchCategoryDialogFromAnnotation">
                    {{ tt('Open Batch Category Editor') }}
                </v-btn>
                <v-btn color="warning" variant="tonal" @click="openBatchAccountDialogFromAnnotation">
                    {{ tt('Open Batch Account Editor') }}
                </v-btn>
                <v-btn color="primary" @click="editFirstAnnotationTransaction">
                    {{ tt('Edit First Transaction') }}
                </v-btn>
                <v-btn color="secondary" variant="tonal" @click="showAnnotationDialog = false">
                    {{ tt('Close') }}
                </v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
</template>

<script setup lang="ts">
import PaginationButtons from '@/components/desktop/PaginationButtons.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import BatchReplaceDialog, { type BatchReplaceDialogDataType } from '../dialogs/BatchReplaceDialog.vue';
import BatchReplaceAllTypesDialog from '../dialogs/BatchReplaceAllTypesDialog.vue';
import BatchCreateDialog, { type BatchCreateDialogDataType } from '../dialogs/BatchCreateDialog.vue';
// v6.34: 导入分类和账户编辑对话框
import CategoryEditDialog from '@/views/desktop/categories/list/dialogs/EditDialog.vue';
import AccountEditDialog from '@/views/desktop/accounts/list/dialogs/EditDialog.vue';

import { ref, computed, useTemplateRef } from 'vue';

import { useI18n } from '@/locales/helpers.ts';

import { useSettingsStore } from '@/stores/setting.ts';
import { useUserStore } from '@/stores/user.ts';
import { useAccountsStore } from '@/stores/account.ts';
import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';

import { type NameValue, type NameNumeralValue, itemAndIndex, reversed, keys } from '@/core/base.ts';
import { type NumeralSystem } from '@/core/numeral.ts';
import { CategoryType } from '@/core/category.ts';
import { TransactionType } from '@/core/transaction.ts';

import { Account, type CategorizedAccountWithDisplayBalance } from '@/models/account.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';
import type { TransactionTag } from '@/models/transaction_tag.ts';
import { ImportTransaction } from '@/models/imported_transaction.ts';

import {
    isString,
    isNumber,
    objectFieldToArrayItem
} from '@/lib/common.ts';
import {
    getUtcOffsetByUtcOffsetMinutes,
    getTimezoneOffsetMinutes
} from '@/lib/datetime.ts';
import { formatCoordinate } from '@/lib/coordinate.ts';
import {
    getAccountMapByName
} from '@/lib/account.ts';
import {
    transactionTypeToCategoryType,
    getSecondaryTransactionMapByName,
    getTransactionPrimaryCategoryName,
    getTransactionSecondaryCategoryName
} from '@/lib/category.ts';
import {
    getCurrentToken
} from '@/lib/userstate.ts';
import { isTransactionFromAIImageRecognitionEnabled } from '@/lib/server_settings.ts';
import logger from '@/lib/logger.ts';

import {
    mdiCheck,
    mdiArrowRight,
    mdiSelectAll,
    mdiSelect,
    mdiSelectInverse,
    mdiPencilOutline,
    mdiAlertOutline,
    mdiPound,
    mdiFindReplace,
    mdiShapePlusOutline,
    mdiTransfer,
    mdiAutoFix,
    mdiSchoolOutline,
    mdiTagMultiple,
    mdiWallet,
    mdiMessageAlertOutline,
    mdiLightbulbOutline,
    mdiChartLine,
    mdiStar
} from '@mdi/js';

type SnackBarType = InstanceType<typeof SnackBar>;
type BatchReplaceDialogType = InstanceType<typeof BatchReplaceDialog>;
type BatchReplaceAllTypesDialogType = InstanceType<typeof BatchReplaceAllTypesDialog>;
type BatchCreateDialogType = InstanceType<typeof BatchCreateDialog>;
// v6.34: 分类和账户编辑对话框类型
type CategoryEditDialogType = InstanceType<typeof CategoryEditDialog>;
type AccountEditDialogType = InstanceType<typeof AccountEditDialog>;

interface ImportTransactionCheckDataFilter {
    minDatetime: number | null; // minDatetime or maxDatetime is null for 'All Date Range', all are not null for 'Custom Date Range'
    maxDatetime: number | null;
    transactionType: TransactionType | null; // null for 'All Transaction Type'
    category: string | null | undefined; // null for 'All Category', undefined for 'Invalid Category'
    account: string | null | undefined; // null for 'All Account', undefined for 'Invalid Account'
    tag: string | null | undefined; // null for 'All Tag', undefined for 'Invalid Tag'
    annotation: string | null; // null=all, 'pending'=needs annotation, 'annotated'=manually annotated, 'no-issues'=no issues
    description: string | null; // null for 'All Description'
}

interface ImportTransactionCheckDataMenuGroup {
    title: string;
    items: ImportTransactionCheckDataMenu[];
}

interface ImportTransactionCheckDataMenu {
    prependIcon?: string;
    title: string;
    subTitle?: string;
    appendIcon?: string;
    disabled?: boolean;
    divider?: boolean;
    onClick: () => void;
}

interface AnnotationReasonSummary {
    key: string;
    label: string;
    count: number;
}

interface RecurringCandidateItem {
    id: string;
    name?: string;
    matchScore?: number;
    matchReasons?: string[];
    matchedOccurrenceDate?: string;
}

const props = defineProps<{
    importTransactions?: ImportTransaction[]
    disabled?: boolean;
    sessionId?: string;  // v6.55: 导入会话ID，用于调用重新分类API
}>();

// v6.55: 定义事件，用于通知父组件数据刷新
const emit = defineEmits<{
    (e: 'reclassified', data: ImportTransaction[]): void;
}>();

const {
    tt,
    getCurrentNumeralSystemType,
    formatUnixTimeToLongDateTime,
    formatAmountToLocalizedNumeralsWithCurrency,
    getCategorizedAccountsWithDisplayBalance
} = useI18n();

const settingsStore = useSettingsStore();
const userStore = useUserStore();
const accountsStore = useAccountsStore();
const transactionCategoriesStore = useTransactionCategoriesStore();
const transactionTagsStore = useTransactionTagsStore();

const snackbar = useTemplateRef<SnackBarType>('snackbar');
const batchReplaceDialog = useTemplateRef<BatchReplaceDialogType>('batchReplaceDialog');
const batchReplaceAllTypesDialog = useTemplateRef<BatchReplaceAllTypesDialogType>('batchReplaceAllTypesDialog');
const batchCreateDialog = useTemplateRef<BatchCreateDialogType>('batchCreateDialog');
// v6.34: 分类和账户编辑对话框引用
const categoryEditDialog = useTemplateRef<CategoryEditDialogType>('categoryEditDialog');
const accountEditDialog = useTemplateRef<AccountEditDialogType>('accountEditDialog');

// v6.34: 分类管理选择对话框状态
const showCategorySelectDialog = ref<boolean>(false);
const manageCategoryType = ref<CategoryType>(CategoryType.Expense);
const manageCategoryId = ref<string>('');

// v6.34: 账户管理选择对话框状态
const showAccountSelectDialog = ref<boolean>(false);
const manageAccountId = ref<string>('');

const editingTransaction = ref<ImportTransaction | null>(null);
const editingTags = ref<string[]>([]);
const filters = ref<ImportTransactionCheckDataFilter>({
    minDatetime: null,
    maxDatetime: null,
    transactionType: null,
    category: null,
    account: null,
    tag: null,
    annotation: null,
    description: null
});

const currentPage = ref<number>(1);
const countPerPage = ref<number>(10);
const showCustomDateRangeDialog = ref<boolean>(false);
const showCustomDescriptionDialog = ref<boolean>(false);
const currentDescriptionFilterValue = ref<string | null>(null);
const showAnnotationDialog = ref<boolean>(false);
const showRecurringCandidateDialog = ref<boolean>(false);
const recurringCandidateLoading = ref<boolean>(false);
const recurringCandidateTarget = ref<ImportTransaction | null>(null);
const recurringCandidates = ref<RecurringCandidateItem[]>([]);
const selectedRecurringCandidateId = ref<string>('');

// 批量编辑对话框状态和数据
const showBatchCategoryDialog = ref<boolean>(false);
const showBatchAccountDialog = ref<boolean>(false);
const batchCategoryType = ref<number>(TransactionType.Expense);
const batchCategoryId = ref<string>('');
const batchAccountId = ref<string>('');

const numeralSystem = computed<NumeralSystem>(() => getCurrentNumeralSystemType());
const showAccountBalance = computed<boolean>(() => settingsStore.appSettings.showAccountBalance);
const currentTimezoneOffsetMinutes = computed<number>(() => getTimezoneOffsetMinutes(settingsStore.appSettings.timeZone));

const defaultCurrency = computed<string>(() => userStore.currentUserDefaultCurrency);
const coordinateDisplayType = computed<number>(() => userStore.currentUserCoordinateDisplayType);

const allAccounts = computed<Account[]>(() => accountsStore.allPlainAccounts);
const allVisibleAccounts = computed<Account[]>(() => accountsStore.allVisiblePlainAccounts);
const allVisibleCategorizedAccounts = computed<CategorizedAccountWithDisplayBalance[]>(() => getCategorizedAccountsWithDisplayBalance(allVisibleAccounts.value, showAccountBalance.value));
const allAccountsMap = computed<Record<string, Account>>(() => accountsStore.allAccountsMap);
const allAccountsMapByName = computed<Record<string, Account>>(() => getAccountMapByName(accountsStore.allAccounts));
const allCategories = computed<Record<number, TransactionCategory[]>>(() => transactionCategoriesStore.allTransactionCategories);
const allCategoriesMap = computed<Record<string, TransactionCategory>>(() => transactionCategoriesStore.allTransactionCategoriesMap);
const allTags = computed<TransactionTag[]>(() => transactionTagsStore.allTransactionTags);
const allTagsMap = computed<Record<string, TransactionTag>>(() => transactionTagsStore.allTransactionTagsMap);
const aiAnnotationEnabled = computed<boolean>(() => isTransactionFromAIImageRecognitionEnabled());

function getAnnotationTextKey(baseKey: string, aiKey: string): string {
    return aiAnnotationEnabled.value ? aiKey : baseKey;
}

function getNeedsAnnotationText(): string {
    return tt(getAnnotationTextKey('Needs Annotation', 'Needs AI Annotation'));
}

function getSelectAllAnnotationText(): string {
    return tt(getAnnotationTextKey('Select All Needs Annotation', 'Select All Needs AI Annotation'));
}

function getAnnotationActionText(): string {
    return tt(getAnnotationTextKey('Request Annotation', 'AI Annotation'));
}

function getAnnotationFilterTitle(): string {
    return tt(getAnnotationTextKey('Annotation', 'AI Annotation'));
}

function getNoAnnotationIssuesText(): string {
    return tt(getAnnotationTextKey('No Annotation Issues', 'No AI Annotation Issues'));
}

function getManuallyAnnotatedText(): string {
    return tt('Manually Annotated');
}

function getAnnotationDialogTitle(): string {
    return tt(getAnnotationTextKey('Review Annotation Queue', 'Review AI Annotation Queue'));
}

function getAnnotationDialogDescription(): string {
    return tt(getAnnotationTextKey(
        'Selected transactions need manual review before import',
        'Selected transactions need AI-assisted review before import'
    ));
}

function getNoSelectedAnnotationText(): string {
    return tt(getAnnotationTextKey(
        'No selected transactions require annotation',
        'No selected transactions require AI annotation'
    ));
}

// 根据交易类型获取对应的分类列表（统一函数，避免多个 v-if 分支导致的 Vue 渲染问题）
function getCategoriesForType(type: number): TransactionCategory[] {
    const typeToCategory: Record<number, number> = {
        [TransactionType.Expense]: CategoryType.Expense,
        [TransactionType.Income]: CategoryType.Income,
        [TransactionType.Transfer]: CategoryType.Transfer,
        [TransactionType.Investment]: CategoryType.Investment
    };
    const categoryType = typeToCategory[type];
    return categoryType !== undefined ? (allCategories.value[categoryType] || []) : [];
}

// 检查指定交易类型是否有可用的分类
function hasAvailableCategoriesForType(type: number): boolean {
    const categories = getCategoriesForType(type);
    return categories.some(cat => !cat.hidden);
}

// 获取分类主文本（处理空 categoryId 的情况，避免触发大量警告日志）
function getCategoryPrimaryText(item: ImportTransaction): string {
    // 如果 categoryId 为空或 '0'，返回空字符串避免触发查找
    if (!item.categoryId || item.categoryId === '0') {
        return '';
    }
    return getTransactionPrimaryCategoryName(item.categoryId, getCategoriesForType(item.type));
}

// 获取分类次要文本（处理空 categoryId 的情况）
function getCategorySecondaryText(item: ImportTransaction): string {
    if (!item.categoryId || item.categoryId === '0') {
        return '';
    }
    return getTransactionSecondaryCategoryName(item.categoryId, getCategoriesForType(item.type));
}

/**
 * 判断交易是否需要双账户（转账或投资类型）
 * 转账：资金从一个账户转到另一个账户
 * 投资：资金从一个账户投入到投资账户（如余额宝、股票账户等）
 */
function requiresDestinationAccount(item: ImportTransaction): boolean {
    return item.type === TransactionType.Transfer || item.type === TransactionType.Investment;
}

/**
 * 获取目标账户标题（根据交易类型返回不同文案）
 */
function getDestinationAccountTitle(item: ImportTransaction): string {
    if (item.type === TransactionType.Investment) {
        return tt('Investment Account');
    }
    return tt('Destination Account');
}

// 批量编辑分类：交易类型选项
const batchCategoryTypeOptions = computed<NameNumeralValue[]>(() => [
    { name: tt('Expense'), value: TransactionType.Expense },
    { name: tt('Income'), value: TransactionType.Income },
    { name: tt('Transfer'), value: TransactionType.Transfer },
    { name: tt('Investment'), value: TransactionType.Investment }
]);

// v6.77: 单行编辑交易类型选项（用于预览表格中的类型编辑）
const transactionTypeOptions = computed<{ text: string; value: number }[]>(() => [
    { text: tt('Expense'), value: TransactionType.Expense },
    { text: tt('Income'), value: TransactionType.Income },
    { text: tt('Transfer'), value: TransactionType.Transfer },
    { text: tt('Investment'), value: TransactionType.Investment }
]);

/**
 * v6.77: 当交易类型改变时的处理函数
 * 重置分类ID（因为不同类型对应不同的分类）
 * @param item 被编辑的交易
 */
function onTransactionTypeChange(item: ImportTransaction): void {
    logger.info(`[类型变更] 交易类型从旧值变更为 ${item.type}，重置分类选择`);
    // 清空分类ID，因为不同交易类型对应不同的分类列表
    item.categoryId = '';
    if (item.hasRecurringMatch()) {
        item.clearRecurringMatch();
    }
}

function clearRecurringMatch(item: ImportTransaction): void {
    item.clearRecurringMatch(false);
    logger.info(`[定时匹配] 已清除自动匹配 index=${item.index}`);
}

function closeRecurringCandidateDialog(): void {
    showRecurringCandidateDialog.value = false;
    recurringCandidateLoading.value = false;
    recurringCandidateTarget.value = null;
    recurringCandidates.value = [];
    selectedRecurringCandidateId.value = '';
}

async function openRecurringCandidateDialog(item: ImportTransaction): Promise<void> {
    const previewId = (item as { _previewId?: number })._previewId;
    if (!previewId) {
        snackbar.value?.showMessage(tt('No preview ID available'));
        return;
    }

    recurringCandidateTarget.value = item;
    recurringCandidates.value = [];
    selectedRecurringCandidateId.value = item.recurringTemplateId || '';
    showRecurringCandidateDialog.value = true;
    recurringCandidateLoading.value = true;

    try {
        const token = getCurrentToken();
        const headers: Record<string, string> = {};
        if (token) {
            headers['Authorization'] = `Bearer ${token}`;
        }

        const response = await fetch(`/api/bills/import/v2/preview-item/${previewId}/recurring-candidates`, {
            method: 'GET',
            headers: headers
        });

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`Load recurring candidates failed: ${response.status} ${errorText}`);
        }

        const result = await response.json();
        if (!result.success) {
            throw new Error(result.error || 'Unknown error');
        }

        recurringCandidates.value = (result.result?.candidates || []) as RecurringCandidateItem[];
        selectedRecurringCandidateId.value = item.recurringTemplateId
            || String(result.result?.linkedRecurringId || '')
            || (recurringCandidates.value[0] ? String(recurringCandidates.value[0].id) : '');
    } catch (error) {
        logger.error(`[定时候选] 加载失败: ${error}`);
        snackbar.value?.showMessage(tt('Load Scheduled Candidates Failed'));
        closeRecurringCandidateDialog();
    } finally {
        recurringCandidateLoading.value = false;
    }
}

function formatRecurringCandidateSubtitle(candidate: RecurringCandidateItem): string {
    const reasons = Array.isArray(candidate.matchReasons)
        ? candidate.matchReasons.join(' | ')
        : '';
    return [
        candidate.matchedOccurrenceDate ? `${tt('Matched Date')}: ${candidate.matchedOccurrenceDate}` : '',
        reasons ? `${tt('Match Reasons')}: ${reasons}` : ''
    ].filter(text => !!text).join(' · ');
}

function isBestRecurringCandidate(candidate: RecurringCandidateItem): boolean {
    const bestCandidate = recurringCandidates.value[0];
    if (!bestCandidate) {
        return false;
    }

    return String(bestCandidate.id) === String(candidate.id);
}

function getRecurringCandidatePrimaryReason(candidate: RecurringCandidateItem): string {
    if (!Array.isArray(candidate.matchReasons) || candidate.matchReasons.length < 1) {
        return '';
    }

    return String(candidate.matchReasons[0] || '');
}

function getPrimaryRecurringReason(item: ImportTransaction): string {
    if (!item.recurringMatchReasons) {
        return '';
    }

    return item.recurringMatchReasons.split('|').map(text => text.trim()).filter(text => !!text)[0] || '';
}

function applySelectedRecurringCandidate(): void {
    if (!recurringCandidateTarget.value || !selectedRecurringCandidateId.value) {
        return;
    }

    const matchedCandidate = recurringCandidates.value.find(
        candidate => String(candidate.id) === selectedRecurringCandidateId.value
    );
    if (!matchedCandidate) {
        return;
    }

    recurringCandidateTarget.value.recurringTemplateId = String(matchedCandidate.id);
    recurringCandidateTarget.value.recurringTemplateName = matchedCandidate.name || '';
    recurringCandidateTarget.value.recurringCandidateCount = recurringCandidates.value.length;
    recurringCandidateTarget.value.recurringMatchScore = Number(matchedCandidate.matchScore || 0);
    recurringCandidateTarget.value.recurringMatchReasons = Array.isArray(matchedCandidate.matchReasons)
        ? matchedCandidate.matchReasons.join('|')
        : '';
    recurringCandidateTarget.value.recurringMatchedDate = matchedCandidate.matchedOccurrenceDate || '';

    logger.info(`[定时候选] 已切换定时匹配 index=${recurringCandidateTarget.value.index}, recurringId=${matchedCandidate.id}`);
    closeRecurringCandidateDialog();
}

function clearRecurringMatchFromDialog(): void {
    if (!recurringCandidateTarget.value) {
        return;
    }

    clearRecurringMatch(recurringCandidateTarget.value);
    closeRecurringCandidateDialog();
}

function getRecurringMatchSummary(item: ImportTransaction): string {
    return [
        item.recurringTemplateName,
        item.recurringMatchedDate,
        item.recurringMatchReasons,
        item.recurringMatchScore ? `score=${item.recurringMatchScore}` : ''
    ].filter(text => !!text).join(' | ');
}

function applySuggestedType(item: ImportTransaction): void {
    if (!item.hasTransferSuggestion() || !item.suggestedType) {
        return;
    }

    item.type = item.suggestedType;
    onTransactionTypeChange(item);
    logger.info(
        `[导入推荐] 应用类型推荐 index=${item.index}, suggestedType=${item.suggestedType}, score=${item.transferSuggestionScore}`
    );
}

const PARSER_LABELS: Record<string, string> = {
    wechat: '微信',
    alipay: '支付宝',
    icbc: '工商银行',
    cmbc: '招商银行',
    abc: '农业银行',
    ccb: '建设银行',
    generic: '通用',
};

const PARSER_COLORS: Record<string, string> = {
    wechat: 'green',
    alipay: 'blue',
    icbc: 'red',
    cmbc: 'orange',
    abc: 'teal',
    ccb: 'indigo',
    generic: 'grey',
};

function getParserLabel(parserId: string): string {
    return PARSER_LABELS[parserId] || parserId;
}

function getParserColor(parserId: string): string {
    return PARSER_COLORS[parserId] || 'grey';
}

function getAnnotationIssues(item: ImportTransaction): string[] {
    const reasons: string[] = [];

    if (item.type !== TransactionType.ModifyBalance && (!item.categoryId || item.categoryId === '0')) {
        reasons.push(tt('Missing Category'));
    }

    if (!item.sourceAccountId || item.sourceAccountId === '0') {
        reasons.push(tt('Missing Source Account'));
    }

    if (requiresDestinationAccount(item) && (!item.destinationAccountId || item.destinationAccountId === '0')) {
        reasons.push(tt('Missing Destination Account'));
    }

    if (requiresDestinationAccount(item)
        && item.sourceAccountId
        && item.destinationAccountId
        && item.sourceAccountId !== '0'
        && item.destinationAccountId !== '0'
        && item.sourceAccountId === item.destinationAccountId) {
        reasons.push(tt('Review Transfer Accounts'));
    }

    return reasons;
}

function needsAnnotation(item: ImportTransaction): boolean {
    return getAnnotationIssues(item).length > 0;
}

function getAnnotationSummary(item: ImportTransaction): string {
    return getAnnotationIssues(item).join(' · ');
}

function getAnnotationListTitle(item: ImportTransaction): string {
    const description = item.comment || item.counterparty || item.paymentMethod || tt('No description');
    return `${getDisplayDateTime(item)} · ${description}`;
}

// 批量编辑分类：根据选中的交易类型获取分类列表
function getBatchCategoryItems(): TransactionCategory[] {
    return getCategoriesForType(batchCategoryType.value);
}

// 批量编辑账户：可用账户列表
const availableAccounts = computed<Account[]>(() => allVisibleAccounts.value);

/**
 * v6.55: 重新分类所有预览账单
 * 调用后端 /api/bills/import/v2/reclassify/{session_id} 端点
 * 后端会：
 * 1. 刷新分类规则
 * 2. 按照 dedup_type 分离账单
 * 3. 对不同类型使用不同的分类规则
 * 4. 更新预览表中的分类和账户信息
 * 5. 返回更新后的完整预览数据
 */
async function reclassifySelected(): Promise<void> {
    if (editingTransaction.value) {
        editingTransaction.value.tagIds = editingTags.value;
        updateTransactionData(editingTransaction.value);
    }

    // 检查 session_id 是否存在
    if (!props.sessionId) {
        logger.error('[重新分类] 缺少 sessionId');
        snackbar.value?.showMessage('No session ID available');
        return;
    }

    logger.info(`[重新分类] 开始重新分类，session_id=${props.sessionId}`);

    try {
        // 获取认证token
        const token = getCurrentToken();
        const headers: Record<string, string> = {
            'Content-Type': 'application/json'
        };
        if (token) {
            headers['Authorization'] = `Bearer ${token}`;
        }

        const previewUpdates = buildSelectedPreviewUpdates();

        // 调用 v6.55 新增的 reclassify API
        const response = await fetch(`/api/bills/import/v2/reclassify/${props.sessionId}`, {
            method: 'POST',
            headers: headers,
            body: JSON.stringify({
                preview_updates: previewUpdates
            })
        });

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`Reclassify failed: ${response.status} ${errorText}`);
        }

        const result = await response.json();

        if (!result.success) {
            throw new Error(result.error || 'Unknown error');
        }

        logger.info(`[重新分类] 后端返回成功，preview数量: ${result.data?.preview?.length || 0}, ` +
            `session_samples_saved=${result.data?.session_samples_saved || 0}, annotation_applied=${result.data?.annotation_applied || 0}`);

        // 通知父组件使用新数据
        // 父组件 ImportDialog.vue 监听 @reclassified 事件并更新 importTransactions
        if (result.data?.preview && result.data.preview.length > 0) {
            emit('reclassified', result.data.preview);
            snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                count: getDisplayCount(result.data.preview.length)
            });
        } else {
            snackbar.value?.showMessage('No transactions updated');
        }

        logger.info('[重新分类] 完成');

    } catch (error) {
        logger.error(`[重新分类] 失败: ${error}`);
        snackbar.value?.showMessage(`Reclassify failed: ${error}`);
    }
}

function buildSelectedPreviewUpdates(): Record<string, unknown>[] {
    const selectedTransactions = (props.importTransactions || []).filter(transaction => transaction.selected);
    const typeReverseMap: Record<number, string> = {
        2: '收入',
        3: '支出',
        4: '转账',
        5: '投资'
    };

    return selectedTransactions.map(transaction => {
        return {
            id: (transaction as { _previewId?: number })._previewId,
            preview_type: typeReverseMap[transaction.type] || '支出',
            preview_amount: transaction.sourceAmount / 100,
            preview_destination_amount: transaction.destinationAmount / 100,
            preview_source_account_id: transaction.sourceAccountId ? parseInt(transaction.sourceAccountId, 10) : null,
            preview_destination_account_id: transaction.destinationAccountId ? parseInt(transaction.destinationAccountId, 10) : null,
            preview_recurring_id: transaction.recurringTemplateId ? parseInt(transaction.recurringTemplateId, 10) : null,
            preview_recurring_name: transaction.recurringTemplateName || '',
            preview_recurring_candidate_count: transaction.recurringCandidateCount || 0,
            preview_recurring_match_score: transaction.recurringMatchScore || 0,
            preview_recurring_match_reasons: transaction.recurringMatchReasons || '',
            preview_recurring_matched_date: transaction.recurringMatchedDate || '',
            category_id: transaction.categoryId ? parseInt(transaction.categoryId, 10) : null,
            selected: transaction.selected
        };
    }).filter(item => !!item.id);
}

async function promoteSelectedToLongTermLearning(): Promise<void> {
    if (editingTransaction.value) {
        editingTransaction.value.tagIds = editingTags.value;
        updateTransactionData(editingTransaction.value);
    }

    if (!props.sessionId) {
        snackbar.value?.showMessage('No session ID available');
        return;
    }

    try {
        const token = getCurrentToken();
        const headers: Record<string, string> = {
            'Content-Type': 'application/json'
        };
        if (token) {
            headers['Authorization'] = `Bearer ${token}`;
        }

        const previewUpdates = buildSelectedPreviewUpdates();
        const response = await fetch(`/api/bills/import/v2/learning/${props.sessionId}/promote`, {
            method: 'POST',
            headers: headers,
            body: JSON.stringify({ preview_updates: previewUpdates })
        });

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`Promote failed: ${response.status} ${errorText}`);
        }

        const result = await response.json();
        if (!result.success) {
            throw new Error(result.error || 'Unknown error');
        }

        snackbar.value?.showMessage(
            tt('Long-term learning saved: {count} rules', {
                count: result.data?.rules_total || 0
            })
        );
    } catch (error) {
        logger.error(`[长期学习提升] 失败: ${error}`);
        snackbar.value?.showMessage(`Promote failed: ${error}`);
    }
}

// v6.34: 打开分类选择对话框
function openCategoryManagement(): void {
    manageCategoryId.value = '';
    manageCategoryType.value = CategoryType.Expense;
    showCategorySelectDialog.value = true;
}

// v6.34: 打开账户选择对话框
function openAccountManagement(): void {
    manageAccountId.value = '';
    showAccountSelectDialog.value = true;
}

// v6.34: 获取当前选中类型的分类列表
function getManageCategoryItems(): TransactionCategory[] {
    return allCategories.value[manageCategoryType.value] || [];
}

// v6.34: 获取所有显示账户（用于选择器）
const allDisplayAccounts = computed<Account[]>(() => {
    const allAccounts: Account[] = [];
    const categorizedAccounts = getCategorizedAccountsWithDisplayBalance(
        accountsStore.allAccounts,
        false  // 不显示余额
    );

    for (const category of categorizedAccounts) {
        for (const account of category.accounts) {
            allAccounts.push(account);
        }
    }

    return allAccounts;
});

// v6.34: 打开选中分类的编辑对话框
function openSelectedCategoryEditDialog(): void {
    if (!manageCategoryId.value) return;

    showCategorySelectDialog.value = false;

    // 查找选中的分类
    const category = allCategoriesMap.value[manageCategoryId.value];
    if (category) {
        categoryEditDialog.value?.open({
            id: manageCategoryId.value,
            type: manageCategoryType.value,
            currentCategory: category
        }).then(() => {
            // 编辑完成后刷新分类数据
            transactionCategoriesStore.loadAllCategories({ force: true });
        }).catch((error: unknown) => {
            if (error && typeof error === 'object' && 'processed' in error && !(error as { processed: boolean }).processed) {
                logger.error(`[分类编辑] 失败: ${error}`);
            }
        });
    }
}

// v6.34: 打开选中账户的编辑对话框
function openSelectedAccountEditDialog(): void {
    if (!manageAccountId.value) return;

    showAccountSelectDialog.value = false;

    // 查找选中的账户
    const account = allDisplayAccounts.value.find(acc => acc.id === manageAccountId.value);
    if (account) {
        accountEditDialog.value?.open({
            id: manageAccountId.value,
            currentAccount: account
        }).then(() => {
            // 编辑完成后刷新账户数据
            accountsStore.loadAllAccounts({ force: true });
        }).catch((error: unknown) => {
            if (error && typeof error === 'object' && 'processed' in error && !(error as { processed: boolean }).processed) {
                logger.error(`[账户编辑] 失败: ${error}`);
            }
        });
    }
}

// 应用批量分类修改
function applyBatchCategory(): void {
    if (!props.importTransactions || !batchCategoryId.value) return;

    let updatedCount = 0;

    for (const importTransaction of props.importTransactions) {
        if (!importTransaction.selected) continue;

        // 更新交易类型和分类ID
        importTransaction.type = batchCategoryType.value;
        importTransaction.categoryId = batchCategoryId.value;

        // 更新分类名称显示
        const category = allCategoriesMap.value[batchCategoryId.value];
        if (category) {
            importTransaction.actualCategoryName = category.name;
            importTransaction.originalCategoryName = category.name;
        }

        importTransaction.isManuallyAnnotated = true;
        updateTransactionData(importTransaction);
        updatedCount++;
    }

    showBatchCategoryDialog.value = false;
    batchCategoryId.value = '';

    if (updatedCount > 0) {
        snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
            count: getDisplayCount(updatedCount)
        });
    }
}

// 应用批量账户修改
function applyBatchAccount(): void {
    if (!props.importTransactions || !batchAccountId.value) return;

    let updatedCount = 0;

    for (const importTransaction of props.importTransactions) {
        if (!importTransaction.selected) continue;

        // 更新账户ID
        importTransaction.sourceAccountId = batchAccountId.value;

        // 更新账户名称显示
        const account = allAccountsMap.value[batchAccountId.value];
        if (account) {
            importTransaction.actualSourceAccountName = account.name;
            importTransaction.originalSourceAccountName = account.name;
        }

        importTransaction.isManuallyAnnotated = true;
        updateTransactionData(importTransaction);
        updatedCount++;
    }

    showBatchAccountDialog.value = false;
    batchAccountId.value = '';

    if (updatedCount > 0) {
        snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
            count: getDisplayCount(updatedCount)
        });
    }
}

const isEditing = computed<boolean>(() => !!editingTransaction.value);
const canImport = computed<boolean>(() => selectedImportTransactionCount.value > 0 && selectedInvalidTransactionCount.value < 1);

const filterMenus = computed<ImportTransactionCheckDataMenuGroup[]>(() => [
    {
        title: tt('Date Range'),
        items: [
            {
                title: tt('All'),
                appendIcon: filters.value.minDatetime === null || filters.value.maxDatetime === null ? mdiCheck : undefined,
                onClick: () => {
                    filters.value.minDatetime = null;
                    filters.value.maxDatetime = null;
                }
            },
            {
                title: tt('Custom'),
                subTitle: displayFilterCustomDateRange.value,
                appendIcon: filters.value.minDatetime !== null && filters.value.maxDatetime !== null ? mdiCheck : undefined,
                onClick: () => showCustomDateRangeDialog.value = true
            }
        ]
    },
    {
        title: tt('Type'),
        items: [
            {
                title: tt('All'),
                appendIcon: filters.value.transactionType === null ? mdiCheck : undefined,
                onClick: () => filters.value.transactionType = null
            },
            {
                title: tt('Income'),
                appendIcon: filters.value.transactionType === TransactionType.Income ? mdiCheck : undefined,
                onClick: () => filters.value.transactionType = TransactionType.Income
            },
            {
                title: tt('Expense'),
                appendIcon: filters.value.transactionType === TransactionType.Expense ? mdiCheck : undefined,
                onClick: () => filters.value.transactionType = TransactionType.Expense
            },
            {
                title: tt('Transfer'),
                appendIcon: filters.value.transactionType === TransactionType.Transfer ? mdiCheck : undefined,
                onClick: () => filters.value.transactionType = TransactionType.Transfer
            },
            {
                title: tt('Investment'),
                appendIcon: filters.value.transactionType === TransactionType.Investment ? mdiCheck : undefined,
                onClick: () => filters.value.transactionType = TransactionType.Investment
            }
        ]
    },
    {
        title: tt('Category'),
        items: [
            {
                title: tt('All'),
                appendIcon: filters.value.category === null ? mdiCheck : undefined,
                onClick: () => filters.value.category = null
            },
            {
                title: tt('Invalid Category'),
                appendIcon: filters.value.category === undefined ? mdiCheck : undefined,
                onClick: () => filters.value.category = undefined
            },
            {
                title: tt('None'),
                appendIcon: filters.value.category === '' ? mdiCheck : undefined,
                onClick: () => filters.value.category = ''
            },
            ...allUsedCategoryNames.value.map(name => ({
                title: name,
                appendIcon: filters.value.category === name ? mdiCheck : undefined,
                onClick: () => filters.value.category = name
            }))
        ]
    },
    {
        title: tt('Account'),
        items: [
            {
                title: tt('All'),
                appendIcon: filters.value.account === null ? mdiCheck : undefined,
                onClick: () => filters.value.account = null
            },
            {
                title: tt('Invalid Account'),
                appendIcon: filters.value.account === undefined ? mdiCheck : undefined,
                onClick: () => filters.value.account = undefined
            },
            {
                title: tt('None'),
                appendIcon: filters.value.account === '' ? mdiCheck : undefined,
                onClick: () => filters.value.account = ''
            },
            ...allUsedAccountNames.value.map(name => ({
                title: name,
                appendIcon: filters.value.account === name ? mdiCheck : undefined,
                onClick: () => filters.value.account = name
            }))
        ]
    },
    {
        title: tt('Tags'),
        items: [
            {
                title: tt('All'),
                appendIcon: filters.value.tag === null ? mdiCheck : undefined,
                onClick: () => filters.value.tag = null
            },
            {
                title: tt('Invalid Tag'),
                appendIcon: filters.value.tag === undefined ? mdiCheck : undefined,
                onClick: () => filters.value.tag = undefined
            },
            {
                title: tt('None'),
                appendIcon: filters.value.tag === '' ? mdiCheck : undefined,
                onClick: () => filters.value.tag = ''
            },
            ...allUsedTagNames.value.map(name => ({
                title: name,
                appendIcon: filters.value.tag === name ? mdiCheck : undefined,
                onClick: () => filters.value.tag = name
            }))
        ]
    },
    {
        title: getAnnotationFilterTitle(),
        items: [
            {
                title: tt('All'),
                appendIcon: filters.value.annotation === null ? mdiCheck : undefined,
                onClick: () => filters.value.annotation = null
            },
            {
                title: getNeedsAnnotationText(),
                appendIcon: filters.value.annotation === 'pending' ? mdiCheck : undefined,
                onClick: () => filters.value.annotation = 'pending'
            },
            {
                title: getManuallyAnnotatedText(),
                appendIcon: filters.value.annotation === 'annotated' ? mdiCheck : undefined,
                onClick: () => filters.value.annotation = 'annotated'
            },
            {
                title: getNoAnnotationIssuesText(),
                appendIcon: filters.value.annotation === 'no-issues' ? mdiCheck : undefined,
                onClick: () => filters.value.annotation = 'no-issues'
            }
        ]
    },
    {
        title: tt('Description'),
        items: [
            {
                title: tt('All'),
                appendIcon: filters.value.description === null ? mdiCheck : undefined,
                onClick: () => filters.value.description = null
            },
            {
                title: tt('None'),
                appendIcon: filters.value.description === '' ? mdiCheck : undefined,
                onClick: () => filters.value.description = ''
            },
            {
                title: tt('Custom'),
                subTitle: filters.value.description !== null ? filters.value.description : undefined,
                appendIcon: filters.value.description !== null && filters.value.description !== '' ? mdiCheck : undefined,
                onClick: () => {
                    currentDescriptionFilterValue.value = filters.value.description || '';
                    showCustomDescriptionDialog.value = true;
                }
            }
        ]
    }
]);

const toolMenus = computed<ImportTransactionCheckDataMenu[]>(() => [
    {
        prependIcon: mdiFindReplace,
        title: tt('Batch Replace Selected Expense Categories'),
        disabled: isEditing.value || selectedExpenseTransactionCount.value < 1,
        onClick: () => showBatchReplaceDialog('expenseCategory')
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Batch Replace Selected Income Categories'),
        disabled: isEditing.value || selectedIncomeTransactionCount.value < 1,
        onClick: () => showBatchReplaceDialog('incomeCategory')
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Batch Replace Selected Transfer Categories'),
        disabled: isEditing.value || selectedTransferTransactionCount.value < 1,
        onClick: () => showBatchReplaceDialog('transferCategory')
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Batch Replace Selected Accounts'),
        disabled: isEditing.value || selectedImportTransactionCount.value < 1,
        onClick: () => showBatchReplaceDialog('account')
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Batch Replace Selected Destination Accounts'),
        disabled: isEditing.value || selectedTransferTransactionCount.value < 1,
        onClick: () => showBatchReplaceDialog('destinationAccount')
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Batch Replace Selected Transaction Tags'),
        disabled: isEditing.value || selectedImportTransactionCount.value < 1,
        onClick: () => showBatchReplaceDialog('tag', allOriginalTransactionTagNames.value)
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Batch Add Transaction Tags'),
        disabled: isEditing.value || selectedImportTransactionCount.value < 1,
        onClick: () => showBatchAddDialog('tag')
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Replace Invalid Expense Categories'),
        disabled: isEditing.value || !allInvalidExpenseCategoryNames.value || allInvalidExpenseCategoryNames.value.length < 1,
        divider: true,
        onClick: () => showReplaceInvalidItemDialog('expenseCategory', allInvalidExpenseCategoryNames.value)
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Replace Invalid Income Categories'),
        disabled: isEditing.value || !allInvalidIncomeCategoryNames.value || allInvalidIncomeCategoryNames.value.length < 1,
        onClick: () => showReplaceInvalidItemDialog('incomeCategory', allInvalidIncomeCategoryNames.value)
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Replace Invalid Transfer Categories'),
        disabled: isEditing.value || !allInvalidTransferCategoryNames.value || allInvalidTransferCategoryNames.value.length < 1,
        onClick: () => showReplaceInvalidItemDialog('transferCategory', allInvalidTransferCategoryNames.value)
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Replace Invalid Accounts'),
        disabled: isEditing.value || !allInvalidAccountNames.value || allInvalidAccountNames.value.length < 1,
        onClick: () => showReplaceInvalidItemDialog('account', allInvalidAccountNames.value)
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Replace Invalid Transaction Tags'),
        disabled: isEditing.value || !allInvalidTransactionTagNames.value || allInvalidTransactionTagNames.value.length < 1,
        onClick: () => showReplaceInvalidItemDialog('tag', allInvalidTransactionTagNames.value)
    },
    {
        prependIcon: mdiFindReplace,
        title: tt('Batch Replace Categories / Accounts / Tags'),
        disabled: isEditing.value,
        divider: true,
        onClick: showReplaceAllTypesDialog
    },
    {
        prependIcon: mdiShapePlusOutline,
        title: tt('Create Nonexistent Expense Categories'),
        disabled: isEditing.value || !allInvalidExpenseCategoryNames.value || allInvalidExpenseCategoryNames.value.length < 1,
        divider: true,
        onClick: () => showBatchCreateInvalidItemDialog('expenseCategory', allInvalidExpenseCategoryNames.value)
    },
    {
        prependIcon: mdiShapePlusOutline,
        title: tt('Create Nonexistent Income Categories'),
        disabled: isEditing.value || !allInvalidIncomeCategoryNames.value || allInvalidIncomeCategoryNames.value.length < 1,
        onClick: () => showBatchCreateInvalidItemDialog('incomeCategory', allInvalidIncomeCategoryNames.value)
    },
    {
        prependIcon: mdiShapePlusOutline,
        title: tt('Create Nonexistent Transfer Categories'),
        disabled: isEditing.value || !allInvalidTransferCategoryNames.value || allInvalidTransferCategoryNames.value.length < 1,
        onClick: () => showBatchCreateInvalidItemDialog('transferCategory', allInvalidTransferCategoryNames.value)
    },
    {
        prependIcon: mdiShapePlusOutline,
        title: tt('Create Nonexistent Transaction Tags'),
        disabled: isEditing.value || !allInvalidTransactionTagNames.value || allInvalidTransactionTagNames.value.length < 1,
        onClick: () => showBatchCreateInvalidItemDialog('tag', allInvalidTransactionTagNames.value)
    },
    {
        prependIcon: mdiTransfer,
        title: tt('Batch Convert Expense Transaction to Income Transaction'),
        disabled: isEditing.value || selectedExpenseTransactionCount.value < 1,
        divider: true,
        onClick: () => convertTransactionType(TransactionType.Expense, TransactionType.Income)
    },
    {
        prependIcon: mdiTransfer,
        title: tt('Batch Convert Expense Transaction to Transfer Transaction'),
        disabled: isEditing.value || selectedExpenseTransactionCount.value < 1,
        onClick: () => convertTransactionType(TransactionType.Expense, TransactionType.Transfer)
    },
    {
        prependIcon: mdiTransfer,
        title: tt('Batch Convert Income Transaction to Expense Transaction'),
        disabled: isEditing.value || selectedIncomeTransactionCount.value < 1,
        onClick: () => convertTransactionType(TransactionType.Income, TransactionType.Expense)
    },
    {
        prependIcon: mdiTransfer,
        title: tt('Batch Convert Income Transaction to Transfer Transaction'),
        disabled: isEditing.value || selectedIncomeTransactionCount.value < 1,
        onClick: () => convertTransactionType(TransactionType.Income, TransactionType.Transfer)
    },
    {
        prependIcon: mdiTransfer,
        title: tt('Batch Convert Transfer Transaction to Expense Transaction'),
        disabled: isEditing.value || selectedTransferTransactionCount.value < 1,
        onClick: () => convertTransactionType(TransactionType.Transfer, TransactionType.Expense)
    },
    {
        prependIcon: mdiTransfer,
        title: tt('Batch Convert Transfer Transaction to Income Transaction'),
        disabled: isEditing.value || selectedTransferTransactionCount.value < 1,
        onClick: () => convertTransactionType(TransactionType.Transfer, TransactionType.Income)
    },
    {
        prependIcon: mdiAutoFix,
        title: tt('Clear Selected Scheduled Matches'),
        disabled: isEditing.value || selectedRecurringMatchCount.value < 1,
        divider: true,
        onClick: clearSelectedRecurringMatches
    }
]);

const importTransactionsTableHeight = computed<number | undefined>(() => {
    if (countPerPage.value <= 10 || !props.importTransactions || props.importTransactions.length <= 10) {
        return undefined;
    } else {
        return 400;
    }
});

const importTransactionHeaders = computed<object[]>(() => {
    return [
        { value: 'valid', sortable: true, nowrap: true, width: 35 },
        { value: 'time', title: tt('Transaction Time'), sortable: true, nowrap: true, maxWidth: 280 },
        { value: 'type', title: tt('Type'), sortable: true, nowrap: true, maxWidth: 140 },
        { value: 'actualCategoryName', title: tt('Category'), sortable: true, nowrap: true },
        { value: 'sourceAmount', title: tt('Amount'), sortable: true, nowrap: true },
        { value: 'actualSourceAccountName', title: tt('Account'), sortable: true, nowrap: true },
        { value: 'geoLocation', title: tt('Geographic Location'), sortable: true, nowrap: true },
        { value: 'tagIds', title: tt('Tags'), sortable: true, nowrap: true },
        // v6.33: 交易对方和支付方式列移到标签列之后
        { value: 'counterparty', title: tt('Counterparty'), sortable: true, nowrap: true },
        { value: 'paymentMethod', title: tt('Payment Method'), sortable: true, nowrap: true },
        { value: 'parserSource', title: '解析器', sortable: true, nowrap: true },
        { value: 'comment', title: tt('Description'), sortable: true, nowrap: true },
    ];
});

const importTransactionsTablePageOptions = computed<NameNumeralValue[]>(() => getTablePageOptions(props.importTransactions?.length));

const totalPageCount = computed<number>(() => {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return 1;
    }

    let count = 0;

    for (const importTransaction of props.importTransactions) {
        if (isTransactionDisplayed(importTransaction)) {
            count++;
        }
    }

    return Math.ceil(count / countPerPage.value);
});

const currentPageTransactions = computed<ImportTransaction[]>(() => {
    const ret: ImportTransaction[] = [];

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return ret;
    }

    const previousCount = Math.max(0, (currentPage.value - 1) * countPerPage.value);
    let count = 0;

    for (const importTransaction of props.importTransactions) {
        if (ret.length >= countPerPage.value) {
            break;
        }

        if (isTransactionDisplayed(importTransaction)) {
            if (count >= previousCount) {
                ret.push(importTransaction);
            }

            count++;
        }
    }

    return ret;
});

const selectedImportTransactionCount = computed<number>(() => {
    let count = 0;

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return count;
    }

    for (const importTransaction of props.importTransactions) {
        if (importTransaction.selected) {
            count++;
        }
    }

    return count;
});

const selectedExpenseTransactionCount = computed<number>(() => {
    let count = 0;

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return count;
    }

    for (const importTransaction of props.importTransactions) {
        if (importTransaction.selected && importTransaction.type === TransactionType.Expense) {
            count++;
        }
    }

    return count;
});

const selectedIncomeTransactionCount = computed<number>(() => {
    let count = 0;

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return count;
    }

    for (const importTransaction of props.importTransactions) {
        if (importTransaction.selected && importTransaction.type === TransactionType.Income) {
            count++;
        }
    }

    return count;
});

const selectedTransferTransactionCount = computed<number>(() => {
    let count = 0;

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return count;
    }

    for (const importTransaction of props.importTransactions) {
        if (importTransaction.selected && importTransaction.type === TransactionType.Transfer) {
            count++;
        }
    }

    return count;
});

const selectedRecurringMatchCount = computed<number>(() => {
    let count = 0;

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return count;
    }

    for (const importTransaction of props.importTransactions) {
        if (importTransaction.selected && importTransaction.hasRecurringMatch()) {
            count++;
        }
    }

    return count;
});

const selectedInvalidTransactionCount = computed<number>(() => {
    let count = 0;

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return count;
    }

    for (const importTransaction of props.importTransactions) {
        if (!importTransaction.valid && importTransaction.selected) {
            count++;
        }
    }

    return count;
});

const annotationTransactionCount = computed<number>(() => {
    let count = 0;

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return count;
    }

    for (const importTransaction of props.importTransactions) {
        if (needsAnnotation(importTransaction)) {
            count++;
        }
    }

    return count;
});

const selectedAnnotationTransactionCount = computed<number>(() => {
    let count = 0;

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return count;
    }

    for (const importTransaction of props.importTransactions) {
        if (importTransaction.selected && needsAnnotation(importTransaction)) {
            count++;
        }
    }

    return count;
});

const selectedAnnotationTransactions = computed<ImportTransaction[]>(() => {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return [];
    }

    return props.importTransactions.filter(transaction => transaction.selected && needsAnnotation(transaction));
});

const annotationReasonSummaries = computed<AnnotationReasonSummary[]>(() => {
    const summary: Record<string, AnnotationReasonSummary> = {};

    for (const transaction of selectedAnnotationTransactions.value) {
        for (const reason of getAnnotationIssues(transaction)) {
            if (!summary[reason]) {
                summary[reason] = {
                    key: reason,
                    label: reason,
                    count: 0
                };
            }

            summary[reason].count++;
        }
    }

    return Object.values(summary).sort((left, right) => right.count - left.count);
});

const anyButNotAllTransactionSelected = computed<boolean>(() => !!props.importTransactions && selectedImportTransactionCount.value > 0 && selectedImportTransactionCount.value !== props.importTransactions.length);
const allTransactionSelected = computed<boolean>(() => !!props.importTransactions && selectedImportTransactionCount.value === props.importTransactions.length);

const allUsedCategoryNames = computed<string[]>(() => {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return [];
    }

    const categoryNames: Record<string, boolean> = {};

    for (const transaction of props.importTransactions) {
        if (transaction.actualCategoryName && transaction.actualCategoryName !== '') {
            categoryNames[transaction.actualCategoryName] = true;
        }
    }

    return objectFieldToArrayItem(categoryNames);
});

const allUsedAccountNames = computed<string[]>(() => {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return [];
    }

    const accountNames: Record<string, boolean> = {};

    for (const transaction of props.importTransactions) {
        if (transaction.actualSourceAccountName && transaction.actualSourceAccountName !== '') {
            accountNames[transaction.actualSourceAccountName] = true;
        }

        if (transaction.actualDestinationAccountName && transaction.actualDestinationAccountName !== '') {
            accountNames[transaction.actualDestinationAccountName] = true;
        }
    }

    return objectFieldToArrayItem(accountNames);
});

const allUsedTagNames = computed<string[]>(() => {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return [];
    }

    const tagNames: Record<string, boolean> = {};

    for (const transaction of props.importTransactions) {
        if (!transaction.tagIds || !transaction.originalTagNames) {
            continue;
        }

        for (const [tagId, tagIndex] of itemAndIndex(transaction.tagIds)) {
            const originalTagName = transaction.originalTagNames[tagIndex] as string | undefined;

            if (tagId && tagId !== '0' && allTagsMap.value[tagId] && allTagsMap.value[tagId].name) {
                tagNames[allTagsMap.value[tagId].name] = true;
            } else if (originalTagName) {
                tagNames[originalTagName] = true;
            }
        }
    }

    return objectFieldToArrayItem(tagNames);
});

const allInvalidExpenseCategoryNames = computed<NameValue[]>(() => getCurrentInvalidCategoryNames(TransactionType.Expense));
const allInvalidIncomeCategoryNames = computed<NameValue[]>(() => getCurrentInvalidCategoryNames(TransactionType.Income));
const allInvalidTransferCategoryNames = computed<NameValue[]>(() => getCurrentInvalidCategoryNames(TransactionType.Transfer));
const allInvalidAccountNames = computed<NameValue[]>(() => getCurrentInvalidAccountNames());
const allInvalidTransactionTagNames = computed<NameValue[]>(() => getCurrentInvalidTagNames());
const allOriginalTransactionTagNames = computed<NameValue[]>(() => getAllOriginalTagNames());

const displayFilterCustomDateRange = computed<string>(() => {
    if (filters.value.minDatetime === null || filters.value.maxDatetime === null) {
        return '';
    }

    const minDisplayTime = formatUnixTimeToLongDateTime(filters.value.minDatetime);
    const maxDisplayTime = formatUnixTimeToLongDateTime(filters.value.maxDatetime);

    return `${minDisplayTime} - ${maxDisplayTime}`
});

function getDisplayCount(count: number): string {
    return numeralSystem.value.formatNumber(count);
}

function getTablePageOptions(linesCount?: number): NameNumeralValue[] {
    const pageOptions: NameNumeralValue[] = [];

    if (!linesCount || linesCount < 1) {
        pageOptions.push({ value: -1, name: tt('All') });
        return pageOptions;
    }

    for (const count of [ 5, 10, 15, 20, 25, 30, 50 ]) {
        if (linesCount < count) {
            break;
        }

        pageOptions.push({ value: count, name: getDisplayCount(count) });
    }

    pageOptions.push({ value: -1, name: tt('All') });

    return pageOptions;
}

function isTransactionDisplayed(transaction: ImportTransaction): boolean {
    if (isNumber(filters.value.minDatetime) && isNumber(filters.value.maxDatetime) && (transaction.time < filters.value.minDatetime || transaction.time > filters.value.maxDatetime)) {
        return false;
    }

    if (isNumber(filters.value.transactionType) && transaction.type !== filters.value.transactionType) {
        return false;
    }

    if (isString(filters.value.category)) {
        if (filters.value.category === '' && transaction.actualCategoryName !== '') {
            return false;
        } else if (filters.value.category !== '' && transaction.actualCategoryName !== filters.value.category) {
            return false;
        }
    } else if (filters.value.category === undefined) {
        if (transaction.type !== TransactionType.ModifyBalance && transaction.categoryId && transaction.categoryId !== '0') {
            return false;
        }
    }

    if (isString(filters.value.account)) {
        if (filters.value.account === '' && (transaction.actualSourceAccountName !== '' || transaction.actualDestinationAccountName !== '')) {
            return false;
        } else if (filters.value.account !== '' && transaction.actualSourceAccountName !== filters.value.account && transaction.actualDestinationAccountName !== filters.value.account) {
            return false;
        }
    } else if (filters.value.account === undefined) {
        if (transaction.type !== TransactionType.Transfer && transaction.sourceAccountId && transaction.sourceAccountId !== '0') {
            return false;
        } else if (transaction.type === TransactionType.Transfer && transaction.sourceAccountId && transaction.sourceAccountId !== '0' && transaction.destinationAccountId && transaction.destinationAccountId !== '0') {
            return false;
        }
    }

    if (isString(filters.value.tag)) {
        if (filters.value.tag === '' && transaction.tagIds && transaction.tagIds.length) {
            return false;
        } else if (filters.value.tag !== '') {
            let hasTagName = false;

            if (transaction.tagIds && transaction.tagIds.length) {
                for (const [tagId, tagIndex] of itemAndIndex(transaction.tagIds)) {
                    let tagName: string = transaction.originalTagNames ? (transaction.originalTagNames[tagIndex] ?? '') : '';

                    if (tagId && tagId !== '0' && allTagsMap.value[tagId] && allTagsMap.value[tagId].name) {
                        tagName = allTagsMap.value[tagId].name;
                    }

                    if (tagName === filters.value.tag) {
                        hasTagName = true;
                        break;
                    }
                }
            }

            if (!hasTagName) {
                return false;
            }
        }
    } else if (filters.value.tag === undefined) {
        if (transaction.tagIds && transaction.tagIds.length) {
            let hasInvalidTag = false;

            for (const tagId of transaction.tagIds) {
                if (!tagId || tagId === '0') {
                    hasInvalidTag = true;
                    break;
                }
            }

            if (!hasInvalidTag) {
                return false;
            }
        } else {
            return false;
        }
    }

    if (filters.value.annotation !== null) {
        if (filters.value.annotation === 'pending' && (!needsAnnotation(transaction) || transaction.isManuallyAnnotated)) {
            return false;
        }
        if (filters.value.annotation === 'annotated' && !transaction.isManuallyAnnotated) {
            return false;
        }
        if (filters.value.annotation === 'no-issues' && needsAnnotation(transaction)) {
            return false;
        }
    }

    if (isString(filters.value.description)) {
        if (filters.value.description === '' && transaction.comment !== '') {
            return false;
        } else if (filters.value.description !== '' && transaction.comment.indexOf(filters.value.description) < 0) {
            return false;
        }
    }

    return true;
}

function isTagValid(tagIds: string[], tagIndex: number): boolean {
    if (!tagIds || !tagIds[tagIndex]) {
        return false;
    }

    if (tagIds[tagIndex] === '0') {
        return false;
    }

    const tagId = tagIds[tagIndex];
    return !!allTagsMap.value[tagId];
}

function getDisplayDateTime(transaction: ImportTransaction): string {
    return formatUnixTimeToLongDateTime(transaction.time, transaction.utcOffset, currentTimezoneOffsetMinutes.value);
}

function getDisplayTimezone(transaction: ImportTransaction): string {
    return `UTC${getUtcOffsetByUtcOffsetMinutes(transaction.utcOffset)}`;
}

function getDisplayCurrency(value: number, currencyCode: string): string {
    return formatAmountToLocalizedNumeralsWithCurrency(value, currencyCode);
}

function getTransactionDisplayAmount(transaction: ImportTransaction): string {
    let currency = transaction.originalSourceAccountCurrency || defaultCurrency.value;

    if (transaction.sourceAccountId && transaction.sourceAccountId !== '0' && allAccountsMap.value[transaction.sourceAccountId]) {
        currency = allAccountsMap.value[transaction.sourceAccountId]!.currency;
    }

    return getDisplayCurrency(transaction.sourceAmount, currency);
}

function getTransactionDisplayDestinationAmount(transaction: ImportTransaction): string {
    // v6.55: 转账和投资类型都需要显示目标金额
    if (transaction.type !== TransactionType.Transfer && transaction.type !== TransactionType.Investment) {
        return '-';
    }

    let currency = transaction.originalDestinationAccountCurrency || defaultCurrency.value;

    if (transaction.destinationAccountId && transaction.destinationAccountId !== '0' && allAccountsMap.value[transaction.destinationAccountId]) {
        currency = allAccountsMap.value[transaction.destinationAccountId]!.currency;
    }

    return getDisplayCurrency(transaction.destinationAmount, currency);
}

function getSourceAccountTitle(transaction: ImportTransaction): string {
    if (transaction.type === TransactionType.Expense || transaction.type === TransactionType.Income) {
        return tt('Account');
    } else if (transaction.type === TransactionType.Transfer) {
        return tt('Source Account');
    } else {
        return tt('Account');
    }
}

function getSourceAccountDisplayName(transaction: ImportTransaction): string {
    if (transaction.sourceAccountId) {
        return Account.findAccountNameById(allAccounts.value, transaction.sourceAccountId) || '';
    } else {
        return tt('None');
    }
}

function getDestinationAccountDisplayName(transaction: ImportTransaction): string {
    if (transaction.destinationAccountId) {
        return Account.findAccountNameById(allAccounts.value, transaction.destinationAccountId) || '';
    } else {
        return tt('None');
    }
}

function getCurrentInvalidCategoryNames(transactionType: TransactionType): NameValue[] {
    const invalidCategoryNames: Record<string, boolean> = {};
    const invalidCategories: NameValue[] = [];

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return invalidCategories;
    }

    for (const importTransaction of props.importTransactions) {
        const categoryId = importTransaction.categoryId;

        if (importTransaction.type === transactionType && (!categoryId || categoryId === '0' || !allCategoriesMap.value[categoryId])) {
            invalidCategoryNames[importTransaction.originalCategoryName] = true;
        }
    }

    for (const name of keys(invalidCategoryNames)) {
        invalidCategories.push({
            name: name || tt('(Empty)'),
            value: name
        });
    }

    return invalidCategories;
}

function getCurrentInvalidAccountNames(): NameValue[] {
    const invalidAccountNames: Record<string, boolean> = {};
    const invalidAccounts: NameValue[] = [];

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return invalidAccounts;
    }

    for (const importTransaction of props.importTransactions) {
        const sourceAccountId = importTransaction.sourceAccountId;
        const destinationAccountId = importTransaction.destinationAccountId;

        if (!sourceAccountId || sourceAccountId === '0' || !allAccountsMap.value[sourceAccountId]) {
            invalidAccountNames[importTransaction.originalSourceAccountName] = true;
        }

        if (importTransaction.type === TransactionType.Transfer && isString(importTransaction.originalDestinationAccountName) && (!destinationAccountId || destinationAccountId === '0' || !allAccountsMap.value[destinationAccountId])) {
            invalidAccountNames[importTransaction.originalDestinationAccountName] = true;
        }
    }

    for (const name of keys(invalidAccountNames)) {
        invalidAccounts.push({
            name: name || tt('(Empty)'),
            value: name
        });
    }

    return invalidAccounts;
}

function getCurrentInvalidTagNames(): NameValue[] {
    const invalidTagNames: Record<string, boolean> = {};
    const invalidTags: NameValue[] = [];

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return invalidTags;
    }

    for (const importTransaction of props.importTransactions) {
        if (!importTransaction.tagIds || !importTransaction.originalTagNames) {
            continue;
        }

        for (const [tagId, tagIndex] of itemAndIndex(importTransaction.tagIds)) {
            const originalTagName = importTransaction.originalTagNames[tagIndex] as string | undefined;

            if (!originalTagName) {
                continue;
            }

            if (!tagId || tagId === '0' || !allTagsMap.value[tagId]) {
                invalidTagNames[originalTagName] = true;
            }
        }
    }

    for (const name of keys(invalidTagNames)) {
        invalidTags.push({
            name: name || tt('(Empty)'),
            value: name
        });
    }

    return invalidTags;
}

function getAllOriginalTagNames(): NameValue[] {
    const allOriginalTagNames: Record<string, boolean> = {};
    const allOriginalTags: NameValue[] = [];

    if (!props.importTransactions || props.importTransactions.length < 1) {
        return allOriginalTags;
    }

    for (const importTransaction of props.importTransactions) {
        if (!importTransaction.originalTagNames) {
            continue;
        }

        for (const tagName of importTransaction.originalTagNames) {
            allOriginalTagNames[tagName] = true;
        }
    }

    for (const name of keys(allOriginalTagNames)) {
        allOriginalTags.push({
            name: name || tt('(Empty)'),
            value: name
        });
    }

    return allOriginalTags;
}

function importTransactionsFilter(value: string, query: string, item?: { value: unknown, raw: ImportTransaction }): boolean {
    if (!item || !item.raw) {
        return false;
    }

    return isTransactionDisplayed(item.raw);
}

function selectAllValid(): void {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return;
    }

    for (const importTransaction of props.importTransactions) {
        if (importTransaction.valid && isTransactionDisplayed(importTransaction)) {
            importTransaction.selected = true;
        }
    }
}

function selectAllInvalid(): void {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return;
    }

    for (const importTransaction of props.importTransactions) {
        if (!importTransaction.valid && isTransactionDisplayed(importTransaction)) {
            importTransaction.selected = true;
        }
    }
}

function selectAllNeedsAnnotation(): void {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return;
    }

    for (const importTransaction of props.importTransactions) {
        if (needsAnnotation(importTransaction) && isTransactionDisplayed(importTransaction)) {
            importTransaction.selected = true;
        }
    }
}

function selectAll(): void {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return;
    }

    for (const importTransaction of props.importTransactions) {
        if (isTransactionDisplayed(importTransaction)) {
            importTransaction.selected = true;
        }
    }
}

function selectNone(): void {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return;
    }

    for (const importTransaction of props.importTransactions) {
        if (isTransactionDisplayed(importTransaction)) {
            importTransaction.selected = false;
        }
    }
}

function selectInvert(): void {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return;
    }

    for (const importTransaction of props.importTransactions) {
        if (isTransactionDisplayed(importTransaction)) {
            importTransaction.selected = !importTransaction.selected;
        }
    }
}

function selectAllInThisPage(): void {
    for (const importTransaction of currentPageTransactions.value) {
        importTransaction.selected = true;
    }
}

function selectNoneInThisPage(): void {
    for (const importTransaction of currentPageTransactions.value) {
        importTransaction.selected = false;
    }
}

function selectInvertInThisPage(): void {
    for (const importTransaction of currentPageTransactions.value) {
        importTransaction.selected = !importTransaction.selected;
    }
}

function editTransaction(transaction: ImportTransaction): void {
    if (editingTransaction.value) {
        editingTransaction.value.tagIds = editingTags.value;
        updateTransactionData(editingTransaction.value);
        editingTransaction.value.isManuallyAnnotated = true;
    }

    if (editingTransaction.value === transaction) {
        editingTags.value = [];
        editingTransaction.value = null;
    } else {
        editingTransaction.value = transaction;
        editingTags.value = editingTransaction.value.tagIds;
    }
}

function openAnnotationDialog(): void {
    if (selectedAnnotationTransactionCount.value < 1) {
        snackbar.value?.showMessage(getNoSelectedAnnotationText());
        return;
    }

    showAnnotationDialog.value = true;
}

function openBatchCategoryDialogFromAnnotation(): void {
    showAnnotationDialog.value = false;
    showBatchCategoryDialog.value = true;
}

function openBatchAccountDialogFromAnnotation(): void {
    showAnnotationDialog.value = false;
    showBatchAccountDialog.value = true;
}

function editFirstAnnotationTransaction(): void {
    const firstTransaction = selectedAnnotationTransactions.value[0];

    if (!firstTransaction) {
        return;
    }

    showAnnotationDialog.value = false;
    editTransaction(firstTransaction);
}

function updateTransactionData(transaction: ImportTransaction): void {
    transaction.valid = transaction.isTransactionValid();

    if (transaction.categoryId && allCategoriesMap.value[transaction.categoryId]) {
        transaction.actualCategoryName = allCategoriesMap.value[transaction.categoryId]!.name;
    }

    if (transaction.sourceAccountId && allAccountsMap.value[transaction.sourceAccountId]) {
        transaction.actualSourceAccountName = allAccountsMap.value[transaction.sourceAccountId]!.name;
    }

    if (transaction.destinationAccountId && allAccountsMap.value[transaction.destinationAccountId]) {
        transaction.actualDestinationAccountName = allAccountsMap.value[transaction.destinationAccountId]!.name;
    }
}

function showBatchReplaceDialog(type: BatchReplaceDialogDataType, allSourceTagItems?: NameValue[]): void {
    if (isEditing.value) {
        return;
    }

    batchReplaceDialog.value?.open({
        mode: 'batchReplace',
        type: type,
        allSourceTagItems: allSourceTagItems
    }).then(result => {
        if (!result) {
            return;
        }

        if (type !== 'tag') {
            if (!result.targetItem) {
                return;
            }
        }

        let updatedCount = 0;

        if (props.importTransactions) {
            for (const importTransaction of props.importTransactions) {
                if (!importTransaction.selected) {
                    continue;
                }

                let updated = false;

                if (type === 'expenseCategory') {
                    if (importTransaction.type === TransactionType.Expense) {
                        importTransaction.categoryId = result.targetItem as string;
                        updated = true;
                    }
                } else if (type === 'incomeCategory') {
                    if (importTransaction.type === TransactionType.Income) {
                        importTransaction.categoryId = result.targetItem as string;
                        updated = true;
                    }
                } else if (type === 'transferCategory') {
                    if (importTransaction.type === TransactionType.Transfer) {
                        importTransaction.categoryId = result.targetItem as string;
                        updated = true;
                    }
                } else if (type === 'account') {
                    importTransaction.sourceAccountId = result.targetItem as string;
                    updated = true;
                } else if (type === 'destinationAccount') {
                    if (importTransaction.type === TransactionType.Transfer) {
                        importTransaction.destinationAccountId = result.targetItem as string;
                        updated = true;
                    }
                } else if (type === 'tag') {
                    const removeIndex: number[] = [];

                    for (let tagIndex = 0; tagIndex < importTransaction.originalTagNames.length; tagIndex++) {
                        const originalTagName = importTransaction.originalTagNames ? (importTransaction.originalTagNames[tagIndex] ?? '') : '';

                        if (originalTagName === result.sourceItem) {
                            if (result.targetItem) {
                                importTransaction.tagIds[tagIndex] = result.targetItem;
                                importTransaction.originalTagNames[tagIndex] = allTagsMap.value[result.targetItem]?.name || '';
                            } else {
                                removeIndex.push(tagIndex);
                            }
                            updated = true;
                        }
                    }

                    for (const tagIndex of reversed(removeIndex)) {
                        importTransaction.tagIds.splice(tagIndex, 1);
                        importTransaction.originalTagNames.splice(tagIndex, 1);
                    }
                }

                if (updated) {
                    updatedCount++;
                    importTransaction.isManuallyAnnotated = true;
                    updateTransactionData(importTransaction);
                }
            }
        }

        if (updatedCount > 0) {
            snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                count: getDisplayCount(updatedCount)
            });
        }
    });
}

function showBatchAddDialog(type: BatchReplaceDialogDataType): void {
    if (isEditing.value) {
        return;
    }

    batchReplaceDialog.value?.open({
        mode: 'batchAdd',
        type: type
    }).then(result => {
        if (!result || !result.targetItem) {
            return;
        }

        let updatedCount = 0;

        if (props.importTransactions) {
            for (const importTransaction of props.importTransactions) {
                if (!importTransaction.selected) {
                    continue;
                }

                let updated = false;

                if (type === 'tag') {
                    let containsTag = false;

                    for (const tagName of importTransaction.originalTagNames) {
                        if (tagName === result.targetItem) {
                            containsTag = true;
                            break;
                        }
                    }

                    if (!containsTag) {
                        if (!importTransaction.tagIds) {
                            importTransaction.tagIds = [];
                        }

                        if (!importTransaction.originalTagNames) {
                            importTransaction.originalTagNames = [];
                        }

                        importTransaction.tagIds.push(result.targetItem);
                        importTransaction.originalTagNames.push(allTagsMap.value[result.targetItem]?.name ?? '');
                        updated = true;
                    }
                }

                if (updated) {
                    updatedCount++;
                    importTransaction.isManuallyAnnotated = true;
                    updateTransactionData(importTransaction);
                }
            }
        }

        if (updatedCount > 0) {
            snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                count: getDisplayCount(updatedCount)
            });
        }
    });
}

function showReplaceInvalidItemDialog(type: BatchReplaceDialogDataType, invalidItems: NameValue[]): void {
    if (isEditing.value) {
        return;
    }

    batchReplaceDialog.value?.open({
        mode: 'replaceInvalidItems',
        type: type,
        invalidItems: invalidItems
    }).then(result => {
        if (!result || (!result.sourceItem && result.sourceItem !== '')) {
            return;
        }

        if (type !== 'tag') {
            if (!result.targetItem) {
                return;
            }
        }

        let updatedCount = 0;

        if (props.importTransactions) {
            for (const importTransaction of props.importTransactions) {
                if (importTransaction.valid) {
                    continue;
                }

                let updated = false;

                if (type === 'expenseCategory' || type === 'incomeCategory' || type === 'transferCategory') {
                    const categoryId = importTransaction.categoryId;
                    const originalCategoryName = importTransaction.originalCategoryName;

                    if (importTransaction.type !== TransactionType.ModifyBalance && originalCategoryName === result.sourceItem && (!categoryId || categoryId === '0' || !allCategoriesMap.value[categoryId])) {
                        if (type === 'expenseCategory' && importTransaction.type === TransactionType.Expense) {
                            importTransaction.categoryId = result.targetItem as string;
                            updated = true;
                        } else if (type === 'incomeCategory' && importTransaction.type === TransactionType.Income) {
                            importTransaction.categoryId = result.targetItem as string;
                            updated = true;
                        } else if (type === 'transferCategory' && importTransaction.type === TransactionType.Transfer) {
                            importTransaction.categoryId = result.targetItem as string;
                            updated = true;
                        }
                    }
                } else if (type === 'account') {
                    const sourceAccountId = importTransaction.sourceAccountId;
                    const originalSourceAccountName = importTransaction.originalSourceAccountName;
                    const destinationAccountId = importTransaction.destinationAccountId;
                    const originalDestinationAccountName = importTransaction.originalDestinationAccountName;

                    if (originalSourceAccountName === result.sourceItem && (!sourceAccountId || sourceAccountId === '0' || !allAccountsMap.value[sourceAccountId])) {
                        importTransaction.sourceAccountId = result.targetItem as string;
                        updated = true;
                    }

                    if (importTransaction.type === TransactionType.Transfer && originalDestinationAccountName === result.sourceItem && (!destinationAccountId || destinationAccountId === '0' || !allAccountsMap.value[destinationAccountId])) {
                        importTransaction.destinationAccountId = result.targetItem as string;
                        updated = true;
                    }
                } else if (type === 'tag' && importTransaction.tagIds) {
                    const removeIndex: number[] = [];

                    for (let tagIndex = 0; tagIndex < importTransaction.tagIds.length; tagIndex++) {
                        const tagId = importTransaction.tagIds[tagIndex] as string;
                        const originalTagName = importTransaction.originalTagNames ? (importTransaction.originalTagNames[tagIndex] ?? '') : '';

                        if (originalTagName === result.sourceItem && (!tagId || tagId === '0' || !allTagsMap.value[tagId])) {
                            if (result.targetItem) {
                                importTransaction.tagIds[tagIndex] = result.targetItem;
                                importTransaction.originalTagNames[tagIndex] = allTagsMap.value[result.targetItem]?.name || '';
                            } else {
                                removeIndex.push(tagIndex);
                            }
                            updated = true;
                        }
                    }

                    for (const tagIndex of reversed(removeIndex)) {
                        importTransaction.tagIds.splice(tagIndex, 1);
                        importTransaction.originalTagNames.splice(tagIndex, 1);
                    }
                }

                if (updated) {
                    updatedCount++;
                    importTransaction.isManuallyAnnotated = true;
                    updateTransactionData(importTransaction);
                }
            }
        }

        if (updatedCount > 0) {
            snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                count: getDisplayCount(updatedCount)
            });
        }
    });
}

function showReplaceAllTypesDialog(): void {
    if (isEditing.value) {
        return;
    }

    batchReplaceAllTypesDialog.value?.open({
        expenseCategoryNames: allInvalidExpenseCategoryNames.value,
        incomeCategoryNames: allInvalidIncomeCategoryNames.value,
        transferCategoryNames: allInvalidTransferCategoryNames.value,
        accountNames: allInvalidAccountNames.value,
        tagNames: allInvalidTransactionTagNames.value
    }).then(result => {
        if (!result || !result.rules) {
            return;
        }

        let updatedCount = 0;

        if (props.importTransactions) {
            for (const importTransaction of props.importTransactions) {
                let updated = false;

                for (const rule of result.rules) {
                    if (!rule || !rule.dataType || !rule.targetId) {
                        continue;
                    }

                    if (rule.dataType === 'expenseCategory' || rule.dataType === 'incomeCategory' || rule.dataType === 'transferCategory') {
                        if (importTransaction.type !== TransactionType.ModifyBalance && importTransaction.originalCategoryName === rule.sourceValue) {
                            if (rule.dataType === 'expenseCategory' && importTransaction.type === TransactionType.Expense) {
                                importTransaction.categoryId = rule.targetId;
                                updated = true;
                            } else if (rule.dataType === 'incomeCategory' && importTransaction.type === TransactionType.Income) {
                                importTransaction.categoryId = rule.targetId;
                                updated = true;
                            } else if (rule.dataType === 'transferCategory' && importTransaction.type === TransactionType.Transfer) {
                                importTransaction.categoryId = rule.targetId;
                                updated = true;
                            }
                        }
                    } else if (rule.dataType === 'account') {
                        if (importTransaction.originalSourceAccountName === rule.sourceValue) {
                            importTransaction.sourceAccountId = rule.targetId;
                            updated = true;
                        }

                        if (importTransaction.type === TransactionType.Transfer && importTransaction.originalDestinationAccountName === rule.sourceValue) {
                            importTransaction.destinationAccountId = rule.targetId;
                            updated = true;
                        }
                    } else if (rule.dataType === 'tag' && importTransaction.tagIds) {
                        for (let tagIndex = 0; tagIndex < importTransaction.tagIds.length; tagIndex++) {
                            const originalTagName = importTransaction.originalTagNames ? (importTransaction.originalTagNames[tagIndex] ?? '') : '';

                            if (originalTagName === rule.sourceValue) {
                                importTransaction.tagIds[tagIndex] = rule.targetId;
                                updated = true;
                            }
                        }
                    }
                }

                if (updated) {
                    updatedCount++;
                    importTransaction.isManuallyAnnotated = true;
                    updateTransactionData(importTransaction);
                }
            }
        }

        if (updatedCount > 0) {
            snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                count: getDisplayCount(updatedCount)
            });
        }
    });
}

function showBatchCreateInvalidItemDialog(type: BatchCreateDialogDataType, invalidItems: NameValue[]): void {
    if (isEditing.value) {
        return;
    }

    batchCreateDialog.value?.open({
        type: type,
        invalidItems: invalidItems
    }).then(result => {
        if (!result || !result.sourceTargetMap) {
            return;
        }

        let updatedCount = 0;

        if (props.importTransactions) {
            const sourceTargetMap: Record<string, string> = result.sourceTargetMap;

            for (const importTransaction of props.importTransactions) {
                if (importTransaction.valid) {
                    continue;
                }

                let updated = false;

                if (type === 'expenseCategory' || type === 'incomeCategory' || type === 'transferCategory') {
                    const categoryId = importTransaction.categoryId;
                    const originalCategoryName = importTransaction.originalCategoryName;
                    const targetItem = sourceTargetMap[originalCategoryName];

                    if (importTransaction.type !== TransactionType.ModifyBalance && targetItem && (!categoryId || categoryId === '0' || !allCategoriesMap.value[categoryId])) {
                        if (type === 'expenseCategory' && importTransaction.type === TransactionType.Expense) {
                            importTransaction.categoryId = targetItem;
                            updated = true;
                        } else if (type === 'incomeCategory' && importTransaction.type === TransactionType.Income) {
                            importTransaction.categoryId = targetItem;
                            updated = true;
                        } else if (type === 'transferCategory' && importTransaction.type === TransactionType.Transfer) {
                            importTransaction.categoryId = targetItem;
                            updated = true;
                        }
                    }
                } else if (type === 'tag' && importTransaction.tagIds) {
                    for (let tagIndex = 0; tagIndex < importTransaction.tagIds.length; tagIndex++) {
                        const tagId = importTransaction.tagIds[tagIndex] as string;
                        const originalTagName = importTransaction.originalTagNames ? (importTransaction.originalTagNames[tagIndex] ?? '') : '';
                        const targetItem = sourceTargetMap[originalTagName];

                        if (targetItem && (!tagId || tagId === '0' || !allTagsMap.value[tagId])) {
                            importTransaction.tagIds[tagIndex] = targetItem;
                            updated = true;
                        }
                    }
                }

                if (updated) {
                    updatedCount++;
                    importTransaction.isManuallyAnnotated = true;
                    updateTransactionData(importTransaction);
                }
            }
        }

        if (updatedCount > 0) {
            snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
                count: getDisplayCount(updatedCount)
            });
        }
    });
}

function convertTransactionType(fromType: TransactionType, toType: TransactionType): void {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return;
    }

    const categoryType = transactionTypeToCategoryType(toType);

    if (!categoryType) {
        return;
    }

    const categoryMapByName: Record<string, TransactionCategory> = getSecondaryTransactionMapByName(allCategories.value[categoryType]);

    for (const importTransaction of props.importTransactions) {
        if (!importTransaction.selected || importTransaction.type !== fromType) {
            continue;
        }

        importTransaction.type = toType;
        importTransaction.categoryId = categoryMapByName[importTransaction.originalCategoryName]?.id || '0';

        if (importTransaction.type === TransactionType.Transfer) {
            importTransaction.destinationAccountId = allAccountsMapByName.value[importTransaction.originalDestinationAccountName || '']?.id || '0';
            importTransaction.destinationAmount = importTransaction.sourceAmount;
        } else {
            if (fromType === TransactionType.Transfer && toType === TransactionType.Income) {
                importTransaction.sourceAccountId = importTransaction.destinationAccountId;
                importTransaction.sourceAmount = importTransaction.destinationAmount;
            }

            importTransaction.destinationAccountId = '0';
            importTransaction.destinationAmount = 0;
        }

        importTransaction.isManuallyAnnotated = true;
        updateTransactionData(importTransaction);
    }
}

function clearSelectedRecurringMatches(): void {
    if (!props.importTransactions || props.importTransactions.length < 1) {
        return;
    }

    let updatedCount = 0;
    for (const importTransaction of props.importTransactions) {
        if (!importTransaction.selected || !importTransaction.hasRecurringMatch()) {
            continue;
        }

        importTransaction.clearRecurringMatch(false);
        updateTransactionData(importTransaction);
        updatedCount++;
    }

    if (updatedCount > 0) {
        snackbar.value?.showMessage('format.misc.youHaveUpdatedTransactions', {
            count: getDisplayCount(updatedCount)
        });
    }
}

function changeCustomDateFilter(minTime: number, maxTime: number): void {
    filters.value.minDatetime = minTime;
    filters.value.maxDatetime = maxTime;
    showCustomDateRangeDialog.value = false;
}

function onShowDateRangeError(message: string): void {
    snackbar.value?.showError(message);
}

function reset(): void {
    editingTransaction.value = null;
    editingTags.value = [];
    filters.value.minDatetime = null;
    filters.value.maxDatetime = null;
    filters.value.transactionType = null;
    filters.value.category = null;
    filters.value.account = null;
    filters.value.tag = null;
    filters.value.annotation = null;
    filters.value.description = null;
    currentPage.value = 1;
    countPerPage.value = 10;
}

function setCountPerPage(count: number): void {
    countPerPage.value = count;
}

defineExpose({
    filterMenus,
    toolMenus,
    isEditing,
    canImport,
    reset,
    setCountPerPage
});
</script>

<style>
.import-transaction-table .v-autocomplete.v-input.v-input--density-compact:not(.v-textarea) .v-field__input,
.import-transaction-table .v-select.v-input.v-input--density-compact:not(.v-textarea) .v-field__input {
    min-height: inherit;
    padding-top: 4px;
}

.import-transaction-table .v-chip.transaction-tag {
    margin-inline-end: 4px;
    margin-top: 2px;
    margin-bottom: 2px;
}

.import-transaction-table .v-chip.transaction-tag > .v-chip__content {
    display: block;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
}

.import-transaction-table .v-text-field.v-input.v-input--density-compact .v-field__input {
    padding-top: 0;
}
</style>
