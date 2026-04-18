<template>
    <v-row class="match-height">
        <v-col cols="12">
            <v-card>
                <v-card-text>
                    <div class="d-flex align-center mb-4">
                        <v-icon :icon="mdiCalendarMonth" class="mr-2" />
                        <span class="text-h6">{{ tt('Calendar View') }}</span>
                        <v-spacer />
                        <v-btn variant="outlined" size="small" class="mr-2" @click="prevMonth">
                            <v-icon :icon="mdiChevronLeft" />
                        </v-btn>
                        <span class="text-subtitle-1 font-weight-medium mx-2">{{ currentMonthLabel }}</span>
                        <v-btn variant="outlined" size="small" class="mr-2" @click="nextMonth">
                            <v-icon :icon="mdiChevronRight" />
                        </v-btn>
                        <v-btn variant="tonal" size="small" @click="goToday">{{ tt('Today') }}</v-btn>
                    </div>

                    <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-4" />

                    <v-alert v-if="error" type="error" closable class="mb-4" @click:close="error = null">
                        {{ error }}
                    </v-alert>

                    <!-- Summary -->
                    <v-row class="mb-4">
                        <v-col cols="3">
                            <v-card variant="tonal" color="success">
                                <v-card-text class="text-center">
                                    <div class="text-caption">{{ tt('Income') }}</div>
                                    <div class="text-h6">¥{{ monthSummary.income.toFixed(2) }}</div>
                                </v-card-text>
                            </v-card>
                        </v-col>
                        <v-col cols="3">
                            <v-card variant="tonal" color="error">
                                <v-card-text class="text-center">
                                    <div class="text-caption">{{ tt('Expense') }}</div>
                                    <div class="text-h6">¥{{ monthSummary.expense.toFixed(2) }}</div>
                                </v-card-text>
                            </v-card>
                        </v-col>
                        <v-col cols="3">
                            <v-card variant="tonal" color="info">
                                <v-card-text class="text-center">
                                    <div class="text-caption">{{ tt('Net') }}</div>
                                    <div class="text-h6">¥{{ monthSummary.net.toFixed(2) }}</div>
                                </v-card-text>
                            </v-card>
                        </v-col>
                        <v-col cols="3">
                            <v-card variant="tonal">
                                <v-card-text class="text-center">
                                    <div class="text-caption">{{ tt('Transactions') }}</div>
                                    <div class="text-h6">{{ monthSummary.count }}</div>
                                </v-card-text>
                            </v-card>
                        </v-col>
                    </v-row>

                    <!-- Calendar Grid -->
                    <v-table density="compact">
                        <thead>
                            <tr>
                                <th v-for="day in weekDays" :key="day" class="text-center">{{ day }}</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="(week, wi) in calendarWeeks" :key="wi">
                                <td v-for="(cell, ci) in week" :key="ci"
                                    class="text-center pa-1"
                                    :class="{ 'bg-grey-lighten-4': !cell.isCurrentMonth }"
                                    style="vertical-align: top; min-height: 80px; height: 80px; width: 14.28%;">
                                    <div :class="{ 'font-weight-bold': cell.isToday, 'text-grey': !cell.isCurrentMonth }">
                                        {{ cell.day }}
                                    </div>
                                    <div v-if="cell.data" class="text-caption">
                                        <div v-if="cell.data.income > 0" class="text-success">+{{ cell.data.income.toFixed(0) }}</div>
                                        <div v-if="cell.data.expense > 0" class="text-error">-{{ cell.data.expense.toFixed(0) }}</div>
                                    </div>
                                    <div v-if="cell.recurring.length > 0">
                                        <v-icon :icon="mdiRepeat" size="x-small" color="purple" :title="cell.recurring.length + ' recurring'" />
                                    </div>
                                </td>
                            </tr>
                        </tbody>
                    </v-table>

                    <!-- Recurring Projections -->
                    <div v-if="recurringProjections.length > 0" class="mt-4">
                        <div class="text-subtitle-2 mb-2">
                            <v-icon :icon="mdiRepeat" size="small" class="mr-1" />
                            {{ tt('Recurring Projections') }} ({{ recurringProjections.length }})
                        </div>
                        <v-chip v-for="(rp, i) in recurringProjections.slice(0, 20)" :key="i"
                                size="small" variant="tonal" color="purple" class="mr-1 mb-1">
                            {{ rp.date }} · {{ rp.name }} · ¥{{ Math.abs(rp.amount).toFixed(2) }}
                        </v-chip>
                    </div>
                </v-card-text>
            </v-card>
        </v-col>
    </v-row>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue';
import { mdiCalendarMonth, mdiChevronLeft, mdiChevronRight, mdiRepeat } from '@mdi/js';
import services from '@/lib/services.ts';

const tt = (key: string) => key;

const loading = ref(false);
const error = ref<string | null>(null);

const currentYear = ref(new Date().getFullYear());
const currentMonth = ref(new Date().getMonth()); // 0-indexed

interface DayEvent {
    date: string;
    income: number;
    expense: number;
    net: number;
    count: number;
    bills: any[];
}

interface RecurringProjection {
    date: string;
    name: string;
    amount: number;
    billType: string;
    frequency: string;
}

const events = ref<DayEvent[]>([]);
const recurringProjections = ref<RecurringProjection[]>([]);

const currentMonthLabel = computed(() => {
    return `${currentYear.value}-${String(currentMonth.value + 1).padStart(2, '0')}`;
});

const weekDays = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

const monthSummary = computed(() => {
    let income = 0, expense = 0, count = 0;
    for (const e of events.value) {
        income += e.income;
        expense += e.expense;
        count += e.count;
    }
    return { income, expense, net: income - expense, count };
});

interface CalendarCell {
    day: number;
    date: string;
    isCurrentMonth: boolean;
    isToday: boolean;
    data: DayEvent | null;
    recurring: RecurringProjection[];
}

const calendarWeeks = computed(() => {
    const year = currentYear.value;
    const month = currentMonth.value;
    const firstDay = new Date(year, month, 1);
    const lastDay = new Date(year, month + 1, 0);
    const today = new Date();
    const todayStr = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}-${String(today.getDate()).padStart(2, '0')}`;

    const eventMap = new Map<string, DayEvent>();
    for (const e of events.value) {
        eventMap.set(e.date, e);
    }

    const recurringMap = new Map<string, RecurringProjection[]>();
    for (const rp of recurringProjections.value) {
        if (!recurringMap.has(rp.date)) recurringMap.set(rp.date, []);
        recurringMap.get(rp.date)!.push(rp);
    }

    const weeks: CalendarCell[][] = [];
    let startOffset = (firstDay.getDay() + 6) % 7; // Monday start
    let currentDate = new Date(year, month, 1 - startOffset);

    for (let w = 0; w < 6; w++) {
        const week: CalendarCell[] = [];
        for (let d = 0; d < 7; d++) {
            const dateStr = `${currentDate.getFullYear()}-${String(currentDate.getMonth() + 1).padStart(2, '0')}-${String(currentDate.getDate()).padStart(2, '0')}`;
            week.push({
                day: currentDate.getDate(),
                date: dateStr,
                isCurrentMonth: currentDate.getMonth() === month,
                isToday: dateStr === todayStr,
                data: eventMap.get(dateStr) || null,
                recurring: recurringMap.get(dateStr) || [],
            });
            currentDate = new Date(currentDate.getTime() + 86400000);
        }
        weeks.push(week);
        if (currentDate.getMonth() !== month && currentDate.getDate() > 7) break;
    }
    return weeks;
});

async function fetchEvents() {
    loading.value = true;
    error.value = null;
    try {
        const startDate = `${currentYear.value}-${String(currentMonth.value + 1).padStart(2, '0')}-01`;
        const lastDay = new Date(currentYear.value, currentMonth.value + 1, 0);
        const endDate = `${lastDay.getFullYear()}-${String(lastDay.getMonth() + 1).padStart(2, '0')}-${String(lastDay.getDate()).padStart(2, '0')}`;

        const resp = await services.getCalendarEvents({ startDate, endDate });
        if (resp.success && resp.data) {
            events.value = resp.data.events || [];
            recurringProjections.value = resp.data.recurringProjections || [];
        }
    } catch (e: any) {
        error.value = e.message || 'Failed to load calendar';
    } finally {
        loading.value = false;
    }
}

function prevMonth() {
    if (currentMonth.value === 0) {
        currentMonth.value = 11;
        currentYear.value--;
    } else {
        currentMonth.value--;
    }
}

function nextMonth() {
    if (currentMonth.value === 11) {
        currentMonth.value = 0;
        currentYear.value++;
    } else {
        currentMonth.value++;
    }
}

function goToday() {
    currentYear.value = new Date().getFullYear();
    currentMonth.value = new Date().getMonth();
}

watch([currentYear, currentMonth], () => fetchEvents());

onMounted(() => fetchEvents());
</script>
