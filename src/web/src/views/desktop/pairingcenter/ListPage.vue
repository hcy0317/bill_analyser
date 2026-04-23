<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-card-text>
                    <v-sheet border rounded="lg" class="rule-center-shell pa-4 mb-4">
                        <div class="d-flex flex-column flex-lg-row align-lg-center ga-4">
                            <div class="min-w-0">
                                <div class="d-flex align-center ga-2">
                                    <v-icon :icon="mdiBookCogOutline" />
                                    <span class="text-h6">{{ tt('Rule Center') }}</span>
                                </div>
                                <div class="text-body-2 text-medium-emphasis mt-2">
                                    {{ tt('Rule Center is the canonical home for persistent matching, learning, and recognition governance.') }}
                                </div>
                            </div>
                            <v-spacer />
                            <v-btn variant="tonal" color="primary" :disabled="loading" @click="refreshActiveView">
                                <v-icon start :icon="mdiRefresh" />
                                {{ tt('Refresh') }}
                            </v-btn>
                        </div>
                    </v-sheet>

                    <div class="d-flex flex-wrap align-center ga-3 mb-4">
                        <v-btn-toggle
                            v-model="activeDomain"
                            mandatory
                            divided
                            color="primary"
                            class="rule-center-domains"
                        >
                            <v-btn v-for="domain in domainOptions"
                                   :key="domain.value"
                                   :value="domain.value"
                                   @click="selectDomain(domain.value)">
                                <v-icon start :icon="domain.icon" />
                                {{ domain.label }}
                            </v-btn>
                        </v-btn-toggle>
                    </div>

                    <v-tabs v-model="activeTab" class="mb-4" @update:model-value="selectTab">
                        <v-tab v-for="tab in secondaryTabs"
                               :key="tab.value"
                               :value="tab.value">
                            <v-icon start :icon="tab.icon" />
                            {{ tab.label }}
                        </v-tab>
                    </v-tabs>

                    <v-progress-linear v-if="showPairsLoading" indeterminate color="primary" class="mb-4" />

                    <v-alert v-if="showPairsError" type="error" closable class="mb-4"
                             @click:close="error = null">
                        {{ error }}
                    </v-alert>

                    <template v-if="isPairingOverview">
                        <div class="d-flex align-center mb-4">
                            <v-icon :icon="activeDomain === 'transfer' ? mdiSwapHorizontal : mdiFinance" class="mr-2" />
                            <span class="text-h6">{{ activeDomainTitle }} · {{ tt('Pairing Overview') }}</span>
                            <v-spacer />
                            <v-chip color="primary" variant="tonal" class="ml-2">
                                {{ filteredPairs.length }} {{ tt('pairs') }}
                            </v-chip>
                        </div>

                        <pairs-overview-table
                            v-if="!loading"
                            :pairs="filteredPairs"
                            :deleting-pair-id="deleting"
                            :empty-headline="tt(activeDomain === 'transfer' ? 'No Transfer Pairs' : 'No Investment Pairs')"
                            :empty-text="tt(activeDomain === 'transfer' ? 'No transfer pairs found. Transfer pairs are created when opposite account movements are linked.' : 'No investment pairs found. Investment pairs are created when investment-related movements are linked.')"
                            @delete="confirmDeletePair"
                        />
                    </template>

                    <template v-else-if="activeDomain === 'transfer' && activeTab === 'rules'">
                        <v-card variant="outlined">
                            <v-card-title class="d-flex align-center">
                                <v-icon start :icon="mdiSwapHorizontal" />
                                {{ tt('Transfer Pairing Rules') }}
                            </v-card-title>
                            <v-card-text>
                                <v-empty-state
                                    :icon="mdiSwapHorizontal"
                                    :headline="tt('No Editable Transfer Pairing Rules')"
                                    :text="tt('Transfer pair matching is currently governed by backend matching constraints. Review transfer candidates from transaction details and import preview.')"
                                />
                            </v-card-text>
                        </v-card>
                    </template>

                    <template v-else-if="activeDomain === 'investment' && activeTab === 'rules'">
                        <investment-recognition-settings-card />
                    </template>

                    <template v-else-if="activeDomain === 'learning' && activeTab === 'overview'">
                        <learning-center-panel key="learning-overview" init-tab="suggestions" />
                    </template>

                    <template v-else-if="activeDomain === 'learning' && activeTab === 'rules'">
                        <learning-center-panel key="learning-rules" init-tab="rules" />
                        <v-expansion-panels class="mt-4" variant="accordion">
                            <v-expansion-panel>
                                <v-expansion-panel-title>
                                    <div class="d-flex align-center ga-2">
                                        <v-icon :icon="mdiBookCogOutline" size="small" />
                                        <span>{{ tt('Category and Recurring Rules') }}</span>
                                    </div>
                                </v-expansion-panel-title>
                                <v-expansion-panel-text>
                                    <rule-center-panel init-tab="rules" :tabs="['rules', 'recurring']" :title="tt('Category and Recurring Rules')" />
                                </v-expansion-panel-text>
                            </v-expansion-panel>
                        </v-expansion-panels>
                    </template>

                    <template v-else-if="activeDomain === 'llm' && activeTab === 'overview'">
                        <learning-center-panel key="llm-overview" init-tab="llm" />
                    </template>

                    <template v-else-if="activeDomain === 'llm' && activeTab === 'config'">
                        <learning-center-panel key="llm-config" init-tab="llm-config" />
                    </template>
                </v-card-text>
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
import { ref, computed, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';

import {
    mdiBookCogOutline,
    mdiRefresh,
    mdiSwapHorizontal,
    mdiFinance,
    mdiBrain,
    mdiRobotOutline,
    mdiFormatListBulleted,
    mdiCog,
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
    icon: string;
};

type SecondaryTabOption = {
    value: RuleCenterTab;
    label: string;
    icon: string;
};

const props = defineProps<{
    initDomain?: unknown;
    initPairType?: unknown;
    initView?: unknown;
    initTab?: unknown;
}>();

const route = useRoute();
const router = useRouter();
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
const showDeleteDialog = ref(false);
const pairToDelete = ref<BillMatchingPairDetail | null>(null);
const deleting = ref<number | null>(null);

const loading = computed(() => matchingStore.loading);
const error = computed({
    get: () => matchingStore.error,
    set: (val) => { matchingStore.error = val; }
});

const domainOptions = computed<DomainOption[]>(() => [
    { value: 'transfer', label: tt('Transfer Pairing'), icon: mdiSwapHorizontal },
    { value: 'investment', label: tt('Investment Pairing'), icon: mdiFinance },
    { value: 'learning', label: tt('Long-term Learning'), icon: mdiBrain },
    { value: 'llm', label: tt('LLM Recognition'), icon: mdiRobotOutline },
]);

const secondaryTabs = computed<SecondaryTabOption[]>(() => {
    if (activeDomain.value === 'learning') {
        return [
            { value: 'overview', label: tt('Learning Overview'), icon: mdiFormatListBulleted },
            { value: 'rules', label: tt('Learning Rules'), icon: mdiBookCogOutline },
        ];
    }

    if (activeDomain.value === 'llm') {
        return [
            { value: 'overview', label: tt('Induction Overview'), icon: mdiFormatListBulleted },
            { value: 'config', label: tt('LLM Config'), icon: mdiCog },
        ];
    }

    return [
        { value: 'overview', label: tt('Pairing Overview'), icon: mdiFormatListBulleted },
        { value: 'rules', label: tt('Pairing Rules'), icon: mdiBookCogOutline },
    ];
});

const activeDomainTitle = computed(() => {
    return domainOptions.value.find(option => option.value === activeDomain.value)?.label ?? tt('Rule Center');
});

const isPairingOverview = computed(() =>
    (activeDomain.value === 'transfer' || activeDomain.value === 'investment') && activeTab.value === 'overview'
);

const filteredPairs = computed(() => matchingStore.pairs.filter(pair => pair.pairType === activeDomain.value));
const showPairsLoading = computed(() => isPairingOverview.value && loading.value);
const showPairsError = computed(() => isPairingOverview.value && !!error.value);

function syncQuery(domain: RuleCenterDomain, tab: RuleCenterTab): void {
    void router.replace({
        path: '/pairing/list',
        query: buildRuleCenterQuery(route.query, domain, tab),
    });
}

function selectDomain(domain: RuleCenterDomain): void {
    activeDomain.value = domain;
    activeTab.value = normalizeRuleCenterTab(domain, activeTab.value);
    syncQuery(activeDomain.value, activeTab.value);
    void refreshActiveView();
}

function selectTab(tabValue: unknown): void {
    if (typeof tabValue !== 'string') {
        return;
    }

    activeTab.value = normalizeRuleCenterTab(activeDomain.value, tabValue);
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
    if (!pairToDelete.value) return;
    deleting.value = pairToDelete.value.id;
    await matchingStore.deletePair(pairToDelete.value.id);
    deleting.value = null;
    showDeleteDialog.value = false;
    pairToDelete.value = null;
}

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
.rule-center-shell {
    background: rgba(var(--v-theme-surface), 1);
}

.rule-center-domains {
    flex-wrap: wrap;
    height: auto;
}
</style>
