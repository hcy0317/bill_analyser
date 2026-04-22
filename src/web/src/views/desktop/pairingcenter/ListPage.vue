<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-layout>
                    <v-navigation-drawer :permanent="alwaysShowNav" v-model="showNav">
                        <div class="mx-6 mt-4" v-if="activeSection === 'pairs'">
                            <v-btn block variant="tonal" color="primary"
                                   :disabled="loading"
                                   @click="loadAllPairs">
                                <v-icon start :icon="mdiRefresh" />
                                {{ tt('Refresh') }}
                            </v-btn>
                        </div>
                        <v-divider class="mt-4" v-if="activeSection === 'pairs'" />
                        <v-list density="compact" nav class="mt-2" v-if="activeSection === 'pairs'">
                            <v-list-item
                                :active="activeSection === 'pairs' && activePairType === undefined"
                                :prepend-icon="mdiFormatListBulleted"
                                class="mb-1"
                                @click="filterByType(undefined)">
                                <v-list-item-title>{{ tt('All Types') }}</v-list-item-title>
                            </v-list-item>
                            <v-list-item
                                v-for="pairType in pairTypeOptions"
                                :key="pairType.value"
                                :active="activeSection === 'pairs' && activePairType === pairType.value"
                                :prepend-icon="pairTypeIcon(pairType.value)"
                                class="mb-1"
                                @click="filterByType(pairType.value)">
                                <v-list-item-title>{{ pairType.label }}</v-list-item-title>
                            </v-list-item>
                        </v-list>
                        <v-divider class="mt-2" />
                        <v-list density="compact" nav class="mt-2">
                            <v-list-item
                                :active="activeSection === 'pairs'"
                                :prepend-icon="mdiFormatListBulleted"
                                class="mb-1"
                                @click="openPairsSection()"
                            >
                                <v-list-item-title>{{ tt('Pairs') }}</v-list-item-title>
                            </v-list-item>
                            <v-list-item
                                :active="activeSection === 'learning-center'"
                                :prepend-icon="mdiBrain"
                                class="mb-1"
                                @click="openLearningCenter('rules')"
                            >
                                <v-list-item-title>{{ tt('Learning Rules') }}</v-list-item-title>
                            </v-list-item>
                            <v-list-item
                                :active="activeSection === 'rule-center'"
                                :prepend-icon="mdiBookCogOutline"
                                class="mb-1"
                                @click="openRuleCenter('rules')"
                            >
                                <v-list-item-title>{{ tt('Category & Recurring Rules') }}</v-list-item-title>
                            </v-list-item>
                            <v-list-item
                                :active="activeSection === 'investment-settings'"
                                :prepend-icon="mdiCog"
                                class="mb-1"
                                @click="openInvestmentSettings"
                            >
                                <v-list-item-title>{{ tt('Investment Settings') }}</v-list-item-title>
                            </v-list-item>
                        </v-list>
                    </v-navigation-drawer>

                    <v-main>
                        <v-card-text>
                            <v-sheet border rounded="lg" class="pairing-shell pa-4 mb-4">
                                <div class="d-flex flex-column flex-lg-row align-lg-center ga-4">
                                    <div class="min-w-0">
                                        <div class="d-flex align-center ga-2">
                                            <v-icon :icon="mdiLinkVariant" />
                                            <span class="text-h6">{{ tt('Pairing Center') }}</span>
                                        </div>
                                        <div class="text-body-2 text-medium-emphasis mt-2">
                                            {{ tt('Pairing Center is now the only canonical persistent home for pairs, learning rules, category and recurring rules, and investment recognition settings. Legacy Learning Center and Rule Center routes only forward here.') }}
                                        </div>
                                    </div>
                                    <v-spacer />
                                    <div class="d-flex flex-wrap ga-2">
                                        <v-btn
                                            color="primary"
                                            :variant="activeSection === 'pairs' ? 'flat' : 'tonal'"
                                            @click="openPairsSection()">
                                            <v-icon start :icon="mdiFormatListBulleted" />
                                            {{ tt('Pairs') }}
                                        </v-btn>
                                        <v-btn
                                            color="secondary"
                                            :variant="activeSection === 'learning-center' ? 'flat' : 'tonal'"
                                            @click="openLearningCenter('rules')">
                                            <v-icon start :icon="mdiBrain" />
                                            {{ tt('Learning Rules') }}
                                        </v-btn>
                                        <v-btn
                                            color="secondary"
                                            :variant="activeSection === 'rule-center' ? 'flat' : 'tonal'"
                                            @click="openRuleCenter('rules')">
                                            <v-icon start :icon="mdiBookCogOutline" />
                                            {{ tt('Category Rules') }}
                                        </v-btn>
                                        <v-btn
                                            color="secondary"
                                            :variant="activeSection === 'investment-settings' ? 'flat' : 'tonal'"
                                            @click="openInvestmentSettings">
                                            <v-icon start :icon="mdiCog" />
                                            {{ tt('Investment Settings') }}
                                        </v-btn>
                                    </div>
                                </div>
                            </v-sheet>

                            <template v-if="activeSection === 'pairs' || activeSection === 'investment-settings'">
                                <div class="d-flex align-center mb-4">
                                    <v-icon :icon="activeSectionIcon" class="mr-2" />
                                    <span class="text-h6">{{ activeSectionTitle }}</span>
                                    <v-spacer />
                                    <v-chip v-if="activeSection === 'pairs'" color="primary" variant="tonal" class="ml-2">
                                        {{ pairCount }} {{ tt('pairs') }}
                                    </v-chip>
                                </div>
                            </template>

                            <v-progress-linear v-if="activeSection === 'pairs' && loading" indeterminate color="primary" class="mb-4" />

                            <v-alert v-if="activeSection === 'pairs' && error" type="error" closable class="mb-4"
                                     @click:close="error = null">
                                {{ error }}
                            </v-alert>

                            <template v-if="activeSection === 'pairs'">
                                <v-table v-if="!loading && pairs.length > 0" hover density="comfortable">
                                    <thead>
                                        <tr>
                                            <th>{{ tt('Type') }}</th>
                                            <th>{{ tt('Source') }}</th>
                                            <th>{{ tt('Left Bill') }}</th>
                                            <th>{{ tt('Right Bill') }}</th>
                                            <th>{{ tt('Created At') }}</th>
                                            <th class="text-center">{{ tt('Actions') }}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        <tr v-for="pair in pairs" :key="pair.id">
                                            <td>
                                                <v-chip size="small" :color="pairTypeColor(pair.pairType)">
                                                    {{ pairTypeLabel(pair.pairType) }}
                                                </v-chip>
                                            </td>
                                            <td>{{ pair.source }}</td>
                                            <td>
                                                <template v-if="pair.leftBill">
                                                    <div class="text-body-2">{{ pair.leftBill.description }}</div>
                                                    <div class="text-caption text-grey">
                                                        {{ formatAmount(pair.leftBill.amount) }} · {{ pair.leftBill.date }}
                                                    </div>
                                                </template>
                                                <span v-else class="text-grey">ID: {{ pair.leftBillId }}</span>
                                            </td>
                                            <td>
                                                <template v-if="pair.rightBill">
                                                    <div class="text-body-2">{{ pair.rightBill.description }}</div>
                                                    <div class="text-caption text-grey">
                                                        {{ formatAmount(pair.rightBill.amount) }} · {{ pair.rightBill.date }}
                                                    </div>
                                                </template>
                                                <span v-else class="text-grey">ID: {{ pair.rightBillId }}</span>
                                            </td>
                                            <td class="text-caption">{{ pair.createdAt }}</td>
                                            <td class="text-center">
                                                <v-btn icon size="small" variant="text" color="error"
                                                       :disabled="deleting === pair.id"
                                                       @click="confirmDeletePair(pair)">
                                                    <v-icon :icon="mdiDeleteOutline" />
                                                    <v-tooltip activator="parent" location="top">{{ tt('Delete') }}</v-tooltip>
                                                </v-btn>
                                            </td>
                                        </tr>
                                    </tbody>
                                </v-table>

                                <v-empty-state v-if="!loading && pairs.length === 0"
                                               :icon="mdiLinkOff"
                                               :headline="tt('No Pairs')"
                                               :text="tt('No matching pairs found. Pairs are created when transactions are linked across accounts.')" />
                            </template>

                            <template v-else-if="activeSection === 'investment-settings'">
                                <v-alert type="info" variant="tonal" class="mb-4">
                                    {{ tt('Investment recognition settings now belong to Pairing Center and are no longer managed from User Settings, Learning Center, or Rule Center.') }}
                                </v-alert>
                                <investment-recognition-settings-card />
                            </template>

                            <template v-else-if="activeSection === 'learning-center'">
                                <learning-center-panel :init-tab="activeLearningTab" />
                            </template>

                            <template v-else>
                                <rule-center-panel :init-tab="activeRuleTab" />
                            </template>
                        </v-card-text>
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
import { ref, computed, watch } from 'vue';
import { useDisplay } from 'vuetify';
import { useRoute, useRouter } from 'vue-router';

import {
    mdiLinkVariant,
    mdiRefresh,
    mdiDeleteOutline,
    mdiFormatListBulleted,
    mdiSwapHorizontal,
    mdiFinance,
    mdiBrain,
    mdiLinkOff,
    mdiCog,
    mdiBookCogOutline
} from '@mdi/js';

import type { BillMatchingPairDetail } from '@/models/bill_matching.ts';
import { useMatchingStore } from '@/stores/matching.ts';
import { useI18n } from '@/locales/helpers.ts';
import InvestmentRecognitionSettingsCard from '@/views/desktop/pairingcenter/components/InvestmentRecognitionSettingsCard.vue';
import LearningCenterPanel from '@/views/desktop/pairingcenter/components/LearningCenterPanel.vue';
import RuleCenterPanel from '@/views/desktop/pairingcenter/components/RuleCenterPanel.vue';

type PairingCenterSection = 'pairs' | 'learning-center' | 'rule-center' | 'investment-settings';

type LearningCenterTab = 'suggestions' | 'rules' | 'llm';
type RuleCenterTab = 'rules' | 'learning' | 'investment' | 'recurring';

const props = defineProps<{
    initPairType?: string;
    initView?: string;
    initTab?: string;
}>();

const route = useRoute();
const router = useRouter();
const display = useDisplay();
const { tt } = useI18n();
const matchingStore = useMatchingStore();

const showNav = ref(true);
const alwaysShowNav = computed(() => display.lgAndUp.value);
const activeSection = ref<PairingCenterSection>('pairs');
const activePairType = ref<string | undefined>(undefined);
const activeLearningTab = ref<LearningCenterTab>('suggestions');
const activeRuleTab = ref<RuleCenterTab>('rules');
const showDeleteDialog = ref(false);
const pairToDelete = ref<BillMatchingPairDetail | null>(null);
const deleting = ref<number | null>(null);

const pairs = computed(() => {
    if (!activePairType.value) return matchingStore.pairs;
    return matchingStore.pairs.filter(p => p.pairType === activePairType.value);
});
const pairCount = computed(() => pairs.value.length);
const loading = computed(() => matchingStore.loading);
const error = computed({
    get: () => matchingStore.error,
    set: (val) => { matchingStore.error = val; }
});
const activeSectionTitle = computed(() => {
    if (activeSection.value === 'investment-settings') {
        return tt('Investment Recognition Settings');
    }

    return tt('Pairs');
});
const activeSectionIcon = computed(() => {
    return activeSection.value === 'investment-settings'
        ? mdiCog
        : mdiFormatListBulleted;
});

const pairTypeOptions = computed(() => [
    { value: 'transfer', label: tt('Transfer') },
    { value: 'investment', label: tt('Investment') },
    { value: 'learning', label: tt('Learning') }
]);

function pairTypeColor(pairType: string): string {
    switch (pairType) {
        case 'transfer': return 'blue';
        case 'investment': return 'green';
        case 'learning': return 'orange';
        default: return 'grey';
    }
}

function pairTypeLabel(pairType: string): string {
    switch (pairType) {
        case 'transfer': return tt('Transfer');
        case 'investment': return tt('Investment');
        case 'learning': return tt('Learning');
        default: return pairType;
    }
}

function pairTypeIcon(pairType: string): string {
    switch (pairType) {
        case 'transfer': return mdiSwapHorizontal;
        case 'investment': return mdiFinance;
        case 'learning': return mdiBrain;
        default: return mdiLinkVariant;
    }
}

function formatAmount(amount: number): string {
    return amount.toFixed(2);
}

async function loadAllPairs(): Promise<void> {
    await matchingStore.loadPairs();
}

function normalizeSection(view?: string): PairingCenterSection {
    if (view === 'learning' || view === 'learning-center') {
        return 'learning-center';
    }

    if (view === 'rules' || view === 'rule-center') {
        return 'rule-center';
    }

    return view === 'investment-settings' ? 'investment-settings' : 'pairs';
}

function normalizePairType(pairType?: string): string | undefined {
    return typeof pairType === 'string' && pairType ? pairType : undefined;
}

function normalizeLearningTab(tab?: string): LearningCenterTab {
    if (tab === 'rules' || tab === 'llm') {
        return tab;
    }

    return 'suggestions';
}

function normalizeRuleTab(tab?: string): RuleCenterTab {
    if (tab === 'learning' || tab === 'investment' || tab === 'recurring') {
        return tab;
    }

    return 'rules';
}

function syncQuery(nextSection: PairingCenterSection, nextPairType?: string, nextTab?: string): void {
    const nextQuery: Record<string, string> = {};

    for (const [key, value] of Object.entries(route.query)) {
        if (typeof value === 'string' && key !== 'view' && key !== 'pairType' && key !== 'tab') {
            nextQuery[key] = value;
        }
    }

    if (nextSection === 'investment-settings') {
        nextQuery['view'] = 'investment-settings';
    } else if (nextSection === 'learning-center') {
        nextQuery['view'] = 'learning';
        if (nextTab) {
            nextQuery['tab'] = nextTab;
        }
    } else if (nextSection === 'rule-center') {
        nextQuery['view'] = 'rules';
        if (nextTab) {
            nextQuery['tab'] = nextTab;
        }
    } else if (nextPairType) {
        nextQuery['pairType'] = nextPairType;
    }

    void router.replace({ path: '/pairing/list', query: nextQuery });
}

function filterByType(pairType?: string): void {
    activeSection.value = 'pairs';
    activePairType.value = pairType;
    syncQuery('pairs', pairType);
    void loadAllPairs();
}

function openPairsSection(): void {
    activeSection.value = 'pairs';
    syncQuery('pairs', activePairType.value);

    if (!matchingStore.loading && matchingStore.pairs.length === 0) {
        void loadAllPairs();
    }
}

function openLearningCenter(tab?: string): void {
    activeSection.value = 'learning-center';
    activeLearningTab.value = normalizeLearningTab(tab);
    syncQuery('learning-center', undefined, activeLearningTab.value);
}

function openRuleCenter(tab?: string): void {
    activeSection.value = 'rule-center';
    activeRuleTab.value = normalizeRuleTab(tab);
    syncQuery('rule-center', undefined, activeRuleTab.value);
}

function openInvestmentSettings(): void {
    activeSection.value = 'investment-settings';
    syncQuery('investment-settings');
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
    () => [props.initView, props.initPairType, props.initTab] as const,
    ([initView, initPairType, initTab]) => {
        const nextSection = normalizeSection(initView);
        activeSection.value = nextSection;
        activePairType.value = normalizePairType(initPairType);

        if (nextSection === 'learning-center') {
            activeLearningTab.value = normalizeLearningTab(initTab);
        } else if (nextSection === 'rule-center') {
            activeRuleTab.value = normalizeRuleTab(initTab);
        }
    },
    { immediate: true }
);

watch(
    activeSection,
    (section) => {
        if (section === 'pairs' && !matchingStore.loading && matchingStore.pairs.length === 0) {
            void loadAllPairs();
        }
    },
    { immediate: true }
);
</script>

<style scoped>
.v-table :deep(td) {
    padding-block: 12px;
    vertical-align: middle;
}

.v-table :deep(th) {
    white-space: nowrap;
}

.pairing-shell {
    background: rgba(var(--v-theme-surface), 1);
}
</style>
