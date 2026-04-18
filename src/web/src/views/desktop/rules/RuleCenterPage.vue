<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-card-text>
                    <div class="d-flex align-center mb-4">
                        <v-icon :icon="mdiShieldCheck" class="mr-2" />
                        <span class="text-h6">{{ tt('Rule Center') }}</span>
                        <v-chip class="ml-2" size="small" variant="tonal">
                            {{ overview.totalRuleCount }} {{ tt('rules total') }}
                        </v-chip>
                        <v-spacer />
                        <v-btn variant="outlined" :disabled="loading" @click="fetchOverview">
                            <v-icon start :icon="mdiRefresh" />
                            {{ tt('Refresh') }}
                        </v-btn>
                    </div>

                    <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-4" />

                    <v-alert v-if="error" type="error" closable class="mb-4" @click:close="error = null">
                        {{ error }}
                    </v-alert>

                    <v-tabs v-model="activeTab" class="mb-4">
                        <v-tab value="learning">
                            <v-icon start :icon="mdiBrain" />
                            {{ tt('Learning Rules') }} ({{ overview.learningRuleCount }})
                        </v-tab>
                        <v-tab value="keywords">
                            <v-icon start :icon="mdiTagMultiple" />
                            {{ tt('Category Keywords') }} ({{ overview.categoryKeywordCount }})
                        </v-tab>
                        <v-tab value="recurring">
                            <v-icon start :icon="mdiCalendarSync" />
                            {{ tt('Recurring Rules') }} ({{ overview.recurringRuleCount }})
                        </v-tab>
                    </v-tabs>

                    <v-tabs-window v-model="activeTab">
                        <!-- Learning Rules -->
                        <v-tabs-window-item value="learning">
                            <v-data-table
                                :headers="learningHeaders"
                                :items="overview.learningRules"
                                :items-per-page="20"
                                density="compact">
                                <template #item.enabled="{ item }">
                                    <v-icon :icon="item.enabled ? mdiCheckCircle : mdiCloseCircle"
                                            :color="item.enabled ? 'success' : 'grey'" size="small" />
                                </template>
                            </v-data-table>
                        </v-tabs-window-item>

                        <!-- Category Keywords -->
                        <v-tabs-window-item value="keywords">
                            <v-data-table
                                :headers="keywordHeaders"
                                :items="overview.categoryKeywords"
                                :items-per-page="20"
                                density="compact" />
                        </v-tabs-window-item>

                        <!-- Recurring Rules -->
                        <v-tabs-window-item value="recurring">
                            <v-data-table
                                :headers="recurringHeaders"
                                :items="overview.recurringRules"
                                :items-per-page="20"
                                density="compact">
                                <template #item.amount="{ item }">
                                    ¥{{ Math.abs(item.amount || 0).toFixed(2) }}
                                </template>
                                <template #item.enabled="{ item }">
                                    <v-icon :icon="item.enabled ? mdiCheckCircle : mdiCloseCircle"
                                            :color="item.enabled ? 'success' : 'grey'" size="small" />
                                </template>
                            </v-data-table>
                        </v-tabs-window-item>
                    </v-tabs-window>
                </v-card-text>
            </v-card>
        </v-col>
    </v-row>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import {
    mdiShieldCheck, mdiRefresh, mdiBrain, mdiTagMultiple,
    mdiCalendarSync, mdiCheckCircle, mdiCloseCircle
} from '@mdi/js';
import services from '@/lib/services.ts';

const tt = (key: string) => key;

const loading = ref(false);
const error = ref<string | null>(null);
const activeTab = ref('learning');

interface Overview {
    learningRules: any[];
    learningRuleCount: number;
    categoryKeywords: any[];
    categoryKeywordCount: number;
    recurringRules: any[];
    recurringRuleCount: number;
    totalRuleCount: number;
}

const overview = ref<Overview>({
    learningRules: [], learningRuleCount: 0,
    categoryKeywords: [], categoryKeywordCount: 0,
    recurringRules: [], recurringRuleCount: 0,
    totalRuleCount: 0,
});

const learningHeaders = [
    { title: 'Match Type', key: 'matchType' },
    { title: 'Match Value', key: 'matchValue' },
    { title: 'Learned Type', key: 'learnedType' },
    { title: 'Applied', key: 'appliedCount' },
    { title: 'Enabled', key: 'enabled' },
];

const keywordHeaders = [
    { title: 'Keyword', key: 'keyword' },
    { title: 'Category', key: 'categoryName' },
];

const recurringHeaders = [
    { title: 'Name', key: 'name' },
    { title: 'Amount', key: 'amount' },
    { title: 'Frequency', key: 'frequency' },
    { title: 'Next Date', key: 'nextDate' },
    { title: 'Enabled', key: 'enabled' },
];

async function fetchOverview() {
    loading.value = true;
    error.value = null;
    try {
        const resp = await services.getRulesOverview();
        if (resp.success && resp.data) {
            overview.value = resp.data;
        }
    } catch (e: any) {
        error.value = e.message || 'Failed to load rules';
    } finally {
        loading.value = false;
    }
}

onMounted(() => fetchOverview());
</script>
