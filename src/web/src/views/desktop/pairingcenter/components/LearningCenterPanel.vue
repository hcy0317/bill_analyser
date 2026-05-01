<template>
    <v-card variant="flat" class="learning-center-panel">

        <v-progress-linear v-if="loading" indeterminate color="primary" />

        <v-card-text>
            <v-alert v-if="error" type="error" closable class="mb-4"
                     @click:close="clearError">
                {{ error }}
            </v-alert>

                            <!-- ── Suggestions Tab ── -->
                            <template v-if="activeTab === 'suggestions'">
                                <div class="learning-section-header"
                                     :class="{ 'learning-section-header--empty': hideSectionTitle && hasHeaderActionsTarget }">
                                    <h3 v-if="!hideSectionTitle" class="learning-section-title">{{ tt('Auto Suggestions') }}</h3>
                                    <Teleport :disabled="!hasHeaderActionsTarget" :to="headerActionsTarget">
                                     <div class="learning-section-actions"
                                          :class="{
                                              'learning-section-actions--external': hasHeaderActionsTarget,
                                              'learning-section-actions--split': true,
                                          }">
                                        <div class="learning-section-actions__primary">
                                            <v-btn class="learning-panel-action"
                                                   variant="outlined"
                                                   color="default"
                                                   :disabled="loading"
                                                   @click="handleGenerate">
                                                {{ tt('Generate Suggestions') }}
                                            </v-btn>
                                            <v-btn class="learning-panel-action"
                                                   variant="outlined"
                                                   color="default"
                                                   :disabled="loading || selectedIds.length === 0"
                                                   @click="handleBatchAccept">
                                                {{ tt('Batch Accept') }} ({{ selectedIds.length }})
                                            </v-btn>
                                            <v-chip size="small" variant="tonal" color="info">
                                                {{ filteredSuggestions.length }} / {{ suggestionsTotal }}
                                            </v-chip>
                                            <v-btn class="learning-panel-refresh"
                                                   variant="text"
                                                   color="default"
                                                   :icon="true"
                                                   :loading="loading"
                                                   @click="refreshCurrentTab">
                                                <v-icon :icon="mdiRefresh" />
                                                <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                                            </v-btn>
                                        </div>
                                        <div class="learning-section-actions__spacer" />
                                        <v-select v-model="statusFilter"
                                                  class="learning-status-select"
                                                  :items="suggestionStatusOptions"
                                                  item-title="title"
                                                  item-value="value"
                                                  density="compact"
                                                  variant="outlined"
                                                  hide-details
                                                  :aria-label="tt('Status')" />
                                    </div>
                                    </Teleport>
                                </div>

                                <v-table v-if="!loading && filteredSuggestions.length > 0" hover density="comfortable">
                                    <thead>
                                        <tr>
                                            <th style="width:38px">
                                                <v-checkbox-btn v-model="selectAll" :indeterminate="indeterminate"
                                                                density="compact" hide-details />
                                            </th>
                                            <th>{{ tt('Match Pattern') }}</th>
                                            <th>{{ tt('Features') }}</th>
                                            <th>{{ tt('Suggested Action') }}</th>
                                            <th class="text-center">{{ tt('Samples') }}</th>
                                            <th>{{ tt('Status') }}</th>
                                            <th class="text-center">{{ tt('Actions') }}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        <tr v-for="item in filteredSuggestions" :key="item.id">
                                            <td>
                                                <v-checkbox-btn v-model="selectedIds" :value="item.id"
                                                                density="compact" hide-details
                                                                :disabled="item.status !== 'pending'" />
                                            </td>
                                            <td>
                                                <div class="text-body-2 font-weight-medium">{{ translateMatchType(item.matchType) }}</div>
                                                <div class="text-caption text-grey">{{ item.matchValue }}</div>
                                            </td>
                                            <td>
                                                <div class="d-flex flex-wrap ga-1">
                                                    <v-chip v-for="(feat, idx) in getFeatureSummary(item).split(' · ').filter(Boolean)"
                                                            :key="idx" size="x-small" variant="tonal" color="secondary">
                                                        {{ feat }}
                                                    </v-chip>
                                                </div>
                                            </td>
                                            <td>
                                                <div class="text-body-2">{{ item.summary || item.suggestedType }}</div>
                                            </td>
                                            <td class="text-center">
                                                <v-chip size="x-small" color="info" variant="tonal">{{ item.sampleCount }}</v-chip>
                                            </td>
                                            <td>
                                                <v-chip size="small" :color="statusColor(item.status)">
                                                    {{ tt(statusLabel(item.status)) }}
                                                </v-chip>
                                            </td>
                                            <td class="text-center">
                                                <template v-if="item.status === 'pending'">
                                                    <v-btn size="small" variant="text" color="success"
                                                           :icon="true" @click="handleAccept(item.id)">
                                                        <v-icon :icon="mdiCheck" />
                                                        <v-tooltip activator="parent">{{ tt('Accept') }}</v-tooltip>
                                                    </v-btn>
                                                    <v-btn size="small" variant="text" color="error"
                                                           :icon="true" @click="handleReject(item.id)">
                                                        <v-icon :icon="mdiClose" />
                                                        <v-tooltip activator="parent">{{ tt('Reject') }}</v-tooltip>
                                                    </v-btn>
                                                </template>
                                                <span v-else class="text-grey text-caption">—</span>
                                            </td>
                                        </tr>
                                    </tbody>
                                </v-table>

                                <v-empty-state v-if="!loading && filteredSuggestions.length === 0"
                                               :icon="mdiLightbulbOutline"
                                               :headline="tt('No Suggestions')"
                                               :text="tt('Click Generate Suggestions to mine patterns from your transaction history.')" />
                            </template>

                            <!-- ── Rules Tab ── -->
                            <template v-if="activeTab === 'rules'">
                                <div class="learning-section-header"
                                     :class="{ 'learning-section-header--empty': hideSectionTitle && hasHeaderActionsTarget }">
                                    <h3 v-if="!hideSectionTitle" class="learning-section-title">{{ tt('Learning Rules') }}</h3>
                                    <Teleport :disabled="!hasHeaderActionsTarget" :to="headerActionsTarget">
                                    <div class="learning-section-actions"
                                         :class="{ 'learning-section-actions--external': hasHeaderActionsTarget }">
                                        <v-chip size="small" variant="tonal" color="info">
                                            {{ filteredRules.length }} / {{ rulesTotal }}
                                        </v-chip>
                                        <v-btn variant="text"
                                               class="learning-panel-refresh"
                                               color="default"
                                               :icon="true"
                                               :loading="loading"
                                               @click="refreshCurrentTab">
                                            <v-icon :icon="mdiRefresh" />
                                            <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                                        </v-btn>
                                    </div>
                                    </Teleport>
                                </div>

                                <v-table v-if="!loading && filteredRules.length > 0"
                                         class="learning-rules-table"
                                         hover
                                         density="comfortable">
                                    <thead>
                                        <tr>
                                            <th>
                                                <v-menu v-model="ruleMatchTypeFilterMenu" location="bottom">
                                                    <template #activator="{ props: menuProps }">
                                                        <v-btn v-bind="menuProps"
                                                               variant="text"
                                                               density="compact"
                                                               size="small"
                                                               class="learning-header-button"
                                                               :color="ruleMatchTypeFilter ? 'primary' : undefined">
                                                            {{ tt('Match Pattern') }}
                                                            <v-icon end size="16"
                                                                    :icon="ruleMatchTypeFilter ? mdiFilterVariant : mdiChevronDown" />
                                                        </v-btn>
                                                    </template>
                                                    <v-list class="learning-header-menu" density="compact" min-width="180">
                                                        <v-list-item v-for="option in ruleMatchTypeOptions"
                                                                     :key="option.value"
                                                                     :value="option.value"
                                                                     :class="{ 'list-item-selected': ruleMatchTypeFilter === option.value }"
                                                                     :append-icon="ruleMatchTypeFilter === option.value ? mdiCheck : undefined"
                                                                     @click="setRuleMatchTypeFilter(option.value)">
                                                            <v-list-item-title>{{ option.title }}</v-list-item-title>
                                                        </v-list-item>
                                                    </v-list>
                                                </v-menu>
                                            </th>
                                            <th>
                                                <v-menu v-model="ruleFeatureFilterMenu"
                                                        :close-on-content-click="false"
                                                        location="bottom">
                                                    <template #activator="{ props: menuProps }">
                                                        <v-btn v-bind="menuProps"
                                                               variant="text"
                                                               density="compact"
                                                               size="small"
                                                               class="learning-header-button"
                                                               :color="ruleFeatureFilterActive ? 'primary' : undefined">
                                                            {{ tt('Features') }}
                                                            <v-icon end size="16"
                                                                    :icon="ruleFeatureFilterActive ? mdiFilterVariant : mdiChevronDown" />
                                                        </v-btn>
                                                    </template>
                                                    <v-card class="learning-header-menu" min-width="280">
                                                        <v-card-text class="pb-2">
                                                            <v-text-field v-model="ruleFeatureFilter"
                                                                          density="compact"
                                                                          variant="outlined"
                                                                          hide-details
                                                                          clearable
                                                                          :label="tt('Features')"
                                                                          :placeholder="tt('Filter')" />
                                                        </v-card-text>
                                                        <v-card-actions class="pt-0">
                                                            <v-spacer />
                                                            <v-btn variant="text" size="small" @click="clearRuleFeatureFilter">
                                                                {{ tt('Clear') }}
                                                            </v-btn>
                                                        </v-card-actions>
                                                    </v-card>
                                                </v-menu>
                                            </th>
                                            <th>
                                                <v-menu v-model="ruleLearnedActionFilterMenu"
                                                        :close-on-content-click="false"
                                                        location="bottom">
                                                    <template #activator="{ props: menuProps }">
                                                        <v-btn v-bind="menuProps"
                                                               variant="text"
                                                               density="compact"
                                                               size="small"
                                                               class="learning-header-button"
                                                               :color="ruleLearnedActionFilterActive ? 'primary' : undefined">
                                                            {{ tt('Learned Action') }}
                                                            <v-icon end size="16"
                                                                    :icon="ruleLearnedActionFilterActive ? mdiFilterVariant : mdiChevronDown" />
                                                        </v-btn>
                                                    </template>
                                                    <v-card class="learning-header-menu" min-width="280">
                                                        <v-card-text class="pb-2">
                                                            <v-text-field v-model="ruleLearnedActionFilter"
                                                                          density="compact"
                                                                          variant="outlined"
                                                                          hide-details
                                                                          clearable
                                                                          :label="tt('Learned Action')"
                                                                          :placeholder="tt('Filter')" />
                                                        </v-card-text>
                                                        <v-card-actions class="pt-0">
                                                            <v-spacer />
                                                            <v-btn variant="text" size="small" @click="clearRuleLearnedActionFilter">
                                                                {{ tt('Clear') }}
                                                            </v-btn>
                                                        </v-card-actions>
                                                    </v-card>
                                                </v-menu>
                                            </th>
                                            <th>
                                                <v-menu v-model="ruleEnabledFilterMenu" location="bottom">
                                                    <template #activator="{ props: menuProps }">
                                                        <v-btn v-bind="menuProps"
                                                               variant="text"
                                                               density="compact"
                                                               size="small"
                                                               class="learning-header-button"
                                                               :color="ruleEnabledFilter !== 'all' ? 'primary' : undefined">
                                                            {{ tt('Enabled') }}
                                                            <v-icon end size="16"
                                                                    :icon="ruleEnabledFilter !== 'all' ? mdiFilterVariant : mdiChevronDown" />
                                                        </v-btn>
                                                    </template>
                                                    <v-list class="learning-header-menu" density="compact" min-width="160">
                                                        <v-list-item v-for="option in ruleEnabledFilterOptions"
                                                                     :key="option.value"
                                                                     :value="option.value"
                                                                     :class="{ 'list-item-selected': ruleEnabledFilter === option.value }"
                                                                     :append-icon="ruleEnabledFilter === option.value ? mdiCheck : undefined"
                                                                     @click="setRuleEnabledFilter(option.value)">
                                                            <v-list-item-title>{{ option.title }}</v-list-item-title>
                                                        </v-list-item>
                                                    </v-list>
                                                </v-menu>
                                            </th>
                                            <th>
                                                <v-menu v-model="ruleAppliedFilterMenu" location="bottom">
                                                    <template #activator="{ props: menuProps }">
                                                        <v-btn v-bind="menuProps"
                                                               variant="text"
                                                               density="compact"
                                                               size="small"
                                                               class="learning-header-button"
                                                               :color="ruleAppliedFilter !== 'all' ? 'primary' : undefined">
                                                            {{ tt('Applied') }}
                                                            <v-icon end size="16"
                                                                    :icon="ruleAppliedFilter !== 'all' ? mdiFilterVariant : mdiChevronDown" />
                                                        </v-btn>
                                                    </template>
                                                    <v-list class="learning-header-menu" density="compact" min-width="160">
                                                        <v-list-item v-for="option in ruleAppliedFilterOptions"
                                                                     :key="option.value"
                                                                     :value="option.value"
                                                                     :class="{ 'list-item-selected': ruleAppliedFilter === option.value }"
                                                                     :append-icon="ruleAppliedFilter === option.value ? mdiCheck : undefined"
                                                                     @click="setRuleAppliedFilter(option.value)">
                                                            <v-list-item-title>{{ option.title }}</v-list-item-title>
                                                        </v-list-item>
                                                    </v-list>
                                                </v-menu>
                                            </th>
                                            <th class="text-center">{{ tt('Actions') }}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        <tr v-for="rule in filteredRules" :key="rule.id">
                                            <td class="text-center">
                                                <v-chip size="small" variant="tonal" color="info">
                                                    {{ translateMatchType(rule.matchType) }}
                                                </v-chip>
                                            </td>
                                            <td class="text-center">
                                                <div class="learning-rules-feature-cell d-flex flex-wrap ga-1 align-center justify-center">
                                                    <v-chip v-if="rule.matchValue"
                                                            size="x-small"
                                                            variant="outlined"
                                                            color="primary">
                                                        {{ rule.matchValue }}
                                                    </v-chip>
                                                    <v-chip v-for="(feat, idx) in getRuleFeatSummary(rule).split(' · ').filter(Boolean)"
                                                            :key="idx" size="x-small" variant="tonal" color="secondary">
                                                        {{ feat }}
                                                    </v-chip>
                                                </div>
                                            </td>
                                            <td class="text-center">
                                                <div class="text-body-2">{{ rule.learnedType }}</div>
                                            </td>
                                            <td class="text-center">
                                                <v-switch density="compact" color="success" hide-details
                                                          :model-value="rule.enabled"
                                                          @update:model-value="(v: boolean | null) => handleToggleRule(rule.id, !!v)" />
                                            </td>
                                            <td class="text-center">
                                                <v-chip size="x-small" color="info" variant="tonal">{{ rule.appliedCount }}</v-chip>
                                            </td>
                                            <td class="text-center">
                                                <div class="learning-rules-actions">
                                                    <v-btn size="small" variant="text" color="primary"
                                                           :icon="true" @click="openEditRuleDialog(rule)">
                                                        <v-icon :icon="mdiPencil" />
                                                        <v-tooltip activator="parent">{{ tt('Edit') }}</v-tooltip>
                                                    </v-btn>
                                                    <v-btn size="small" variant="text" color="error"
                                                           :icon="true" @click="handleDeleteRule(rule.id)">
                                                        <v-icon :icon="mdiDelete" />
                                                        <v-tooltip activator="parent">{{ tt('Delete') }}</v-tooltip>
                                                    </v-btn>
                                                </div>
                                            </td>
                                        </tr>
                                    </tbody>
                                </v-table>

                                <v-empty-state v-if="!loading && filteredRules.length === 0"
                                               :icon="mdiBookOpenPageVariant"
                                               :headline="tt('No Rules')"
                                               :text="tt('Accept suggestions to create learning rules that auto-classify future imports.')" />
                            </template>

            <template v-if="activeTab === 'llm-config'">
                                <!-- LLM 多配置管理 -->
                                <div class="learning-section-header"
                                     :class="{ 'learning-section-header--empty': hideSectionTitle && hasHeaderActionsTarget }">
                                    <h3 v-if="!hideSectionTitle" class="learning-section-title">{{ tt('LLM Config') }}</h3>
                                    <Teleport :disabled="!hasHeaderActionsTarget" :to="headerActionsTarget">
                                    <div class="learning-section-actions"
                                         :class="{ 'learning-section-actions--external': hasHeaderActionsTarget }">
                                        <v-btn class="learning-panel-action"
                                               variant="outlined"
                                               color="default"
                                               :disabled="loading"
                                               @click="openAddConfigDialog">
                                            {{ tt('Add Config') }}
                                        </v-btn>
                                        <v-btn class="learning-panel-refresh"
                                               variant="text"
                                               color="default"
                                               :icon="true"
                                               :loading="loading"
                                               @click="refreshCurrentTab">
                                            <v-icon :icon="mdiRefresh" />
                                            <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                                        </v-btn>
                                    </div>
                                    </Teleport>
                                </div>
                                <v-card variant="outlined" class="mb-4">
                                    <v-card-text class="pt-4">
                                        <v-table v-if="llmSavedConfigs.length > 0" density="compact" hover>
                                            <thead>
                                                <tr>
                                                    <th>{{ tt('Name') }}</th>
                                                    <th>{{ tt('Provider') }}</th>
                                                    <th>{{ tt('Model') }}</th>
                                                    <th class="text-center">{{ tt('Active') }}</th>
                                                    <th class="text-center">{{ tt('Actions') }}</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                <tr v-for="cfg in llmSavedConfigs" :key="cfg.id">
                                                    <td>{{ cfg.name }}</td>
                                                    <td>{{ llmProviderLabel(cfg.provider) }}</td>
                                                    <td>{{ cfg.model }}</td>
                                                    <td class="text-center">
                                                        <v-icon v-if="cfg.is_active" :icon="mdiCheck" color="success" size="small" />
                                                    </td>
                                                    <td class="text-center">
                                                        <v-btn v-if="!cfg.is_active" size="x-small" variant="text" color="primary"
                                                               @click="handleActivateConfig(cfg.id)">
                                                            {{ tt('Activate') }}
                                                        </v-btn>
                                                        <v-btn size="x-small" variant="text" color="error"
                                                               @click="handleDeleteConfig(cfg.id)">
                                                            <v-icon :icon="mdiDelete" size="small" />
                                                        </v-btn>
                                                    </td>
                                                </tr>
                                            </tbody>
                                        </v-table>
                                        <div v-else class="text-center text-caption text-medium-emphasis py-3">
                                            {{ tt('No saved configs. Click Add Config to create one.') }}
                                        </div>
                                    </v-card-text>
                                </v-card>

                                <v-card variant="outlined" class="mb-4">
                                    <v-card-title class="text-subtitle-1 d-flex align-center">
                                        <span>{{ tt('OCR Config') }}</span>
                                        <v-spacer />
                                        <v-chip size="small"
                                                :color="ocrConfig.configured ? 'success' : 'warning'"
                                                variant="tonal">
                                            {{ tt(ocrConfig.configured ? 'Enabled' : 'Disabled') }}
                                        </v-chip>
                                    </v-card-title>
                                    <v-divider />
                                    <v-card-text class="pt-4">
                                        <v-row>
                                            <v-col cols="12" md="5">
                                                <v-select v-model="ocrConfigForm.provider"
                                                          :items="ocrProviderOptions"
                                                          item-title="title"
                                                          item-value="value"
                                                          :label="tt('Provider')"
                                                          variant="outlined"
                                                          density="comfortable"
                                                          hide-details
                                                          :loading="ocrConfigLoading"
                                                          :disabled="ocrConfigLoading || ocrConfigSaving" />
                                            </v-col>
                                            <v-col cols="12" md="5">
                                                <v-text-field v-model="ocrConfigForm.lang"
                                                              :label="tt('OCR Language')"
                                                              variant="outlined"
                                                              density="comfortable"
                                                              hide-details
                                                              :disabled="ocrConfigForm.provider === 'disabled' || ocrConfigLoading || ocrConfigSaving" />
                                            </v-col>
                                            <v-col cols="12" md="2" class="d-flex align-center">
                                                <v-btn class="learning-panel-action w-100"
                                                       variant="outlined"
                                                       color="default"
                                                       :loading="ocrConfigSaving"
                                                       :disabled="ocrConfigLoading"
                                                       @click="saveOCRConfig">
                                                    {{ tt('Save') }}
                                                </v-btn>
                                            </v-col>
                                        </v-row>
                                    </v-card-text>
                                </v-card>
            </template>

            <template v-if="activeTab === 'llm'">
                                <!-- 建议规则列表 -->
                                <div class="learning-section-header"
                                     :class="{ 'learning-section-header--empty': hideSectionTitle && hasHeaderActionsTarget }">
                                    <h3 v-if="!hideSectionTitle" class="learning-section-title">{{ tt('Suggested Rules') }}</h3>
                                     <Teleport :disabled="!hasHeaderActionsTarget" :to="headerActionsTarget">
                                     <div class="learning-section-actions"
                                          :class="{
                                              'learning-section-actions--external': hasHeaderActionsTarget,
                                              'learning-section-actions--split': true,
                                          }">
                                        <div class="learning-section-actions__primary">
                                            <v-btn class="learning-panel-action"
                                                   variant="outlined"
                                                   color="default"
                                                   :disabled="loading"
                                                   @click="handleLLMGenerate">
                                                {{ tt('Generate Suggestions') }}
                                            </v-btn>
                                            <v-btn class="learning-panel-action"
                                                   variant="outlined"
                                                   color="default"
                                                   :disabled="loading || selectedLLMIds.length === 0"
                                                   @click="handleLLMBatchAccept">
                                                {{ tt('Batch Accept') }} ({{ selectedLLMIds.length }})
                                            </v-btn>
                                            <v-btn class="learning-panel-refresh"
                                                   variant="text"
                                                   color="default"
                                                   :icon="true"
                                                   :loading="loading"
                                                   @click="refreshCurrentTab">
                                                <v-icon :icon="mdiRefresh" />
                                                <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                                            </v-btn>
                                            <v-chip v-if="llmPendingCount > 0"
                                                    size="small"
                                                    color="warning"
                                                    variant="tonal">
                                                {{ llmPendingCount }} {{ tt('Pending') }}
                                            </v-chip>
                                        </div>
                                        <div class="learning-section-actions__spacer" />
                                        <v-select v-model="llmStatusFilter"
                                                  class="learning-status-select"
                                                  :items="llmStatusOptions"
                                                  item-title="title"
                                                  item-value="value"
                                                  density="compact"
                                                  variant="outlined"
                                                  hide-details
                                                  :aria-label="tt('Status')" />
                                    </div>
                                    </Teleport>
                                </div>

                                <v-table v-if="filteredLLMCandidates.length > 0" hover density="comfortable">
                                    <thead>
                                        <tr>
                                            <th style="width:38px">
                                                <v-checkbox-btn v-model="selectAllLLM"
                                                                :indeterminate="llmIndeterminate"
                                                                density="compact"
                                                                hide-details />
                                            </th>
                                            <th>{{ tt('Type') }}</th>
                                            <th>{{ tt('Rule Content') }}</th>
                                            <th>{{ tt('Target Category') }}</th>
                                            <th>{{ tt('Confidence') }}</th>
                                            <th>{{ tt('Status') }}</th>
                                            <th class="text-center">{{ tt('Actions') }}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        <tr v-for="candidate in filteredLLMCandidates" :key="candidate.id">
                                            <td>
                                                <v-checkbox-btn v-model="selectedLLMIds"
                                                                :value="candidate.id"
                                                                density="compact"
                                                                hide-details
                                                                :disabled="candidate.status !== 'pending'" />
                                            </td>
                                            <td>
                                                <v-chip size="x-small" variant="tonal" color="info">
                                                    {{ candidate.rule_type || candidate.type || 'keyword' }}
                                                </v-chip>
                                            </td>
                                            <td>
                                                <div class="text-body-2 font-weight-medium">{{ candidate.rule_content || candidate.expression || '-' }}</div>
                                                <div v-if="candidate.reason" class="text-caption text-grey">{{ candidate.reason }}</div>
                                            </td>
                                            <td>{{ candidate.category_name || candidate.target_category || '-' }}</td>
                                            <td>
                                                <v-chip size="x-small" :color="confidenceColor(candidate.confidence ?? 0)">
                                                    {{ ((candidate.confidence || 0) * 100).toFixed(0) }}%
                                                </v-chip>
                                            </td>
                                            <td>
                                                <v-chip size="small" :color="llmStatusColor(candidate.status)">
                                                    {{ llmStatusLabel(candidate.status) }}
                                                </v-chip>
                                            </td>
                                            <td class="text-center">
                                                <template v-if="candidate.status === 'pending'">
                                                    <v-btn size="small" variant="text" color="success"
                                                           :icon="true" @click="handleLLMAccept(candidate.id)">
                                                        <v-icon :icon="mdiCheck" />
                                                        <v-tooltip activator="parent">{{ tt('Accept') }}</v-tooltip>
                                                    </v-btn>
                                                    <v-btn size="small" variant="text" color="error"
                                                           :icon="true" @click="handleLLMReject(candidate.id)">
                                                        <v-icon :icon="mdiClose" />
                                                        <v-tooltip activator="parent">{{ tt('Reject') }}</v-tooltip>
                                                    </v-btn>
                                                </template>
                                                <span v-else class="text-grey text-caption">—</span>
                                            </td>
                                        </tr>
                                    </tbody>
                                </v-table>

                                <v-empty-state v-if="filteredLLMCandidates.length === 0"
                                               :icon="mdiRobotOutline"
                                               :headline="tt('No Suggested Rules')"
                                               :text="tt('No LLM suggested rules are waiting for review.')" />
            </template>
        </v-card-text>
    </v-card>

    <!-- Edit Rule Dialog -->
    <v-dialog v-model="editRuleDialog" max-width="640" persistent>
        <v-card class="pa-2 pa-sm-4 pa-md-8">
            <template #title>
                <div class="d-flex align-center justify-center">
                    <div class="d-flex w-100 align-center justify-center">
                        <h4 class="text-h4">{{ tt('Edit Rule') }}</h4>
                    </div>
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                           :icon="true" @click="editRuleDialog = false">
                        <v-icon :icon="mdiClose" size="24" />
                    </v-btn>
                </div>
            </template>
            <v-card-text class="mt-md-4 pt-0">
                <v-text-field v-model="editRuleForm.matchValue" :label="tt('Match Value')"
                              density="compact" class="mb-3" />
                <v-text-field v-model="editRuleForm.learnedType" :label="tt('Learned Type')"
                              density="compact" class="mb-3" />
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn variant="text" @click="editRuleDialog = false">{{ tt('Cancel') }}</v-btn>
                <v-btn color="primary" variant="tonal" :loading="editRuleSaving"
                       @click="saveEditRule">{{ tt('Save') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>

    <!-- Add Config Dialog -->
    <v-dialog v-model="addConfigDialog" max-width="800" persistent>
        <v-card class="pa-2 pa-sm-4 pa-md-8">
            <template #title>
                <div class="d-flex align-center justify-center">
                    <div class="d-flex w-100 align-center justify-center">
                        <h4 class="text-h4">{{ tt('Add Config') }}</h4>
                    </div>
                    <v-btn density="comfortable" color="default" variant="text" class="ms-2"
                           :icon="true" @click="closeAddConfigDialog">
                        <v-icon :icon="mdiClose" size="24" />
                    </v-btn>
                </div>
            </template>
            <form autocomplete="off" @submit.prevent="saveNewConfig">
                <v-card-text class="mt-md-4 pt-0">
                    <input class="llm-config-autofill-decoy"
                           type="text"
                           name="username"
                           autocomplete="username"
                           tabindex="-1"
                           aria-hidden="true" />
                    <input class="llm-config-autofill-decoy"
                           type="password"
                           name="password"
                           autocomplete="current-password"
                           tabindex="-1"
                           aria-hidden="true" />
                    <v-text-field v-model="newConfigForm.name" :label="tt('Name')"
                                  :name="llmConfigFieldNames.name"
                                  :readonly="autofillFieldsLocked"
                                  autocomplete="off"
                                  data-lpignore="true"
                                  data-1p-ignore="true"
                                  @focus="unlockAutofillFields"
                                  density="compact" class="mb-3" placeholder="My OpenAI Config" />
                    <v-select v-model="newConfigForm.provider" :label="tt('Provider')"
                              :items="llmProviderOptions"
                              item-title="title"
                              item-value="value"
                              :name="llmConfigFieldNames.provider"
                              autocomplete="off"
                              density="compact" class="mb-3" />
                    <v-text-field v-model="newConfigForm.model" :label="tt('Model')"
                                  :name="llmConfigFieldNames.model"
                                  :readonly="autofillFieldsLocked"
                                  autocomplete="off"
                                  data-lpignore="true"
                                  data-1p-ignore="true"
                                  @focus="unlockAutofillFields"
                                  density="compact" class="mb-3"
                                  :placeholder="selectedLLMProviderOption.modelPlaceholder" />
                    <v-text-field v-model="newConfigForm.api_key" label="API Key"
                                  :name="llmConfigFieldNames.apiKey"
                                  :readonly="autofillFieldsLocked"
                                  autocomplete="new-password"
                                  data-lpignore="true"
                                  data-1p-ignore="true"
                                  @focus="unlockAutofillFields"
                                  density="compact" class="mb-3" type="password"
                                  :placeholder="selectedLLMProviderOption.apiKeyPlaceholder" />
                    <v-text-field v-model="newConfigForm.base_url" :label="baseUrlFieldLabel"
                                  :name="llmConfigFieldNames.baseUrl"
                                  :readonly="autofillFieldsLocked"
                                  autocomplete="off"
                                  data-lpignore="true"
                                  data-1p-ignore="true"
                                  @focus="unlockAutofillFields"
                                  density="compact"
                                  :placeholder="selectedLLMProviderOption.baseUrlPlaceholder" />
                    <v-switch v-model="newConfigForm.advancedMode"
                              :label="tt('Advanced Mode')"
                              color="primary"
                              density="compact"
                              hide-details
                              class="mb-2" />
                    <v-expand-transition>
                        <div v-if="newConfigForm.advancedMode" class="llm-config-advanced-fields">
                            <v-select v-model="newConfigForm.reasoning_depth"
                                      :label="tt('Reasoning Depth')"
                                      :items="llmReasoningDepthOptions"
                                      item-title="title"
                                      item-value="value"
                                      autocomplete="off"
                                      density="compact"
                                      class="mb-3" />
                            <div class="d-flex flex-wrap ga-3">
                                <v-text-field v-model="newConfigForm.temperature"
                                              :label="tt('Temperature')"
                                              :name="llmConfigFieldNames.temperature"
                                              type="number"
                                              min="0"
                                              max="2"
                                              step="0.1"
                                              density="compact"
                                              class="llm-config-number-field mb-3" />
                                <v-text-field v-model="newConfigForm.max_tokens"
                                              :label="tt('Max Tokens')"
                                              :name="llmConfigFieldNames.maxTokens"
                                              type="number"
                                              min="1"
                                              step="1"
                                              density="compact"
                                              class="llm-config-number-field mb-3" />
                            </div>
                            <v-textarea v-model="newConfigForm.system_prompt"
                                        :label="tt('System Prompt')"
                                        :name="llmConfigFieldNames.systemPrompt"
                                        autocomplete="off"
                                        density="compact"
                                        rows="2"
                                        auto-grow
                                        class="mb-3" />
                            <v-textarea v-model="newConfigForm.classification_prompt_template"
                                        :label="tt('Classification Prompt Template')"
                                        :name="llmConfigFieldNames.classificationPrompt"
                                        autocomplete="off"
                                        density="compact"
                                        rows="2"
                                        auto-grow
                                        class="mb-3" />
                            <v-textarea v-model="newConfigForm.rule_prompt_template"
                                        :label="tt('Rule Prompt Template')"
                                        :name="llmConfigFieldNames.rulePrompt"
                                        autocomplete="off"
                                        density="compact"
                                        rows="2"
                                        auto-grow />
                        </div>
                    </v-expand-transition>
                </v-card-text>
                <v-card-actions>
                    <v-spacer />
                    <v-btn variant="text" type="button" @click="closeAddConfigDialog">{{ tt('Cancel') }}</v-btn>
                    <v-btn color="primary" variant="tonal" :loading="addConfigSaving"
                           type="submit">{{ tt('Save') }}</v-btn>
                </v-card-actions>
            </form>
        </v-card>
    </v-dialog>

    <snack-bar ref="snackbar" />
</template>

<script setup lang="ts">
import axios from 'axios';
import { ref, computed, watch, useTemplateRef, onBeforeUnmount } from 'vue';

import SnackBar from '@/components/desktop/SnackBar.vue';
import { useI18n } from '@/locales/helpers.ts';
import { useLearningStore } from '@/stores/learning.ts';
import type { LearningSuggestion, LearningRule } from '@/models/learning_center.ts';
import { getSuggestionFeatureSummary, getRuleFeatureSummary } from '@/models/learning_center.ts';
import services, { type OCRConfigResponse } from '@/lib/services.ts';

import {
    mdiRefresh,
    mdiCheck,
    mdiClose,
    mdiDelete,
    mdiPencil,
    mdiLightbulbOutline,
    mdiBookOpenPageVariant,
    mdiRobotOutline,
    mdiChevronDown,
    mdiFilterVariant,
} from '@mdi/js';

interface LLMConfigItem {
    id: number;
    name: string;
    provider: string;
    model: string;
    base_url?: string;
    advanced_settings?: LLMAdvancedSettings;
    is_active?: boolean;
    created_at?: string;
    updated_at?: string;
}

interface LLMAdvancedSettings {
    reasoning_depth?: string;
    temperature?: number;
    max_tokens?: number;
    system_prompt?: string;
    classification_prompt_template?: string;
    rule_prompt_template?: string;
}

interface LLMConfigForm {
    name: string;
    provider: string;
    model: string;
    api_key: string;
    base_url: string;
    advancedMode: boolean;
    reasoning_depth: string;
    temperature: string;
    max_tokens: string;
    system_prompt: string;
    classification_prompt_template: string;
    rule_prompt_template: string;
}

interface LLMProviderOption {
    title: string;
    value: string;
    modelPlaceholder: string;
    apiKeyPlaceholder: string;
    baseUrlPlaceholder: string;
    requiresBaseUrl?: boolean;
}

interface LLMCandidateItem {
    id: number;
    type?: string;
    status: string;
    confidence?: number;
    rule_type?: string;
    rule_content?: string;
    expression?: string;
    reason?: string;
    category_name?: string;
    target_category?: string;
    suggested_main_category?: string;
    suggested_sub_category?: string;
    suggested_rule_expression?: string;
}

interface SelectOption {
    title: string;
    value: string;
}

const props = defineProps<{
    initTab?: string;
    hideSectionTitle?: boolean;
    headerActionsTarget?: string;
}>();

type SnackBarType = InstanceType<typeof SnackBar>;

const { tt } = useI18n();
const store = useLearningStore();
const snackbar = useTemplateRef<SnackBarType>('snackbar');

function normalizePanelTab(tab?: string): string {
    if (tab === 'rules' || tab === 'llm' || tab === 'llm-config') {
        return tab;
    }

    return 'suggestions';
}

const activeTab = ref<string>(normalizePanelTab(props.initTab));
const statusFilter = ref<string>('');
const selectedIds = ref<number[]>([]);
const ruleMatchTypeFilter = ref<string>('');
const ruleFeatureFilter = ref<string>('');
const ruleLearnedActionFilter = ref<string>('');
const ruleEnabledFilter = ref<string>('all');
const ruleAppliedFilter = ref<string>('all');
const ruleMatchTypeFilterMenu = ref(false);
const ruleFeatureFilterMenu = ref(false);
const ruleLearnedActionFilterMenu = ref(false);
const ruleEnabledFilterMenu = ref(false);
const ruleAppliedFilterMenu = ref(false);
const llmLoading = ref(false);
const llmConfigLoading = ref(false);
const ocrConfigLoading = ref(false);
const ocrConfigSaving = ref(false);
const llmSavedConfigs = ref<LLMConfigItem[]>([]);
const llmCandidates = ref<LLMCandidateItem[]>([]);
const llmStatusFilter = ref<string>('');
const selectedLLMIds = ref<number[]>([]);
const ocrConfig = ref<OCRConfigResponse>({
    provider: 'disabled',
    lang: 'chi_sim+eng',
    available_providers: ['disabled', 'tesseract', 'cloud_stub'],
    configured: false,
});
const ocrConfigForm = ref<{ provider: string; lang: string }>({
    provider: 'disabled',
    lang: 'chi_sim+eng',
});

const llmProviderOptions: LLMProviderOption[] = [
    {
        title: 'OpenAI',
        value: 'openai',
        modelPlaceholder: 'gpt-4o-mini',
        apiKeyPlaceholder: 'sk-...',
        baseUrlPlaceholder: 'https://api.openai.com/v1',
    },
    {
        title: 'Claude (Anthropic)',
        value: 'claude',
        modelPlaceholder: 'claude-sonnet-4-20250514',
        apiKeyPlaceholder: 'sk-ant-...',
        baseUrlPlaceholder: 'https://api.anthropic.com/v1',
    },
    {
        title: 'DeepSeek',
        value: 'deepseek',
        modelPlaceholder: 'deepseek-chat',
        apiKeyPlaceholder: 'sk-...',
        baseUrlPlaceholder: 'https://api.deepseek.com/v1',
    },
    {
        title: 'Ollama (local)',
        value: 'ollama',
        modelPlaceholder: 'llama3.1',
        apiKeyPlaceholder: tt('Not required'),
        baseUrlPlaceholder: 'http://localhost:11434',
    },
    {
        title: 'xAI',
        value: 'xai',
        modelPlaceholder: 'grok-3-mini',
        apiKeyPlaceholder: 'xai-...',
        baseUrlPlaceholder: 'https://api.x.ai/v1',
    },
    {
        title: 'Google (Gemini)',
        value: 'google',
        modelPlaceholder: 'gemini-2.0-flash',
        apiKeyPlaceholder: 'AIza...',
        baseUrlPlaceholder: 'https://generativelanguage.googleapis.com/v1beta/openai',
    },
    {
        title: 'OpenRouter',
        value: 'openrouter',
        modelPlaceholder: 'openai/gpt-4o-mini',
        apiKeyPlaceholder: 'sk-or-...',
        baseUrlPlaceholder: 'https://openrouter.ai/api/v1',
    },
    {
        title: 'OpenAI-compatible',
        value: 'openai_compatible',
        modelPlaceholder: 'gpt-4o-mini',
        apiKeyPlaceholder: 'sk-...',
        baseUrlPlaceholder: 'https://your-provider.example.com/v1',
        requiresBaseUrl: true,
    },
    {
        title: 'Azure OpenAI',
        value: 'azure',
        modelPlaceholder: 'deployment-name',
        apiKeyPlaceholder: 'Azure API key',
        baseUrlPlaceholder: 'https://<resource>.openai.azure.com/openai/v1',
        requiresBaseUrl: true,
    },
];

const ocrProviderOptions = computed<SelectOption[]>(() => {
    const providers = ocrConfig.value.available_providers.length
        ? ocrConfig.value.available_providers
        : ['disabled', 'tesseract', 'cloud_stub'];
    return providers.map(provider => ({
        title: ocrProviderLabel(provider),
        value: provider,
    }));
});

const defaultLLMProviderOption = llmProviderOptions[0] as LLMProviderOption;

const llmReasoningDepthOptions = [
    { title: tt('Default'), value: '' },
    { title: tt('Low'), value: 'low' },
    { title: tt('Medium'), value: 'medium' },
    { title: tt('High'), value: 'high' },
];

const legacyLLMProviderLabels: Record<string, string> = {
    anthropic: 'Claude (Anthropic)',
    'openai-compatible': 'OpenAI-compatible',
    azure_openai: 'Azure OpenAI',
};

const loading = computed(() => (
    store.suggestionsLoading
    || store.rulesLoading
    || llmLoading.value
    || llmConfigLoading.value
    || ocrConfigLoading.value
    || ocrConfigSaving.value
));
const error = computed(() => store.error);
const suggestionsTotal = computed(() => store.suggestionsTotal);
const rules = computed(() => store.rules);
const rulesTotal = computed(() => store.rulesTotal);
const hideSectionTitle = computed(() => Boolean(props.hideSectionTitle));
const hasHeaderActionsTarget = computed(() => Boolean(props.headerActionsTarget));
const headerActionsTarget = computed(() => props.headerActionsTarget || 'body');

const suggestionStatusOptions = computed<SelectOption[]>(() => [
    { title: `${tt('All')} (${suggestionsTotal.value})`, value: '' },
    { title: tt('Pending'), value: 'pending' },
    { title: tt('Accepted'), value: 'accepted' },
    { title: tt('Rejected'), value: 'rejected' },
]);

const filteredSuggestions = computed(() => {
    if (!statusFilter.value) return store.suggestions;
    return store.suggestions.filter(s => s.status === statusFilter.value);
});

const ruleMatchTypeOptions = computed<SelectOption[]>(() => {
    const options = new Map<string, string>();
    for (const rule of rules.value) {
        if (rule.matchType) {
            options.set(rule.matchType, translateMatchType(rule.matchType));
        }
    }

    return [
        { title: tt('All'), value: '' },
        ...Array.from(options, ([value, title]) => ({ title, value }))
    ];
});

const ruleEnabledFilterOptions = computed<SelectOption[]>(() => [
    { title: tt('All'), value: 'all' },
    { title: tt('Enabled'), value: 'enabled' },
    { title: tt('Disabled'), value: 'disabled' },
]);

const ruleAppliedFilterOptions = computed<SelectOption[]>(() => [
    { title: tt('All'), value: 'all' },
    { title: tt('Applied'), value: 'applied' },
    { title: tt('Not Applied'), value: 'not-applied' },
]);

const ruleFeatureFilterActive = computed(() => ruleFeatureFilter.value.trim().length > 0);
const ruleLearnedActionFilterActive = computed(() => ruleLearnedActionFilter.value.trim().length > 0);

function setRuleMatchTypeFilter(value: string): void {
    ruleMatchTypeFilter.value = value;
    ruleMatchTypeFilterMenu.value = false;
}

function clearRuleFeatureFilter(): void {
    ruleFeatureFilter.value = '';
}

function clearRuleLearnedActionFilter(): void {
    ruleLearnedActionFilter.value = '';
}

function setRuleEnabledFilter(value: string): void {
    ruleEnabledFilter.value = value;
    ruleEnabledFilterMenu.value = false;
}

function setRuleAppliedFilter(value: string): void {
    ruleAppliedFilter.value = value;
    ruleAppliedFilterMenu.value = false;
}

function containsText(source: string, query: string): boolean {
    const normalizedQuery = query.trim().toLowerCase();
    if (!normalizedQuery) {
        return true;
    }

    return source.toLowerCase().includes(normalizedQuery);
}

const filteredRules = computed(() => rules.value.filter((rule) => {
    if (ruleMatchTypeFilter.value && rule.matchType !== ruleMatchTypeFilter.value) {
        return false;
    }

    const featureText = `${rule.matchValue} ${getRuleFeatSummary(rule)}`;
    if (!containsText(featureText, ruleFeatureFilter.value)) {
        return false;
    }

    if (!containsText(rule.learnedType, ruleLearnedActionFilter.value)) {
        return false;
    }

    if (ruleEnabledFilter.value === 'enabled' && !rule.enabled) {
        return false;
    }

    if (ruleEnabledFilter.value === 'disabled' && rule.enabled) {
        return false;
    }

    if (ruleAppliedFilter.value === 'applied' && rule.appliedCount <= 0) {
        return false;
    }

    if (ruleAppliedFilter.value === 'not-applied' && rule.appliedCount > 0) {
        return false;
    }

    return true;
}));

const selectAll = computed({
    get() {
        const pending = filteredSuggestions.value.filter(s => s.status === 'pending');
        return pending.length > 0 && pending.every(s => selectedIds.value.includes(s.id));
    },
    set(val: boolean) {
        if (val) {
            selectedIds.value = filteredSuggestions.value
                .filter(s => s.status === 'pending')
                .map(s => s.id);
        } else {
            selectedIds.value = [];
        }
    }
});

const indeterminate = computed(() => {
    const pending = filteredSuggestions.value.filter(s => s.status === 'pending');
    const selectedCount = pending.filter(s => selectedIds.value.includes(s.id)).length;
    return selectedCount > 0 && selectedCount < pending.length;
});

function clearError() {
    store.error = null;
}

function showInfoMessage(message: string, options?: Record<string, unknown>): void {
    snackbar.value?.showMessage(message, options);
}

function toLLMConfigs(result: unknown): LLMConfigItem[] {
    if (!Array.isArray(result)) {
        return [];
    }

    return result.map((item) => {
        const safeItem = { ...(item as Record<string, unknown>) };
        delete safeItem['api_key'];
        return safeItem as unknown as LLMConfigItem;
    });
}

function toLLMCandidates(result: unknown): LLMCandidateItem[] {
    const rawItems = Array.isArray(result)
        ? result
        : (result && typeof result === 'object' && 'candidates' in result && Array.isArray((result as { candidates?: unknown }).candidates)
            ? (result as { candidates: unknown[] }).candidates
            : []);

    return rawItems.map((item) => {
        const record = (item && typeof item === 'object') ? item as Record<string, unknown> : {};
        const mainCategory = typeof record['suggested_main_category'] === 'string'
            ? record['suggested_main_category']
            : '';
        const subCategory = typeof record['suggested_sub_category'] === 'string'
            ? record['suggested_sub_category']
            : '';
        const categoryName = typeof record['category_name'] === 'string' && record['category_name']
            ? record['category_name']
            : [mainCategory, subCategory].filter(Boolean).join('/');
        const suggestedRuleExpression = typeof record['suggested_rule_expression'] === 'string'
            ? record['suggested_rule_expression']
            : '';
        const llmResponseRaw = typeof record['llm_response_raw'] === 'string'
            ? record['llm_response_raw']
            : '';
        let parsedReason = '';
        if (llmResponseRaw) {
            try {
                const parsed = JSON.parse(llmResponseRaw) as Record<string, unknown>;
                const payloadReason = parsed['reason'];
                const payloadExplanation = parsed['explanation'];
                parsedReason = typeof payloadReason === 'string'
                    ? payloadReason
                    : (typeof payloadExplanation === 'string' ? payloadExplanation : '');
            } catch {
                parsedReason = '';
            }
        }

        return {
            id: Number(record['id'] || 0),
            type: typeof record['type'] === 'string' ? record['type'] : undefined,
            status: typeof record['status'] === 'string' ? record['status'] : 'pending',
            confidence: typeof record['confidence'] === 'number'
                ? record['confidence']
                : Number(record['confidence'] || 0),
            rule_type: typeof record['rule_type'] === 'string'
                ? record['rule_type']
                : (typeof record['type'] === 'string' ? record['type'] : undefined),
            rule_content: typeof record['rule_content'] === 'string' && record['rule_content']
                ? record['rule_content']
                : suggestedRuleExpression,
            expression: typeof record['expression'] === 'string' && record['expression']
                ? record['expression']
                : suggestedRuleExpression,
            reason: typeof record['reason'] === 'string' && record['reason']
                ? record['reason']
                : parsedReason,
            category_name: categoryName || undefined,
            target_category: typeof record['target_category'] === 'string'
                ? record['target_category']
                : categoryName || undefined,
            suggested_main_category: mainCategory || undefined,
            suggested_sub_category: subCategory || undefined,
            suggested_rule_expression: suggestedRuleExpression || undefined,
        };
    });
}

function extractPayloadMessage(payload: unknown, depth = 0): string | null {
    if (depth > 2) {
        return null;
    }

    if (typeof payload === 'string' && payload) {
        return payload;
    }

    if (!payload || typeof payload !== 'object') {
        return null;
    }

    const errorMessage = 'error' in payload ? extractPayloadMessage(payload.error, depth + 1) : null;
    if (errorMessage) {
        return errorMessage;
    }

    const message = 'message' in payload ? extractPayloadMessage(payload.message, depth + 1) : null;
    if (message) {
        return message;
    }

    return null;
}

function getPayloadErrorMessage(payload: unknown, fallback: string): string {
    return extractPayloadMessage(payload) || fallback;
}

function getRequestErrorMessage(error: unknown, fallback: string): string {
    if (axios.isAxiosError(error)) {
        return getPayloadErrorMessage(error.response?.data, fallback);
    }

    if (error instanceof Error && error.message) {
        return error.message;
    }

    return fallback;
}

function getFeatureSummary(item: LearningSuggestion): string {
    return getSuggestionFeatureSummary(item);
}

function getRuleFeatSummary(rule: LearningRule): string {
    return getRuleFeatureSummary(rule);
}

function statusColor(status: string): string {
    switch (status) {
        case 'pending': return 'warning';
        case 'accepted': return 'success';
        case 'rejected': return 'error';
        default: return 'grey';
    }
}

function statusLabel(status: string): string {
    switch (status) {
        case 'pending': return 'Pending';
        case 'accepted': return 'Accepted';
        case 'rejected': return 'Rejected';
        default: return status;
    }
}

function translateMatchType(type: string): string {
    const map: Record<string, string> = {
        'composite': tt('Composite Rule'),
        'counterparty': tt('Counterparty'),
        'description': tt('Description'),
        'keyword': tt('Keyword'),
        'amount': tt('Amount'),
        'payment_method': tt('Payment Method'),
    };
    return map[type] || type;
}

async function refreshCurrentTab() {
    if (activeTab.value === 'suggestions') {
        await store.loadSuggestions();
    } else if (activeTab.value === 'llm') {
        await loadLLMCandidates();
    } else if (activeTab.value === 'llm-config') {
        await Promise.all([loadLLMConfigs(), loadOCRConfig()]);
    } else {
        await store.loadRules();
    }
}

async function handleGenerate() {
    const result = await store.generateSuggestions();
    if (result) {
        showInfoMessage('Generated Suggestions Summary', {
            created: result.created,
            updated: result.updated,
        });
    }
}

async function handleAccept(id: number) {
    await store.acceptSuggestion(id);
}

async function handleReject(id: number) {
    await store.rejectSuggestion(id);
}

async function handleBatchAccept() {
    if (selectedIds.value.length === 0) return;
    await store.batchAcceptSuggestions([...selectedIds.value]);
    selectedIds.value = [];
}

async function handleToggleRule(ruleId: number, enabled: boolean) {
    await store.toggleRule(ruleId, enabled);
}

async function handleDeleteRule(ruleId: number) {
    await store.deleteRule(ruleId);
}

// ── Edit Rule Dialog ──────────
const editRuleDialog = ref(false);
const editRuleSaving = ref(false);
const editRuleForm = ref({ id: 0, matchValue: '', learnedType: '' });

function openEditRuleDialog(rule: LearningRule) {
    editRuleForm.value = {
        id: rule.id,
        matchValue: rule.matchValue || '',
        learnedType: rule.learnedType || '',
    };
    editRuleDialog.value = true;
}

async function saveEditRule() {
    editRuleSaving.value = true;
    try {
        const resp = await services.updateLearningRule({
            ruleId: editRuleForm.value.id,
            matchValue: editRuleForm.value.matchValue,
            learnedType: editRuleForm.value.learnedType,
        });
        if (resp.data?.success) {
            editRuleDialog.value = false;
            await store.loadRules();
        } else {
            store.error = getPayloadErrorMessage(resp.data?.result, 'Failed to update rule');
        }
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to update rule');
    } finally {
        editRuleSaving.value = false;
    }
}

// Add Config Dialog
const addConfigDialog = ref(false);
const addConfigSaving = ref(false);
const autofillNonce = ref(Date.now());
const autofillFieldsLocked = ref(false);
const autofillUnlockTimer = ref<number | null>(null);

function createEmptyLLMConfigForm(): LLMConfigForm {
    return {
        name: '',
        provider: 'openai',
        model: '',
        api_key: '',
        base_url: '',
        advancedMode: false,
        reasoning_depth: '',
        temperature: '0.3',
        max_tokens: '4096',
        system_prompt: '',
        classification_prompt_template: '',
        rule_prompt_template: '',
    };
}

const newConfigForm = ref<LLMConfigForm>(createEmptyLLMConfigForm());

const selectedLLMProviderOption = computed<LLMProviderOption>(() => (
    llmProviderOptions.find(option => option.value === newConfigForm.value.provider) ?? defaultLLMProviderOption
));

const baseUrlFieldLabel = computed(() => (
    selectedLLMProviderOption.value.requiresBaseUrl ? tt('Base URL') : tt('Base URL (optional)')
));

const llmConfigFieldNames = computed(() => ({
    name: `llm-config-label-${autofillNonce.value}`,
    provider: `llm-config-provider-${autofillNonce.value}`,
    model: `llm-config-model-${autofillNonce.value}`,
    apiKey: `llm-config-credential-${autofillNonce.value}`,
    baseUrl: `llm-config-endpoint-${autofillNonce.value}`,
    temperature: `llm-config-temperature-${autofillNonce.value}`,
    maxTokens: `llm-config-max-tokens-${autofillNonce.value}`,
    systemPrompt: `llm-config-system-prompt-${autofillNonce.value}`,
    classificationPrompt: `llm-config-classification-prompt-${autofillNonce.value}`,
    rulePrompt: `llm-config-rule-prompt-${autofillNonce.value}`,
}));

const llmPendingCount = computed(() =>
    llmCandidates.value.filter(c => c.status === 'pending').length
);

const llmStatusOptions = computed<SelectOption[]>(() => [
    { title: tt('All'), value: '' },
    { title: tt('Pending'), value: 'pending' },
    { title: tt('Accepted'), value: 'accepted' },
    { title: tt('Rejected'), value: 'rejected' },
]);

const filteredLLMCandidates = computed(() => {
    if (!llmStatusFilter.value) return llmCandidates.value;
    return llmCandidates.value.filter(c => c.status === llmStatusFilter.value);
});

const selectableLLMCandidates = computed(() =>
    filteredLLMCandidates.value.filter(candidate => candidate.status === 'pending')
);

const selectAllLLM = computed({
    get() {
        return selectableLLMCandidates.value.length > 0
            && selectableLLMCandidates.value.every(candidate => selectedLLMIds.value.includes(candidate.id));
    },
    set(val: boolean) {
        if (val) {
            selectedLLMIds.value = selectableLLMCandidates.value.map(candidate => candidate.id);
        } else {
            selectedLLMIds.value = [];
        }
    }
});

const llmIndeterminate = computed(() => {
    const selectedCount = selectableLLMCandidates.value
        .filter(candidate => selectedLLMIds.value.includes(candidate.id))
        .length;
    return selectedCount > 0 && selectedCount < selectableLLMCandidates.value.length;
});

function llmStatusColor(status: string): string {
    switch (status) {
        case 'pending': return 'warning';
        case 'accepted': return 'success';
        case 'rejected': return 'error';
        default: return 'grey';
    }
}

function llmStatusLabel(status: string): string {
    switch (status) {
        case 'pending': return tt('Pending');
        case 'accepted': return tt('Accepted');
        case 'rejected': return tt('Rejected');
        default: return status;
    }
}

function confidenceColor(confidence: number): string {
    if (confidence >= 0.8) return 'success';
    if (confidence >= 0.5) return 'warning';
    return 'error';
}

function clearAutofillUnlockTimer(): void {
    if (autofillUnlockTimer.value === null) {
        return;
    }
    window.clearTimeout(autofillUnlockTimer.value);
    autofillUnlockTimer.value = null;
}

function unlockAutofillFields(): void {
    autofillFieldsLocked.value = false;
    clearAutofillUnlockTimer();
}

function lockAutofillFieldsBriefly(): void {
    clearAutofillUnlockTimer();
    autofillFieldsLocked.value = true;
    autofillUnlockTimer.value = window.setTimeout(() => {
        autofillFieldsLocked.value = false;
        autofillUnlockTimer.value = null;
    }, 350);
}

function closeAddConfigDialog(): void {
    clearAutofillUnlockTimer();
    autofillFieldsLocked.value = false;
    addConfigDialog.value = false;
}

function buildAdvancedSettingsPayload(form: LLMConfigForm): LLMAdvancedSettings {
    if (!form.advancedMode) {
        return {};
    }

    const settings: LLMAdvancedSettings = {};
    if (form.reasoning_depth) {
        settings.reasoning_depth = form.reasoning_depth;
    }

    const temperature = Number(form.temperature);
    if (Number.isFinite(temperature)) {
        settings.temperature = temperature;
    }

    const maxTokens = Number.parseInt(form.max_tokens, 10);
    if (Number.isFinite(maxTokens)) {
        settings.max_tokens = maxTokens;
    }

    const systemPrompt = form.system_prompt.trim();
    if (systemPrompt) {
        settings.system_prompt = systemPrompt;
    }

    const classificationPrompt = form.classification_prompt_template.trim();
    if (classificationPrompt) {
        settings.classification_prompt_template = classificationPrompt;
    }

    const rulePrompt = form.rule_prompt_template.trim();
    if (rulePrompt) {
        settings.rule_prompt_template = rulePrompt;
    }

    return settings;
}

async function loadLLMConfigs() {
    llmConfigLoading.value = true;
    try {
        const resp = await services.getLLMConfigs();
        if (resp.data?.success && resp.data.result) {
            llmSavedConfigs.value = toLLMConfigs(resp.data.result);
        }
    } catch { /* ignore config load errors */ }
    finally {
        llmConfigLoading.value = false;
    }
}

function openAddConfigDialog() {
    autofillNonce.value = Date.now();
    newConfigForm.value = createEmptyLLMConfigForm();
    addConfigDialog.value = true;
    lockAutofillFieldsBriefly();
}

async function saveNewConfig() {
    const form = newConfigForm.value;
    const payload = {
        name: form.name.trim(),
        provider: form.provider,
        model: form.model.trim(),
        api_key: form.api_key.trim(),
        base_url: form.base_url.trim(),
        advanced_settings: buildAdvancedSettingsPayload(form),
        is_active: llmSavedConfigs.value.length === 0,
    };

    if (!payload.name) {
        store.error = 'Name is required';
        return;
    }

    if (selectedLLMProviderOption.value.requiresBaseUrl && !payload.base_url) {
        store.error = `${selectedLLMProviderOption.value.title} requires a Base URL`;
        return;
    }

    addConfigSaving.value = true;
    try {
        const resp = await services.createLLMConfig(payload);
        if (resp.data?.success) {
            closeAddConfigDialog();
            newConfigForm.value = createEmptyLLMConfigForm();
            await loadLLMConfigs();
        } else {
            store.error = getPayloadErrorMessage(resp.data?.result, 'Failed to create config');
        }
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to create config');
    } finally {
        addConfigSaving.value = false;
    }
}

onBeforeUnmount(() => {
    clearAutofillUnlockTimer();
});

function llmProviderLabel(provider: string): string {
    return llmProviderOptions.find(option => option.value === provider)?.title
        ?? legacyLLMProviderLabels[provider]
        ?? provider;
}

function ocrProviderLabel(provider: string): string {
    const labels: Record<string, string> = {
        disabled: tt('Disabled'),
        tesseract: 'Tesseract',
        cloud_stub: 'Cloud Stub',
    };
    return labels[provider] ?? provider;
}

function applyOCRConfig(config: OCRConfigResponse): void {
    ocrConfig.value = {
        provider: config.provider || 'disabled',
        lang: config.lang || 'chi_sim+eng',
        available_providers: Array.isArray(config.available_providers) && config.available_providers.length
            ? config.available_providers
            : ['disabled', 'tesseract', 'cloud_stub'],
        configured: !!config.configured,
    };
    ocrConfigForm.value = {
        provider: ocrConfig.value.provider,
        lang: ocrConfig.value.lang,
    };
}

async function loadOCRConfig() {
    ocrConfigLoading.value = true;
    try {
        const resp = await services.getOCRConfig();
        if (resp.data?.success && resp.data.result) {
            applyOCRConfig(resp.data.result);
        }
    } catch { /* ignore config load errors */ }
    finally {
        ocrConfigLoading.value = false;
    }
}

async function saveOCRConfig() {
    ocrConfigSaving.value = true;
    try {
        const resp = await services.updateOCRConfig({
            provider: ocrConfigForm.value.provider,
            lang: ocrConfigForm.value.lang.trim() || 'chi_sim+eng',
        });
        if (resp.data?.success && resp.data.result) {
            applyOCRConfig(resp.data.result);
            showInfoMessage('OCR Config Saved', {});
        } else {
            store.error = getPayloadErrorMessage(resp.data?.result, 'Failed to save OCR config');
        }
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to save OCR config');
    } finally {
        ocrConfigSaving.value = false;
    }
}

async function handleActivateConfig(configId: number) {
    try {
        await services.activateLLMConfig(configId);
        await loadLLMConfigs();
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to activate config');
    }
}

async function handleDeleteConfig(configId: number) {
    try {
        await services.deleteLLMConfig(configId);
        await loadLLMConfigs();
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to delete config');
    }
}

async function loadLLMCandidates() {
    llmLoading.value = true;
    try {
        const resp = await services.getLLMCandidates({ type: 'rule_synthesis', limit: 100 });
        if (resp.data?.success && resp.data.result) {
            llmCandidates.value = toLLMCandidates(resp.data.result);
            selectedLLMIds.value = selectedLLMIds.value.filter(id =>
                llmCandidates.value.some(candidate => candidate.id === id && candidate.status === 'pending')
            );
        }
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to load LLM candidates');
    } finally {
        llmLoading.value = false;
    }
}

async function handleLLMGenerate() {
    llmLoading.value = true;
    try {
        const resp = await services.generateLLMRuleSynthesis({ limit: 8 });
        const created = resp.data?.result?.candidates_created ?? 0;
        showInfoMessage('Generated LLM Rule Candidates Summary', { count: created });
        await loadLLMCandidates();
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to generate LLM rule candidates');
    } finally {
        llmLoading.value = false;
    }
}

async function handleLLMBatchAccept() {
    const ids = [...selectedLLMIds.value];
    if (ids.length === 0) {
        return;
    }

    llmLoading.value = true;
    try {
        for (const id of ids) {
            await services.acceptLLMCandidate(id);
        }
        selectedLLMIds.value = [];
        await loadLLMCandidates();
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to batch accept LLM rule candidates');
    } finally {
        llmLoading.value = false;
    }
}

async function handleLLMAccept(id: number) {
    llmLoading.value = true;
    try {
        await services.acceptLLMCandidate(id);
        selectedLLMIds.value = selectedLLMIds.value.filter(selectedId => selectedId !== id);
        await loadLLMCandidates();
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to accept candidate');
    } finally {
        llmLoading.value = false;
    }
}

async function handleLLMReject(id: number) {
    llmLoading.value = true;
    try {
        await services.rejectLLMCandidate(id);
        selectedLLMIds.value = selectedLLMIds.value.filter(selectedId => selectedId !== id);
        await loadLLMCandidates();
    } catch (error: unknown) {
        store.error = getRequestErrorMessage(error, 'Failed to reject candidate');
    } finally {
        llmLoading.value = false;
    }
}

watch(
    () => props.initTab,
    (initTab) => {
        activeTab.value = normalizePanelTab(initTab);
    },
    { immediate: true }
);

watch(activeTab, (tab) => {
    if (tab === 'suggestions') {
        store.loadSuggestions();
    } else if (tab === 'llm') {
        loadLLMCandidates();
    } else if (tab === 'llm-config') {
        loadLLMConfigs();
        loadOCRConfig();
    } else {
        store.loadRules();
    }
}, { immediate: true });
</script>

<style scoped>
.learning-center-panel {
    background: transparent;
}

.learning-section-header {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 38px;
    margin-bottom: 12px;
}

.learning-section-header--empty {
    min-height: 0;
    margin-bottom: 0;
}

.learning-section-title {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    line-height: 1.5;
}

.learning-section-actions {
    display: flex;
    flex: 1 1 auto;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-start;
    gap: 8px;
}

.learning-section-actions--external {
    min-width: 0;
    width: 100%;
}

.learning-section-actions--split {
    flex-wrap: nowrap;
}

.learning-section-actions__primary {
    display: flex;
    flex: 0 0 auto;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
}

.learning-section-actions__spacer {
    flex: 1 1 auto;
    min-width: 16px;
}

.learning-panel-action {
    height: 38px;
    min-height: 38px;
    min-width: 136px;
    padding-inline: 18px;
    text-transform: none;
    letter-spacing: normal;
}

.learning-panel-refresh {
    width: 38px;
    height: 38px;
    min-width: 38px;
    min-height: 38px;
}

.learning-panel-refresh :deep(.v-icon) {
    font-size: 22px;
    width: 22px;
    height: 22px;
}

.learning-status-select {
    flex: 0 0 200px;
    max-width: 210px;
}

.learning-status-select :deep(.v-field) {
    min-height: 38px;
    height: 38px;
}

.learning-status-select :deep(.v-field__input) {
    min-height: 38px;
    padding-top: 0;
    padding-bottom: 0;
    text-align: center;
    justify-content: center;
}

.learning-status-select :deep(.v-select__selection) {
    justify-content: center;
    width: 100%;
}

.learning-status-select :deep(.v-select__selection-text) {
    width: 100%;
    text-align: center;
}

.learning-rules-table :deep(th) {
    padding-block: 8px;
}

.learning-rules-table :deep(td) {
    text-align: center;
}

.learning-rules-feature-cell {
    margin-inline: auto;
}

.learning-rules-actions {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 2px;
    flex-wrap: nowrap;
}

.learning-rules-table :deep(.v-switch) {
    display: inline-flex;
    justify-content: center;
}

.learning-header-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 32px;
    width: 100%;
    padding-inline: 4px;
    text-transform: none;
    letter-spacing: normal;
}

.learning-header-menu {
    max-width: min(360px, 90vw);
}

.llm-config-autofill-decoy {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: 0;
    padding: 0;
    overflow: hidden;
    opacity: 0;
    pointer-events: none;
    border: 0;
}

.llm-config-advanced-fields {
    border-top: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
    margin-top: 8px;
    padding-top: 12px;
}

.llm-config-number-field {
    flex: 1 1 180px;
}

.v-table :deep(td) {
    padding-block: 12px;
    vertical-align: middle;
}

.v-table :deep(th) {
    white-space: nowrap;
    text-align: center;
    vertical-align: middle;
}
</style>
