<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-card-text>
                    <div class="d-flex align-center mb-4">
                        <v-icon :icon="mdiWallet" class="mr-2" />
                        <span class="text-h6">{{ tt('Net Worth') }}</span>
                        <v-spacer />
                        <v-btn variant="outlined" :disabled="loading" @click="fetchSnapshot">
                            <v-icon start :icon="mdiRefresh" />
                            {{ tt('Refresh') }}
                        </v-btn>
                    </div>

                    <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-4" />

                    <v-alert v-if="error" type="error" closable class="mb-4" @click:close="error = null">
                        {{ error }}
                    </v-alert>

                    <!-- Net Worth Summary -->
                    <v-row class="mb-6">
                        <v-col cols="4">
                            <v-card variant="tonal" color="success">
                                <v-card-text class="text-center">
                                    <div class="text-caption">{{ tt('Total Assets') }}</div>
                                    <div class="text-h5 font-weight-bold">¥{{ (snapshot.totalAssets || 0).toFixed(2) }}</div>
                                </v-card-text>
                            </v-card>
                        </v-col>
                        <v-col cols="4">
                            <v-card variant="tonal" color="error">
                                <v-card-text class="text-center">
                                    <div class="text-caption">{{ tt('Total Liabilities') }}</div>
                                    <div class="text-h5 font-weight-bold">¥{{ (snapshot.totalLiabilities || 0).toFixed(2) }}</div>
                                </v-card-text>
                            </v-card>
                        </v-col>
                        <v-col cols="4">
                            <v-card variant="tonal" color="primary">
                                <v-card-text class="text-center">
                                    <div class="text-caption">{{ tt('Net Worth') }}</div>
                                    <div class="text-h5 font-weight-bold">¥{{ (snapshot.netWorth || 0).toFixed(2) }}</div>
                                </v-card-text>
                            </v-card>
                        </v-col>
                    </v-row>

                    <!-- Assets -->
                    <div class="text-subtitle-1 font-weight-medium mb-2">
                        <v-icon :icon="mdiTrendingUp" color="success" size="small" class="mr-1" />
                        {{ tt('Assets') }} ({{ snapshot.assets?.length || 0 }})
                    </div>
                    <v-table density="compact" class="mb-6">
                        <thead>
                            <tr>
                                <th>{{ tt('Account') }}</th>
                                <th>{{ tt('Type') }}</th>
                                <th class="text-right">{{ tt('Balance') }}</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="acc in snapshot.assets || []" :key="acc.id">
                                <td>{{ acc.name }}</td>
                                <td>{{ acc.type }}</td>
                                <td class="text-right text-success">¥{{ acc.balance.toFixed(2) }}</td>
                            </tr>
                            <tr v-if="!snapshot.assets?.length">
                                <td colspan="3" class="text-center text-grey">{{ tt('No assets') }}</td>
                            </tr>
                        </tbody>
                    </v-table>

                    <!-- Liabilities -->
                    <div class="text-subtitle-1 font-weight-medium mb-2">
                        <v-icon :icon="mdiTrendingDown" color="error" size="small" class="mr-1" />
                        {{ tt('Liabilities') }} ({{ snapshot.liabilities?.length || 0 }})
                    </div>
                    <v-table density="compact">
                        <thead>
                            <tr>
                                <th>{{ tt('Account') }}</th>
                                <th>{{ tt('Type') }}</th>
                                <th class="text-right">{{ tt('Balance') }}</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="acc in snapshot.liabilities || []" :key="acc.id">
                                <td>{{ acc.name }}</td>
                                <td>{{ acc.type }}</td>
                                <td class="text-right text-error">¥{{ acc.balance.toFixed(2) }}</td>
                            </tr>
                            <tr v-if="!snapshot.liabilities?.length">
                                <td colspan="3" class="text-center text-grey">{{ tt('No liabilities') }}</td>
                            </tr>
                        </tbody>
                    </v-table>
                </v-card-text>
            </v-card>
        </v-col>
    </v-row>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { mdiWallet, mdiRefresh, mdiTrendingUp, mdiTrendingDown } from '@mdi/js';
import services from '@/lib/services.ts';

const tt = (key: string) => key;

const loading = ref(false);
const error = ref<string | null>(null);

interface AccountEntry {
    id: number;
    name: string;
    type: string;
    icon: string;
    balance: number;
    currency: string;
}

interface Snapshot {
    assets: AccountEntry[];
    liabilities: AccountEntry[];
    totalAssets: number;
    totalLiabilities: number;
    netWorth: number;
    accountCount: number;
}

const snapshot = ref<Snapshot>({
    assets: [], liabilities: [],
    totalAssets: 0, totalLiabilities: 0, netWorth: 0, accountCount: 0,
});

async function fetchSnapshot() {
    loading.value = true;
    error.value = null;
    try {
        const resp = await services.getNetWorthSnapshot();
        if (resp.success && resp.data) {
            snapshot.value = resp.data;
        }
    } catch (e: any) {
        error.value = e.message || 'Failed to load net worth';
    } finally {
        loading.value = false;
    }
}

onMounted(() => fetchSnapshot());
</script>
