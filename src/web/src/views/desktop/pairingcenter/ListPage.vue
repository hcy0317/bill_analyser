<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-layout class="rule-center-layout">
                    <v-navigation-drawer width="320"
                                         :permanent="alwaysShowNav"
                                         v-model="showNav"
                                         class="rule-center-navigation">
                        <div class="rule-center-nav-stack mx-4 my-4">
                            <div class="rule-center-nav-group">
                                <v-btn v-for="domain in domainOptions"
                                       :key="domain.value"
                                       block
                                       border
                                       class="rule-center-nav-button text-none"
                                       :color="activeDomain === domain.value ? 'primary' : 'default'"
                                       :variant="activeDomain === domain.value ? 'tonal' : 'outlined'"
                                       @click="selectDomain(domain.value)">
                                    <span>{{ domain.label }}</span>
                                </v-btn>
                            </div>

                            <v-divider class="my-3" />

                            <div class="rule-center-nav-group">
                                <v-btn v-for="tab in secondaryTabs"
                                       :key="tab.value"
                                       block
                                       border
                                       class="rule-center-nav-button text-none"
                                       :color="activeTab === tab.value ? 'primary' : 'default'"
                                       :variant="activeTab === tab.value ? 'tonal' : 'outlined'"
                                       @click="selectTab(tab.value)">
                                    <span>{{ tab.label }}</span>
                                </v-btn>
                            </div>
                        </div>
                    </v-navigation-drawer>

                    <v-main>
                        <v-card variant="flat" min-height="760">
                            <template #title>
                                <div class="title-and-toolbar d-flex flex-wrap align-center ga-3">
                                    <v-btn class="me-1 d-md-none"
                                           density="compact"
                                           color="default"
                                           variant="plain"
                                           :ripple="false"
                                           :icon="true"
                                           @click="showNav = !showNav">
                                        <v-icon :icon="mdiMenu" size="24" />
                                    </v-btn>

                                    <div class="min-w-0">
                                        <div class="text-h6">{{ currentDomainOption.label }}</div>
                                        <div class="text-body-2 text-medium-emphasis">{{ currentTabOption.label }}</div>
                                    </div>

                                    <v-spacer />

                                    <v-chip v-if="isPairingOverview" color="primary" variant="tonal">
                                        {{ filteredPairs.length }} {{ tt('pairs') }}
                                    </v-chip>

                                    <v-btn v-if="isPairingOverview"
                                           color="default"
                                           variant="text"
                                           density="compact"
                                           size="36"
                                           :icon="true"
                                           :loading="loading"
                                           @click="refreshActiveView">
                                        <v-icon :icon="mdiRefresh" size="22" />
                                        <v-tooltip activator="parent">{{ tt('Refresh') }}</v-tooltip>
                                    </v-btn>
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

                                <template v-else-if="activeDomain === 'transfer' && activeTab === 'rules'">
                                    <rule-center-panel init-tab="rules"
                                                       :tabs="['rules', 'recurring']"
                                                       :title="tt('Pairing Rules')" />
                                </template>

                                <template v-else-if="activeDomain === 'investment' && activeTab === 'rules'">
                                    <investment-recognition-settings-card />
                                </template>

                                <template v-else-if="activeDomain === 'learning' && activeTab === 'overview'">
                                    <learning-center-panel key="learning-overview" init-tab="suggestions" />
                                </template>

                                <template v-else-if="activeDomain === 'learning' && activeTab === 'rules'">
                                    <learning-center-panel key="learning-rules" init-tab="rules" />
                                </template>

                                <template v-else-if="activeDomain === 'llm' && activeTab === 'overview'">
                                    <learning-center-panel key="llm-overview" init-tab="llm" />
                                </template>

                                <template v-else-if="activeDomain === 'llm' && activeTab === 'config'">
                                    <learning-center-panel key="llm-config" init-tab="llm-config" />
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
    type RuleCenterDomain,
    type RuleCenterTab,
} from '@/views/desktop/pairingcenter/rule_center_navigation.ts';

type DomainOption = {
    value: RuleCenterDomain;
    label: string;
};

type SecondaryTabOption = {
    value: RuleCenterTab;
    label: string;
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
const alwaysShowNav = ref<boolean>(display.mdAndUp.value);
const showNav = ref<boolean>(display.mdAndUp.value);
const showDeleteDialog = ref(false);
const pairToDelete = ref<BillMatchingPairDetail | null>(null);
const deleting = ref<number | null>(null);

const loading = computed(() => matchingStore.loading);
const error = computed({
    get: () => matchingStore.error,
    set: (val) => { matchingStore.error = val; }
});

const domainOptions = computed<DomainOption[]>(() => [
    {
        value: 'transfer',
        label: tt('Transfer Pairing'),
    },
    {
        value: 'investment',
        label: tt('Investment Pairing'),
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

const secondaryTabs = computed<SecondaryTabOption[]>(() => {
    if (activeDomain.value === 'transfer') {
        return [
            {
                value: 'overview',
                label: tt('Pairing Overview'),
            },
            {
                value: 'rules',
                label: tt('Pairing Rules'),
            },
        ];
    }

    if (activeDomain.value === 'investment') {
        return [
            {
                value: 'overview',
                label: tt('Pairing Overview'),
            },
            {
                value: 'rules',
                label: tt('Pairing Rules'),
            },
        ];
    }

    if (activeDomain.value === 'learning') {
        return [
            {
                value: 'overview',
                label: tt('Suggestions'),
            },
            {
                value: 'rules',
                label: tt('Learning Rules'),
            },
        ];
    }

    return [
        {
            value: 'overview',
            label: tt('Candidate Rules'),
        },
        {
            value: 'config',
            label: tt('LLM Config'),
        },
    ];
});

const currentDomainOption = computed<DomainOption>(() => {
    return domainOptions.value.find(option => option.value === activeDomain.value) ?? domainOptions.value[0]!;
});

const currentTabOption = computed<SecondaryTabOption>(() => {
    return secondaryTabs.value.find(option => option.value === activeTab.value) ?? secondaryTabs.value[0]!;
});

const isPairingOverview = computed(() =>
    (activeDomain.value === 'transfer' || activeDomain.value === 'investment') && activeTab.value === 'overview'
);
const filteredPairs = computed(() => matchingStore.pairs.filter(pair => pair.pairType === activeDomain.value));
const showPairsLoading = computed(() => isPairingOverview.value && loading.value);
const showPairsError = computed(() => isPairingOverview.value && !!error.value);

function collapseNavOnMobile(): void {
    if (!alwaysShowNav.value) {
        showNav.value = false;
    }
}

function syncQuery(domain: RuleCenterDomain, tab: RuleCenterTab): void {
    void router.replace({
        path: '/pairing/list',
        query: buildRuleCenterQuery(route.query, domain, tab),
    });
}

function selectDomain(domain: RuleCenterDomain): void {
    const nextTab = normalizeRuleCenterTab(domain, activeTab.value);
    const unchanged = activeDomain.value === domain && activeTab.value === nextTab;

    activeDomain.value = domain;
    activeTab.value = nextTab;
    collapseNavOnMobile();

    if (unchanged) {
        return;
    }

    syncQuery(activeDomain.value, activeTab.value);
    void refreshActiveView();
}

function selectTab(tabValue: RuleCenterTab): void {
    const nextTab = normalizeRuleCenterTab(activeDomain.value, tabValue);
    const unchanged = activeTab.value === nextTab;

    activeTab.value = nextTab;
    collapseNavOnMobile();

    if (unchanged) {
        return;
    }

    syncQuery(activeDomain.value, activeTab.value);
    void refreshActiveView();
}

async function refreshActiveView(): Promise<void> {
    if (isPairingOverview.value) {
        await matchingStore.loadPairs();
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

        if (selection.shouldRewriteQuery) {
            syncQuery(selection.domain, selection.tab);
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

.rule-center-nav-group {
    display: flex;
    flex-direction: column;
}

.rule-center-nav-stack {
    max-width: 220px;
    margin-inline: auto;
}

.rule-center-nav-group .v-btn:not(:first-child) {
    border-top-left-radius: inherit;
    border-top-right-radius: inherit;
}

.rule-center-nav-group .v-btn:not(:last-child) {
    border-bottom: 0;
    border-bottom-left-radius: inherit;
    border-bottom-right-radius: inherit;
}

.rule-center-nav-button {
    justify-content: center;
    min-height: 40px;
    height: 40px;
    font-size: 0.875rem;
    font-weight: 500;
}

.rule-center-nav-button :deep(.v-btn__content) {
    width: 100%;
    justify-content: center;
}
</style>
