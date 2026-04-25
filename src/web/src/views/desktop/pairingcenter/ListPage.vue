<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-layout class="rule-center-layout">
                    <v-navigation-drawer :permanent="alwaysShowNav"
                                         v-model="showNav"
                                         class="rule-center-navigation">
                        <div class="mx-6 mt-4">
                            <btn-vertical-group
                                class="rule-center-nav-buttons"
                                :buttons="primaryNavButtons"
                                :model-value="activePrimary"
                                @update:model-value="selectPrimaryNav"
                            />
                        </div>
                        <v-divider class="mt-4" />
                        <v-tabs show-arrows
                                class="my-4"
                                direction="vertical"
                                :model-value="activeSecondary"
                                @update:model-value="selectSecondaryNav">
                            <v-tab
                                v-for="tab in secondaryTabs"
                                :key="tab.value"
                                class="tab-text-truncate"
                                :value="tab.value">
                                <span class="text-truncate">{{ tab.label }}</span>
                            </v-tab>
                        </v-tabs>
                    </v-navigation-drawer>

                    <v-main>
                        <v-card variant="flat" min-height="760">
                            <template #title>
                                <div class="title-and-toolbar d-flex align-center text-no-wrap">
                                    <v-btn class="me-3 d-md-none"
                                           density="compact"
                                           color="default"
                                           variant="plain"
                                           :ripple="false"
                                           :icon="true"
                                           @click="showNav = !showNav">
                                        <v-icon :icon="mdiMenu" size="24" />
                                    </v-btn>

                                    <span>{{ currentPageTitle }}</span>

                                    <v-btn v-if="showHeaderRefresh"
                                           color="default"
                                           variant="text"
                                           density="compact"
                                           size="24"
                                           class="ms-2"
                                           :icon="true"
                                           :loading="activeToolbarRefreshing"
                                           @click="refreshActiveView">
                                        <template #loader>
                                            <v-progress-circular indeterminate size="20"/>
                                        </template>
                                        <v-icon :icon="mdiRefresh" size="24" />
                                        <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                                    </v-btn>

                                    <v-spacer />

                                    <div
                                        v-if="showLearningHeaderActions"
                                        :id="learningHeaderActionsTargetId"
                                        class="rule-center-title-actions ms-3"
                                    />

                                    <v-chip
                                        v-if="isPairingOverview"
                                        class="ms-2 rule-center-pair-count-chip"
                                        color="primary"
                                        variant="tonal"
                                        size="small"
                                        label
                                    >
                                        {{ filteredPairs.length }} {{ tt('pairs') }}
                                    </v-chip>
                                </div>
                            </template>

                            <v-card-text>
                                <v-progress-linear v-if="showPairsLoading" indeterminate color="primary" class="mb-4" />

                                <v-alert v-if="showPairsError"
                                         type="error"
                                         closable
                                         class="mb-4"
                                         @click:close="error = null">
                                    {{ error }}
                                </v-alert>

                                <template v-if="isPairingOverview">
                                    <pairs-overview-table v-if="!loading"
                                                          :pairs="filteredPairs"
                                                          :deleting-pair-id="deleting"
                                                          :empty-headline="tt(activeDomain === 'transfer' ? 'No Transfer Pairs' : 'No Investment Pairs')"
                                                          @delete="confirmDeletePair" />
                                </template>

                                <template v-else-if="activeDomain === 'transfer' && activeTab === 'rules' && activeLegacyRuleTab === 'rules'">
                                    <div class="embedded-rule-panel">
                                        <rule-center-panel init-tab="rules"
                                                           ref="categoryRulePanel"
                                                           :tabs="['rules']"
                                                           :title="tt('Category Recognition')"
                                                           hide-header />
                                    </div>
                                </template>

                                <template v-else-if="activeDomain === 'transfer' && activeTab === 'rules' && activeLegacyRuleTab === 'recurring'">
                                    <div class="embedded-rule-panel">
                                        <rule-center-panel init-tab="recurring"
                                                           ref="recurringRulePanel"
                                                           :tabs="['recurring']"
                                                           :title="tt('Recurring Recognition')"
                                                           hide-header />
                                    </div>
                                </template>

                                <template v-else-if="activeDomain === 'investment' && activeTab === 'rules'">
                                    <investment-recognition-settings-card
                                        ref="investmentRecognitionSettings"
                                        hide-header
                                    />
                                </template>

                                <template v-else-if="activeDomain === 'learning' && activeTab === 'overview'">
                                    <learning-center-panel
                                        key="learning-overview"
                                        init-tab="suggestions"
                                        hide-section-title
                                        :header-actions-target="learningHeaderActionsTarget" />
                                </template>

                                <template v-else-if="activeDomain === 'learning' && activeTab === 'rules'">
                                    <learning-center-panel
                                        key="learning-rules"
                                        init-tab="rules"
                                        hide-section-title
                                        :header-actions-target="learningHeaderActionsTarget" />
                                </template>

                                <template v-else-if="activeDomain === 'llm' && activeTab === 'overview'">
                                    <learning-center-panel
                                        key="llm-overview"
                                        init-tab="llm"
                                        hide-section-title
                                        :header-actions-target="learningHeaderActionsTarget" />
                                </template>

                                <template v-else-if="activeDomain === 'llm' && activeTab === 'config'">
                                    <learning-center-panel
                                        key="llm-config"
                                        init-tab="llm-config"
                                        hide-section-title
                                        :header-actions-target="learningHeaderActionsTarget" />
                                </template>
                            </v-card-text>
                        </v-card>
                    </v-main>
                </v-layout>
            </v-card>
        </v-col>
    </v-row>

    <v-dialog v-model="showDeleteDialog" max-width="400" persistent>
        <v-card>
            <v-card-title>{{ tt('Delete Pair') }}</v-card-title>
            <v-card-text>
                {{ tt('Are you sure you want to delete this matching pair?') }}
            </v-card-text>
            <v-card-actions>
                <v-spacer />
                <v-btn @click="showDeleteDialog = false">{{ tt('Cancel') }}</v-btn>
                <v-btn color="error" variant="tonal" :loading="deleting !== null"
                       @click="doDeletePair">{{ tt('Delete') }}</v-btn>
            </v-card-actions>
        </v-card>
    </v-dialog>
</template>

<script lang="ts" setup>
import { computed, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useDisplay } from 'vuetify';

import {
    mdiMenu,
    mdiRefresh,
} from '@mdi/js';

import type { BillMatchingPairDetail } from '@/models/bill_matching.ts';
import { useMatchingStore } from '@/stores/matching.ts';
import { useI18n } from '@/locales/helpers.ts';
import InvestmentRecognitionSettingsCard from '@/views/desktop/pairingcenter/components/InvestmentRecognitionSettingsCard.vue';
import LearningCenterPanel from '@/views/desktop/pairingcenter/components/LearningCenterPanel.vue';
import PairsOverviewTable from '@/views/desktop/pairingcenter/components/PairsOverviewTable.vue';
import RuleCenterPanel from '@/views/desktop/pairingcenter/components/RuleCenterPanel.vue';
import {
    buildRuleCenterQuery,
    normalizeRuleCenterSelection,
    normalizeRuleCenterTab,
    type LegacyRuleTab,
    type RuleCenterDomain,
    type RuleCenterSelection,
    type RuleCenterTab,
} from '@/views/desktop/pairingcenter/rule_center_navigation.ts';

type PrimaryNavValue = 'pairing-overview' | 'rule-config' | 'learning' | 'llm';
type SecondaryNavValue =
    | 'transfer-overview'
    | 'investment-overview'
    | 'category-recognition'
    | 'investment-recognition'
    | 'recurring-recognition'
    | 'learning-overview'
    | 'learning-rules'
    | 'llm-recognition'
    | 'llm-config';

type PrimaryNavOption = {
    value: PrimaryNavValue;
    label: string;
};

type SecondaryTabOption = {
    value: SecondaryNavValue;
    label: string;
    selection: Pick<RuleCenterSelection, 'domain' | 'tab' | 'legacyRuleTab'>;
};

const props = defineProps<{
    initDomain?: unknown;
    initPairType?: unknown;
    initView?: unknown;
    initTab?: unknown;
}>();

const route = useRoute();
const router = useRouter();
const display = useDisplay();
const { tt } = useI18n();
const matchingStore = useMatchingStore();

const initialSelection = normalizeRuleCenterSelection({
    domain: props.initDomain,
    tab: props.initTab,
    view: props.initView,
    pairType: props.initPairType,
});

const activeDomain = ref<RuleCenterDomain>(initialSelection.domain);
const activeTab = ref<RuleCenterTab>(initialSelection.tab);
const activeLegacyRuleTab = ref<LegacyRuleTab>(initialSelection.legacyRuleTab);
const alwaysShowNav = ref<boolean>(display.mdAndUp.value);
const showNav = ref<boolean>(display.mdAndUp.value);
const showDeleteDialog = ref(false);
const pairToDelete = ref<BillMatchingPairDetail | null>(null);
const deleting = ref<number | null>(null);
const rulePanelRefreshing = ref(false);
const investmentSettingsRefreshing = ref(false);

interface RefreshablePanel {
    refresh: () => Promise<void>;
}

const categoryRulePanel = ref<RefreshablePanel | null>(null);
const recurringRulePanel = ref<RefreshablePanel | null>(null);
const investmentRecognitionSettings = ref<RefreshablePanel | null>(null);
const learningHeaderActionsTargetId = 'rule-center-learning-title-actions';
const learningHeaderActionsTarget = `#${learningHeaderActionsTargetId}`;

const loading = computed(() => matchingStore.loading);
const error = computed({
    get: () => matchingStore.error,
    set: (val) => { matchingStore.error = val; }
});

const primaryNavButtons = computed<PrimaryNavOption[]>(() => [
    {
        value: 'pairing-overview',
        label: tt('Pairing Overview'),
    },
    {
        value: 'rule-config',
        label: tt('Rules Configuration'),
    },
    {
        value: 'learning',
        label: tt('Long-term Learning'),
    },
    {
        value: 'llm',
        label: tt('LLM Recognition'),
    },
]);

const activePrimary = computed<PrimaryNavValue>(() => {
    if (activeDomain.value === 'learning') {
        return 'learning';
    }

    if (activeDomain.value === 'llm') {
        return 'llm';
    }

    if (activeTab.value === 'rules') {
        return 'rule-config';
    }

    return 'pairing-overview';
});

const activeSecondary = computed<SecondaryNavValue>(() => {
    if (activeDomain.value === 'investment' && activeTab.value === 'overview') {
        return 'investment-overview';
    }

    if (activeDomain.value === 'investment' && activeTab.value === 'rules') {
        return 'investment-recognition';
    }

    if (activeDomain.value === 'transfer' && activeTab.value === 'rules' && activeLegacyRuleTab.value === 'recurring') {
        return 'recurring-recognition';
    }

    if (activeDomain.value === 'transfer' && activeTab.value === 'rules') {
        return 'category-recognition';
    }

    if (activeDomain.value === 'learning' && activeTab.value === 'rules') {
        return 'learning-rules';
    }

    if (activeDomain.value === 'learning') {
        return 'learning-overview';
    }

    if (activeDomain.value === 'llm' && activeTab.value === 'config') {
        return 'llm-config';
    }

    if (activeDomain.value === 'llm') {
        return 'llm-recognition';
    }

    return 'transfer-overview';
});

function secondaryTabsForPrimary(primary: PrimaryNavValue): SecondaryTabOption[] {
    if (primary === 'pairing-overview') {
        return [
            {
                value: 'transfer-overview',
                label: tt('Transfer Pairing'),
                selection: {
                    domain: 'transfer',
                    tab: 'overview',
                    legacyRuleTab: 'rules',
                },
            },
            {
                value: 'investment-overview',
                label: tt('Investment Pairing'),
                selection: {
                    domain: 'investment',
                    tab: 'overview',
                    legacyRuleTab: 'rules',
                },
            },
        ];
    }

    if (primary === 'rule-config') {
        return [
            {
                value: 'category-recognition',
                label: tt('Category Recognition'),
                selection: {
                    domain: 'transfer',
                    tab: 'rules',
                    legacyRuleTab: 'rules',
                },
            },
            {
                value: 'investment-recognition',
                label: tt('Investment Recognition'),
                selection: {
                    domain: 'investment',
                    tab: 'rules',
                    legacyRuleTab: 'rules',
                },
            },
            {
                value: 'recurring-recognition',
                label: tt('Recurring Recognition'),
                selection: {
                    domain: 'transfer',
                    tab: 'rules',
                    legacyRuleTab: 'recurring',
                },
            },
        ];
    }

    if (primary === 'learning') {
        return [
            {
                value: 'learning-overview',
                label: tt('Auto Suggestions'),
                selection: {
                    domain: 'learning',
                    tab: 'overview',
                    legacyRuleTab: 'learning',
                },
            },
            {
                value: 'learning-rules',
                label: tt('Learning Rules'),
                selection: {
                    domain: 'learning',
                    tab: 'rules',
                    legacyRuleTab: 'learning',
                },
            },
        ];
    }

    return [
        {
            value: 'llm-recognition',
            label: tt('LLM Recognition'),
            selection: {
                domain: 'llm',
                tab: 'overview',
                legacyRuleTab: 'learning',
            },
        },
        {
            value: 'llm-config',
            label: tt('LLM Config'),
            selection: {
                domain: 'llm',
                tab: 'config',
                legacyRuleTab: 'learning',
            },
        },
    ];
}

const secondaryTabs = computed<SecondaryTabOption[]>(() => secondaryTabsForPrimary(activePrimary.value));

const currentTabOption = computed<SecondaryTabOption>(() => {
    return secondaryTabs.value.find(option => option.value === activeSecondary.value) ?? secondaryTabs.value[0]!;
});

const isPairingOverview = computed(() =>
    (activeDomain.value === 'transfer' || activeDomain.value === 'investment') && activeTab.value === 'overview'
);
const isCategoryRecognition = computed(() =>
    activeDomain.value === 'transfer' && activeTab.value === 'rules' && activeLegacyRuleTab.value === 'rules'
);
const isRecurringRecognition = computed(() =>
    activeDomain.value === 'transfer' && activeTab.value === 'rules' && activeLegacyRuleTab.value === 'recurring'
);
const isInvestmentRecognition = computed(() =>
    activeDomain.value === 'investment' && activeTab.value === 'rules'
);
const showLearningHeaderActions = computed(() =>
    activeDomain.value === 'learning' || activeDomain.value === 'llm'
);
const currentPageTitle = computed(() => (
    isInvestmentRecognition.value
        ? tt('Investment Recognition Settings')
        : currentTabOption.value.label
));
const filteredPairs = computed(() => matchingStore.pairs.filter(pair => pair.pairType === activeDomain.value));
const showPairsLoading = computed(() => isPairingOverview.value && loading.value);
const showPairsError = computed(() => isPairingOverview.value && !!error.value);
const canRefreshActiveView = computed(() => (
    isPairingOverview.value
        || isCategoryRecognition.value
        || isRecurringRecognition.value
        || isInvestmentRecognition.value
));
const showHeaderRefresh = computed(() => (
    canRefreshActiveView.value && !isCategoryRecognition.value
));
const activeToolbarRefreshing = computed(() => (
    isPairingOverview.value
        ? loading.value
        : isInvestmentRecognition.value
            ? investmentSettingsRefreshing.value
            : rulePanelRefreshing.value
));

function collapseNavOnMobile(): void {
    if (!alwaysShowNav.value) {
        showNav.value = false;
    }
}

function syncQuery(domain: RuleCenterDomain, tab: RuleCenterTab, legacyRuleTab: LegacyRuleTab): void {
    void router.replace({
        path: '/pairing/list',
        query: buildRuleCenterQuery(route.query, domain, tab, legacyRuleTab),
    });
}

function applySelection(selection: Pick<RuleCenterSelection, 'domain' | 'tab' | 'legacyRuleTab'>): void {
    const nextTab = normalizeRuleCenterTab(selection.domain, selection.tab);
    const unchanged = activeDomain.value === selection.domain
        && activeTab.value === nextTab
        && activeLegacyRuleTab.value === selection.legacyRuleTab;

    activeDomain.value = selection.domain;
    activeTab.value = nextTab;
    activeLegacyRuleTab.value = selection.legacyRuleTab;
    collapseNavOnMobile();

    if (unchanged) {
        return;
    }

    syncQuery(activeDomain.value, activeTab.value, activeLegacyRuleTab.value);
    void refreshActiveView();
}

function selectPrimaryNav(value: unknown): void {
    const targetPrimary = String(value) as PrimaryNavValue;
    const firstTarget = (activePrimary.value === targetPrimary
        ? currentTabOption.value
        : secondaryTabsForPrimary(targetPrimary)[0]);

    if (firstTarget) {
        applySelection(firstTarget.selection);
    }
}

function selectSecondaryNav(value: unknown): void {
    const targetValue = String(value) as SecondaryNavValue;
    const target = secondaryTabs.value.find(option => option.value === targetValue);

    if (target) {
        applySelection(target.selection);
    }
}

async function refreshActiveView(): Promise<void> {
    if (isPairingOverview.value) {
        await matchingStore.loadPairs();
        return;
    }

    if (isInvestmentRecognition.value) {
        investmentSettingsRefreshing.value = true;
        try {
            await investmentRecognitionSettings.value?.refresh();
        } finally {
            investmentSettingsRefreshing.value = false;
        }
        return;
    }

    const activeRulePanel = isCategoryRecognition.value
        ? categoryRulePanel.value
        : isRecurringRecognition.value
            ? recurringRulePanel.value
            : null;

    if (activeRulePanel) {
        rulePanelRefreshing.value = true;
        try {
            await activeRulePanel.refresh();
        } finally {
            rulePanelRefreshing.value = false;
        }
    }
}

function confirmDeletePair(pair: BillMatchingPairDetail): void {
    pairToDelete.value = pair;
    showDeleteDialog.value = true;
}

async function doDeletePair(): Promise<void> {
    if (!pairToDelete.value) {
        return;
    }

    deleting.value = pairToDelete.value.id;
    await matchingStore.deletePair(pairToDelete.value.id);
    deleting.value = null;
    showDeleteDialog.value = false;
    pairToDelete.value = null;
}

watch(() => display.mdAndUp.value, (newValue) => {
    alwaysShowNav.value = newValue;

    if (!showNav.value) {
        showNav.value = newValue;
    }
});

watch(
    () => [props.initDomain, props.initView, props.initPairType, props.initTab] as const,
    ([initDomain, initView, initPairType, initTab]) => {
        const selection = normalizeRuleCenterSelection({
            domain: initDomain,
            tab: initTab,
            view: initView,
            pairType: initPairType,
        });

        activeDomain.value = selection.domain;
        activeTab.value = selection.tab;
        activeLegacyRuleTab.value = selection.legacyRuleTab;

        if (selection.shouldRewriteQuery) {
            syncQuery(selection.domain, selection.tab, selection.legacyRuleTab);
        }
    },
    { immediate: true }
);

watch(
    isPairingOverview,
    (enabled) => {
        if (enabled && !matchingStore.loading && matchingStore.pairs.length === 0) {
            void matchingStore.loadPairs();
        }
    },
    { immediate: true }
);
</script>

<style scoped>
.rule-center-layout {
    min-height: 760px;
}

.rule-center-navigation {
    border-inline-end: 1px solid rgba(var(--v-theme-on-surface), 0.12);
}

.rule-center-nav-buttons {
    width: 100%;
}

.rule-center-nav-buttons:deep(.v-btn) {
    width: 100%;
}

.tab-text-truncate {
    justify-content: flex-start;
    padding-inline: 12px;
}

.tab-text-truncate .text-truncate {
    width: 100%;
    text-align: left;
}

.embedded-rule-panel :deep(> .v-row) {
    margin: 0;
}

.embedded-rule-panel :deep(> .v-row > .v-col) {
    padding: 0;
}

.rule-center-pair-count-chip {
    cursor: default;
    user-select: none;
}

.rule-center-title-actions {
    display: flex;
    flex: 0 1 auto;
    min-width: 0;
    align-items: center;
    justify-content: flex-end;
}
</style>
