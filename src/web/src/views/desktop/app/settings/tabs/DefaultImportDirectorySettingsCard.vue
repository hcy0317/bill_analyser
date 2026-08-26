<template>
    <v-col cols="12">
        <v-card :title="tt('Import Transactions')">
            <v-card-text>
                <v-row align="center">
                    <v-col cols="12" md="8">
                        <v-text-field
                            persistent-placeholder
                            readonly
                            :loading="selecting"
                            :label="tt('Default Import Folder')"
                            :model-value="displayName"
                            @click="chooseDirectory"
                        />
                    </v-col>
                    <v-col cols="12" md="4" class="d-flex ga-2">
                        <v-btn color="primary" variant="tonal" :loading="selecting" @click="chooseDirectory">
                            {{ tt('Choose Folder') }}
                        </v-btn>
                        <v-btn variant="text" :disabled="!configuredName" @click="resetDirectory">
                            {{ tt('Reset') }}
                        </v-btn>
                    </v-col>
                </v-row>
                <p class="text-body-2 text-medium-emphasis mb-0">
                    {{ tt('The folder preference is stored only in this browser.') }}
                </p>
            </v-card-text>
        </v-card>
        <snack-bar ref="snackbar" />
    </v-col>
</template>

<script setup lang="ts">
import { computed, ref, useTemplateRef } from 'vue';

import SnackBar from '@/components/desktop/SnackBar.vue';
import { useI18n } from '@/locales/helpers.ts';
import { useSettingsStore } from '@/stores/setting.ts';
import {
    chooseDefaultImportDirectory,
    clearDefaultImportDirectory
} from '@/lib/importDirectoryPreference.ts';

type SnackBarType = InstanceType<typeof SnackBar>;

const { tt } = useI18n();
const settingsStore = useSettingsStore();
const snackbar = useTemplateRef<SnackBarType>('snackbar');
const selecting = ref<boolean>(false);

const configuredName = computed<string>(() => settingsStore.appSettings.billImportDefaultDirectoryName);
const displayName = computed<string>(() => configuredName.value || tt('Browser default'));

async function chooseDirectory(): Promise<void> {
    if (selecting.value) return;
    selecting.value = true;
    try {
        const selection = await chooseDefaultImportDirectory();
        settingsStore.setBillImportDefaultDirectoryName(selection.name);
        snackbar.value?.showMessage(tt('Default import folder updated'));
    } catch (error) {
        if (!(error instanceof DOMException && error.name === 'AbortError')) {
            snackbar.value?.showError(error instanceof Error ? error.message : String(error));
        }
    } finally {
        selecting.value = false;
    }
}

async function resetDirectory(): Promise<void> {
    try {
        await clearDefaultImportDirectory();
        settingsStore.setBillImportDefaultDirectoryName('');
    } catch (error) {
        snackbar.value?.showError(error instanceof Error ? error.message : String(error));
    }
}
</script>
