<template>
    <v-table v-if="pairs.length > 0" hover density="comfortable">
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
                           :disabled="deletingPairId === pair.id"
                           @click="emit('delete', pair)">
                        <v-icon :icon="mdiDeleteOutline" />
                        <v-tooltip activator="parent" location="top">{{ tt('Delete') }}</v-tooltip>
                    </v-btn>
                </td>
            </tr>
        </tbody>
    </v-table>

    <v-empty-state v-else
                   :icon="mdiLinkOff"
                   :headline="emptyHeadline"
                   :text="emptyText" />
</template>

<script setup lang="ts">
import { mdiDeleteOutline, mdiLinkOff } from '@mdi/js';

import type { BillMatchingPairDetail } from '@/models/bill_matching.ts';
import { useI18n } from '@/locales/helpers.ts';

defineProps<{
    pairs: BillMatchingPairDetail[];
    deletingPairId: number | null;
    emptyHeadline: string;
    emptyText: string;
}>();

const emit = defineEmits<{
    delete: [pair: BillMatchingPairDetail];
}>();

const { tt } = useI18n();

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

function formatAmount(amount: number): string {
    return amount.toFixed(2);
}
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
