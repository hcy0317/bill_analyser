<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-layout>
                    <!-- 左侧筛选抽屉 -->
                    <v-navigation-drawer :permanent="alwaysShowNav" v-model="showNav">
                        <div class="mx-6 mt-4">
                            <v-btn block variant="tonal" color="primary"
                                   :disabled="loading"
                                   @click="loadAllPairs">
                                <v-icon start :icon="mdiRefresh" />
                                {{ tt('Refresh') }}
                            </v-btn>
                        </div>
                        <v-divider class="mt-4" />
                        <v-list density="compact" nav class="mt-2">
                            <v-list-item
                                :active="activePairType === undefined"
                                :prepend-icon="mdiFormatListBulleted"
                                class="mb-1"
                                @click="filterByType(undefined)">
                                <v-list-item-title>{{ tt('All Types') }}</v-list-item-title>
                            </v-list-item>
                            <v-list-item
                                v-for="pairType in pairTypeOptions"
                                :key="pairType.value"
                                :active="activePairType === pairType.value"
                                :prepend-icon="pairTypeIcon(pairType.value)"
                                class="mb-1"
                                @click="filterByType(pairType.value)">
                                <v-list-item-title>{{ pairType.label }}</v-list-item-title>
                            </v-list-item>
                        </v-list>
                        <v-divider class="mt-2" />
                        <div class="mx-6 mt-4">
                            <v-btn block variant="outlined" color="green" @click="goToInvestmentSettings">
                                <v-icon start :icon="mdiCog" />
                                {{ tt('Investment Settings') }}
                            </v-btn>
                        </div>
                    </v-navigation-drawer>

                    <!-- 主内容区 -->
                    <v-main>
                        <v-card-text>
                            <div class="d-flex align-center mb-4">
                                <v-icon :icon="mdiLinkVariant" class="mr-2" />
                                <span class="text-h6">{{ tt('Pairing Center') }}</span>
                                <v-spacer />
                                <v-chip color="primary" variant="tonal" class="ml-2">
                                    {{ pairCount }} {{ tt('pairs') }}
                                </v-chip>
                            </div>

                            <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-4" />

                            <v-alert v-if="error" type="error" closable class="mb-4"
                                     @click:close="error = null">
                                {{ error }}
                            </v-alert>

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
                        </v-card-text>
                    </v-main>
                </v-layout>
            </v-card>
        </v-col>
    </v-row>

    <!-- 删除确认对话框 -->
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
import { ref, computed, onMounted } from 'vue';
import { useDisplay } from 'vuetify';
import { useRouter } from 'vue-router';

import {
    mdiLinkVariant,
    mdiRefresh,
    mdiDeleteOutline,
    mdiFormatListBulleted,
    mdiSwapHorizontal,
    mdiFinance,
    mdiBrain,
    mdiLinkOff,
    mdiCog
} from '@mdi/js';

import type { BillMatchingPairDetail } from '@/models/bill_matching.ts';
import { useMatchingStore } from '@/stores/matching.ts';
import { useI18n } from '@/locales/helpers.ts';

const props = defineProps<{
    initPairType?: string
}>();

const router = useRouter();
const display = useDisplay();
const { tt } = useI18n();
const matchingStore = useMatchingStore();

const showNav = ref(true);
const alwaysShowNav = computed(() => display.lgAndUp.value);
const activePairType = ref<string | undefined>(props.initPairType || undefined);
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

function filterByType(pairType?: string): void {
    activePairType.value = pairType;
    loadAllPairs();
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

function goToInvestmentSettings() {
    router.push('/user/settings?tab=dataManagementSetting');
}

onMounted(() => {
    loadAllPairs();
});
</script>

<style scoped>
.v-table :deep(td) {
    padding-block: 12px;
    vertical-align: middle;
}

.v-table :deep(th) {
    white-space: nowrap;
}
</style>
