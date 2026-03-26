<template>
    <v-select
        persistent-placeholder
        :readonly="readonly"
        :disabled="disabled"
        :label="label"
        :menu-props="menuProps"
        :model-value="selectedDateTime"
        v-model:menu="menuState"
        @paste="onPaste"
    >
        <template #selection>
            <span class="cursor-pointer"
                  :class="displayMultiline ? 'date-time-select-display--multiline' : 'text-truncate'">
                <template v-if="displayMultiline && displayDatePart && displayTimePart">
                    <span class="date-time-select-display__line">{{ displayDatePart }}</span>
                    <span class="date-time-select-display__line">{{ displayTimePart }}</span>
                </template>
                <template v-else>
                    {{ displayTime }}
                </template>
            </span>
        </template>

        <template #no-data>
            <date-time-picker :is-dark-mode="isDarkMode"
                              :enable-time-picker="false"
                              :vertical="true"
                              :show-alternate-dates="true"
                              v-model="draftDateTime">
            </date-time-picker>
            <div class="date-time-select-time-picker-container">
                <v-btn class="px-3" color="primary" variant="flat"
                       v-if="!is24Hour && isMeridiemIndicatorFirst"
                       @click="toggleMeridiemIndicator">
                    {{ tt(`datetime.${currentMeridiemIndicator}.content`) }}
                </v-btn>
                <v-autocomplete eager ref="hourInput"
                                density="compact"
                                max-width="70px"
                                item-title="value"
                                item-value="value"
                                auto-select-first="exact"
                                :items="hourItems"
                                :hide-no-data="true"
                                v-model="currentHour"
                                @update:focused="onFocused(hourInput, $event)"
                                @click="onFocused(hourInput, true)"
                                @keydown="onKeyDown('hour', $event)"
                />
                <span>:</span>
                <v-autocomplete eager ref="minuteInput"
                                density="compact"
                                max-width="70px"
                                item-title="value"
                                item-value="value"
                                auto-select-first="exact"
                                :items="minuteItems"
                                :hide-no-data="true"
                                v-model="currentMinute"
                                @update:focused="onFocused(minuteInput, $event)"
                                @click="onFocused(minuteInput, true)"
                                @keydown="onKeyDown('minute', $event)"
                />
                <span>:</span>
                <v-autocomplete eager ref="secondInput"
                                density="compact"
                                max-width="70px"
                                item-title="value"
                                item-value="value"
                                auto-select-first="exact"
                                :items="secondItems"
                                :hide-no-data="true"
                                v-model="currentSecond"
                                @update:focused="onFocused(secondInput, $event)"
                                @click="onFocused(secondInput, true)"
                                @keydown="onKeyDown('second', $event)"
                />
                <v-btn class="px-3" color="primary" variant="flat"
                       v-if="!is24Hour && !isMeridiemIndicatorFirst"
                       @click="toggleMeridiemIndicator">
                    {{ tt(`datetime.${currentMeridiemIndicator}.content`) }}
                </v-btn>
            </div>
            <div class="date-time-select-actions">
                <v-btn variant="tonal" color="secondary" @click="cancelSelection">
                    {{ tt('Cancel') }}
                </v-btn>
                <v-btn variant="flat" color="primary" @click="confirmSelection">
                    {{ tt('Confirm') }}
                </v-btn>
            </div>
        </template>
    </v-select>
</template>

<script setup lang="ts">
import { VAutocomplete } from 'vuetify/components/VAutocomplete';

import { computed, nextTick, ref, useTemplateRef, watch } from 'vue';
import { useTheme } from 'vuetify';

import { useI18n } from '@/locales/helpers.ts';
import { type TimePickerValue, useDateTimeSelectionBase } from '@/components/base/DateTimeSelectionBase.ts';

import { ThemeType } from '@/core/theme.ts';
import { NumeralSystem } from '@/core/numeral.ts';
import {
    type DateTime,
    MeridiemIndicator,
    KnownDateTimeFormat
} from '@/core/datetime.ts';
import {
    getHourIn12HourFormat,
    getTimezoneOffsetMinutes,
    getBrowserTimezoneOffsetMinutes,
    getLocalDatetimeFromUnixTime,
    getUnixTimeFromLocalDatetime,
    getActualUnixTimeForStore,
    getDummyUnixTimeForLocalUsage,
    parseDateTimeFromKnownDateTimeFormat,
    getAMOrPM,
    getCombinedDateAndTimeValues
} from '@/lib/datetime.ts';
import { setChildInputFocus } from '@/lib/ui/desktop.ts';

const props = defineProps<{
    modelValue: number;
    disabled?: boolean;
    readonly?: boolean;
    label?: string;
    displayMultiline?: boolean;
    preferMenuOnTop?: boolean;
}>();

const emit = defineEmits<{
    (e: 'update:modelValue', value: number): void;
    (e: 'error', message: string): void;
}>();

const theme = useTheme();
const {
    tt,
    getCurrentNumeralSystemType,
    parseDateTimeFromLongDateTime,
    parseDateTimeFromShortDateTime,
    formatUnixTimeToLongDateTime
} = useI18n();

const {
    is24Hour,
    isHourTwoDigits,
    isMinuteTwoDigits,
    isSecondTwoDigits,
    isMeridiemIndicatorFirst,
    getDisplayTimeValue,
    generateAllHours,
    generateAllMinutesOrSeconds
} = useDateTimeSelectionBase();

const hourInput = useTemplateRef<VAutocomplete>('hourInput');
const minuteInput = useTemplateRef<VAutocomplete>('minuteInput');
const secondInput = useTemplateRef<VAutocomplete>('secondInput');

const isDarkMode = computed<boolean>(() => theme.global.name.value === ThemeType.Dark);
const numeralSystem = computed<NumeralSystem>(() => getCurrentNumeralSystemType());
const menuProps = computed<Record<string, unknown>>(() => {
    return {
        contentClass: 'date-time-select-menu',
        location: props.preferMenuOnTop ? 'top' : 'bottom',
        origin: props.preferMenuOnTop ? 'bottom center' : 'top center',
        offset: 8,
        scrollStrategy: 'reposition'
    };
});
const menuState = ref<boolean>(false);
const closingWithAction = ref<boolean>(false);
const draftDateTime = ref<Date>(getLocalDatetimeFromUnixTime(props.modelValue));
const selectedDateTime = computed<Date>(() => getLocalDatetimeFromUnixTime(props.modelValue));

const displayTime = computed<string>(() => formatUnixTimeToLongDateTime(getActualUnixTimeForStore(props.modelValue, getTimezoneOffsetMinutes(), getBrowserTimezoneOffsetMinutes())));
const displayDatePart = computed<string>(() => {
    if (!props.displayMultiline) {
        return '';
    }

    const separatorIndex = displayTime.value.indexOf(' ');
    if (separatorIndex < 0) {
        return displayTime.value;
    }

    return displayTime.value.slice(0, separatorIndex);
});
const displayTimePart = computed<string>(() => {
    if (!props.displayMultiline) {
        return '';
    }

    const separatorIndex = displayTime.value.indexOf(' ');
    if (separatorIndex < 0) {
        return '';
    }

    return displayTime.value.slice(separatorIndex + 1);
});

const hourItems = computed<TimePickerValue[]>(() => generateAllHours(1, isHourTwoDigits.value));
const minuteItems = computed<TimePickerValue[]>(() => generateAllMinutesOrSeconds(1, isMinuteTwoDigits.value));
const secondItems = computed<TimePickerValue[]>(() => generateAllMinutesOrSeconds(1, isSecondTwoDigits.value));

const currentMeridiemIndicator = computed<string>({
    get: () => {
        return getAMOrPM(draftDateTime.value.getHours())
    },
    set: (value: string) => {
        if (value !== MeridiemIndicator.AM.name && value !== MeridiemIndicator.PM.name) {
            return;
        }

        draftDateTime.value = getCombinedDateAndTimeValues(draftDateTime.value, numeralSystem.value, currentHour.value, currentMinute.value, currentSecond.value, value, is24Hour.value);
    }
});
const currentHour = computed<string>({
    get: () => {
        return getDisplayTimeValue(is24Hour.value ? draftDateTime.value.getHours() : getHourIn12HourFormat(draftDateTime.value.getHours()), isHourTwoDigits.value);
    },
    set: (value: string) => {
        const hour = numeralSystem.value.parseInt(value);

        if (isNaN(hour) || hour < 0 || (is24Hour.value ? hour > 23 : hour > 12)) {
            return;
        }

        draftDateTime.value = getCombinedDateAndTimeValues(draftDateTime.value, numeralSystem.value, value, currentMinute.value, currentSecond.value, currentMeridiemIndicator.value, is24Hour.value);
    }
});
const currentMinute = computed<string>({
    get: () => {
        return getDisplayTimeValue(draftDateTime.value.getMinutes(), isMinuteTwoDigits.value);
    },
    set: (value: string) => {
        const minute = numeralSystem.value.parseInt(value);

        if (isNaN(minute) || minute < 0 || minute > 59) {
            return;
        }

        draftDateTime.value = getCombinedDateAndTimeValues(draftDateTime.value, numeralSystem.value, currentHour.value, value, currentSecond.value, currentMeridiemIndicator.value, is24Hour.value);
    }
});
const currentSecond = computed<string>({
    get: () => {
        return getDisplayTimeValue(draftDateTime.value.getSeconds(), isSecondTwoDigits.value);
    },
    set: (value: string) => {
        const second = numeralSystem.value.parseInt(value);

        if (isNaN(second) || second < 0 || second > 59) {
            return;
        }

        draftDateTime.value = getCombinedDateAndTimeValues(draftDateTime.value, numeralSystem.value, currentHour.value, currentMinute.value, value, currentMeridiemIndicator.value, is24Hour.value);
    }
});

watch(() => props.modelValue, (value: number) => {
    if (!menuState.value) {
        draftDateTime.value = getLocalDatetimeFromUnixTime(value);
    }
}, { immediate: true });

watch(menuState, (opened: boolean) => {
    if (opened) {
        draftDateTime.value = getLocalDatetimeFromUnixTime(props.modelValue);
        return;
    }

    if (!closingWithAction.value) {
        draftDateTime.value = getLocalDatetimeFromUnixTime(props.modelValue);
    }

    closingWithAction.value = false;
});

function commitDateTimeValue(value: Date): boolean {
    const unixTime = getUnixTimeFromLocalDatetime(value);

    if (unixTime < 0) {
        emit('error', 'Date is too early');
        return false;
    }

    emit('update:modelValue', unixTime);
    return true;
}

function cancelSelection(): void {
    draftDateTime.value = getLocalDatetimeFromUnixTime(props.modelValue);
    closingWithAction.value = true;
    menuState.value = false;
}

function confirmSelection(): void {
    if (!commitDateTimeValue(draftDateTime.value)) {
        return;
    }

    closingWithAction.value = true;
    menuState.value = false;
}

function toggleMeridiemIndicator(): void {
    if (currentMeridiemIndicator.value === MeridiemIndicator.AM.name) {
        currentMeridiemIndicator.value = MeridiemIndicator.PM.name;
    } else {
        currentMeridiemIndicator.value = MeridiemIndicator.AM.name;
    }
}

function onPaste(event: ClipboardEvent): void {
    if (!event.clipboardData || props.readonly || props.disabled) {
        event.preventDefault();
        return;
    }

    let text = event.clipboardData.getData('Text');

    if (!text) {
        event.preventDefault();
        return;
    }

    text = text.trim();

    const formats = KnownDateTimeFormat.detect(text);
    let dt: DateTime | undefined = undefined;

    if (formats && formats.length === 1) {
        dt = parseDateTimeFromKnownDateTimeFormat(text, formats[0] as KnownDateTimeFormat);

        if (dt) {
            draftDateTime.value = getLocalDatetimeFromUnixTime(getDummyUnixTimeForLocalUsage(dt.getUnixTime(), getTimezoneOffsetMinutes(), getBrowserTimezoneOffsetMinutes()));
            commitDateTimeValue(draftDateTime.value);
            return;
        }
    }

    dt = parseDateTimeFromLongDateTime(text);

    if (dt) {
        draftDateTime.value = getLocalDatetimeFromUnixTime(getDummyUnixTimeForLocalUsage(dt.getUnixTime(), getTimezoneOffsetMinutes(), getBrowserTimezoneOffsetMinutes()));
        commitDateTimeValue(draftDateTime.value);
        return;
    }

    dt = parseDateTimeFromShortDateTime(text);

    if (dt) {
        draftDateTime.value = getLocalDatetimeFromUnixTime(getDummyUnixTimeForLocalUsage(dt.getUnixTime(), getTimezoneOffsetMinutes(), getBrowserTimezoneOffsetMinutes()));
        commitDateTimeValue(draftDateTime.value);
        return;
    }

    event.preventDefault();
}

function onFocused(input: VAutocomplete | null | undefined, focused: boolean): void {
    if (input && focused) {
        nextTick(() => {
            setChildInputFocus(input?.$el, 'input');
        });
    }
}

function onKeyDown(type: string, e: KeyboardEvent): void {
    if (e.altKey || e.ctrlKey || e.metaKey || (e.key.indexOf('F') === 0 && (e.key.length === 2 || e.key.length === 3))
        || e.key === 'ArrowLeft' || e.key === 'ArrowRight'
        || e.key === 'Home' || e.key === 'End'
        || e.key === 'Backspace' || e.key === 'Delete' || e.key === 'Del') {
        return;
    }

    if (e.key.length === 1 && numeralSystem.value.isDigit(e.key)) {
        return;
    }

    let value = '';

    if (e.target instanceof HTMLInputElement) {
        const input = e.target as HTMLInputElement;
        value = input.value;
    }

    if (!value) {
        return;
    }

    if (e.key === 'Tab' || e.key === 'Enter') {
        if (type === 'hour') {
            currentHour.value = value;
        } else if (type === 'minute') {
            currentMinute.value = value;
        } else if (type === 'second') {
            currentSecond.value = value;
        }
    }

    if (e.shiftKey && e.key === 'Tab') {
        if (type === 'minute') {
            nextTick(() => {
                setTimeout(() => {
                    setChildInputFocus(hourInput.value?.$el, 'input');
                }, 50);
            });
        } else if (type === 'second') {
            nextTick(() => {
                setTimeout(() => {
                    setChildInputFocus(minuteInput.value?.$el, 'input');
                }, 50);
            });
        }

        e.preventDefault();
        e.stopPropagation();
        return;
    }

    if (!e.shiftKey && (e.key === 'Tab' || e.key === 'Enter')) {
        if (type === 'hour') {
            nextTick(() => {
                setTimeout(() => {
                    setChildInputFocus(minuteInput.value?.$el, 'input');
                }, 50);
            });
        } else if (type === 'minute') {
            nextTick(() => {
                setTimeout(() => {
                    setChildInputFocus(secondInput.value?.$el, 'input');
                }, 50);
            });
        }
    }

    e.preventDefault();
}
</script>

<style>
.date-time-select-menu {
    max-height: min(680px, calc(100vh - 32px)) !important;
    overflow-y: auto;
}

.date-time-select-menu .dp__menu {
    border: 0;
}

.date-time-select-menu .v-list {
    padding: 0;
}

.date-time-select-time-picker-container {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--dp-menu-padding);
    padding-bottom: 0;
    column-gap: 8px;
}

.date-time-select-time-picker-container .v-autocomplete.v-input--density-compact {
    --v-input-control-height: 38px;
    --v-field-input-padding-top: 4px;
    --v-field-input-padding-bottom: 4px;
}

.date-time-select-time-picker-container .v-autocomplete.v-input--density-compact .v-field {
    --v-field-padding-start: 12px;
    --v-field-padding-end: 0;
}

.date-time-select-time-picker-container .v-autocomplete.v-input--density-compact .v-field__input {
    min-height: 38px !important;
}

.date-time-select-time-picker-container .v-autocomplete.v-input--density-compact .v-field__append-inner .v-autocomplete__menu-icon {
    margin-inline-start: 0;
}

.date-time-select-time-picker-container .v-autocomplete .v-field--appended {
    padding-inline-end: 8px;
}

.date-time-select-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px 16px;
    position: sticky;
    bottom: 0;
    background-color: rgb(var(--v-theme-surface));
    border-top: 1px solid rgba(var(--v-theme-outline), 0.2);
}

.date-time-select-display--multiline {
    display: inline-flex;
    flex-direction: column;
    line-height: 1.15;
    white-space: normal;
}

.date-time-select-display__line {
    display: block;
    white-space: nowrap;
}
</style>
