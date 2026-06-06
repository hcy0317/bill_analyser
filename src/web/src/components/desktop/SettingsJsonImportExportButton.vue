<template>
    <v-btn
        :class="props.buttonClass"
        color="default"
        variant="outlined"
        :disabled="props.disabled || busy"
        @click="importSection"
    >
        {{ tt('Import') }}
        <v-progress-circular
            v-if="importing"
            indeterminate
            size="22"
            class="ms-2"
        />
        <v-menu
            activator="parent"
            :open-on-hover="true"
            :open-delay="1500"
            location="bottom start"
        >
            <v-list density="compact" min-width="240" :disabled="props.disabled || busy">
                <v-list-item @click.stop="exportSection">
                    <v-list-item-title>{{ tt('Export Settings JSON') }}</v-list-item-title>
                </v-list-item>
            </v-list>
        </v-menu>
    </v-btn>

    <confirm-dialog ref="confirmDialog" />
    <password-dialog ref="passwordDialog" />
    <snack-bar ref="snackbar" />
</template>

<script setup lang="ts">
import { computed, ref, useTemplateRef } from 'vue';

import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import PasswordDialog from '@/components/desktop/PasswordDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import { useI18n } from '@/locales/helpers.ts';
import { getApiErrorMessageOrDefault } from '@/lib/api_error.ts';
import { openTextFileContent, startDownloadFile } from '@/lib/ui/common.ts';
import {
    SETTINGS_BUNDLE_SECTION_LABEL_KEYS,
    buildSettingsBundleSectionFileName,
    type SettingsBundleImportResult,
    type SettingsBundleSectionKey,
    type SettingsBundleImportSectionSummary,
} from '@/models/data_management.ts';
import { useUserStore } from '@/stores/user.ts';

type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type PasswordDialogType = InstanceType<typeof PasswordDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;

const props = withDefaults(defineProps<{
    sectionKey: SettingsBundleSectionKey;
    filenamePrefix?: string;
    disabled?: boolean;
    buttonClass?: string;
    passwordRequiredForExport?: boolean;
}>(), {
    filenamePrefix: '',
    disabled: false,
    buttonClass: 'ms-3',
    passwordRequiredForExport: false,
});

const emit = defineEmits<{
    imported: [result: SettingsBundleImportResult];
}>();

const { tt } = useI18n();
const userStore = useUserStore();

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const passwordDialog = useTemplateRef<PasswordDialogType>('passwordDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');

const importing = ref(false);
const exporting = ref(false);
const busy = computed(() => importing.value || exporting.value);

const emptySummary: SettingsBundleImportSectionSummary = {
    created: 0,
    updated: 0,
    skipped: 0,
};

function getSectionFileName(): string {
    return buildSettingsBundleSectionFileName(tt(SETTINGS_BUNDLE_SECTION_LABEL_KEYS[props.sectionKey]));
}

function buildImportDetails(result: SettingsBundleImportResult): string[] {
    const summary = result.sections[props.sectionKey] || emptySummary;
    const sectionLabel = tt(SETTINGS_BUNDLE_SECTION_LABEL_KEYS[props.sectionKey]);
    const details = [
        `${sectionLabel}: ${tt('Created')} ${summary.created}, ${tt('Updated')} ${summary.updated}, ${tt('Skipped')} ${summary.skipped}`,
    ];
    details.push(...result.warnings.slice(0, 5));
    return details;
}

async function exportSection(): Promise<void> {
    if (busy.value || props.disabled) {
        return;
    }

    let password: string | undefined;
    if (props.passwordRequiredForExport) {
        try {
            password = await passwordDialog.value?.open(
                'Verify Login Password',
                'Please enter your login password',
                {
                    label: 'Login Password',
                    placeholder: 'Please enter your login password',
                    hint: '',
                }
            );
        } catch {
            return;
        }
        if (!password) {
            return;
        }
    }

    exporting.value = true;

    userStore.getExportedSettingsBundleSection(props.sectionKey, { password }).then(data => {
        startDownloadFile(getSectionFileName(), data);
        exporting.value = false;
        snackbar.value?.showMessage('Settings exported');
    }).catch(error => {
        exporting.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

async function importSection(): Promise<void> {
    if (busy.value || props.disabled) {
        return;
    }

    try {
        const fileContent = await openTextFileContent({ allowedExtensions: '.json,application/json' });
        let bundle: unknown;

        try {
            bundle = JSON.parse(fileContent);
        } catch {
            snackbar.value?.showMessage('Invalid JSON file');
            return;
        }

        importing.value = true;
        const preview = await userStore.previewImportSettingsBundleSection(props.sectionKey, bundle);
        const confirmed = await confirmDialog.value?.open('Import settings from JSON?', {
            color: 'primary',
            details: buildImportDetails(preview),
        });

        if (!confirmed) {
            importing.value = false;
            return;
        }

        const result = await userStore.importSettingsBundleSection(props.sectionKey, bundle);
        importing.value = false;
        snackbar.value?.showMessage('Settings imported');
        emit('imported', result);
    } catch (error: unknown) {
        importing.value = false;

        const payload = error as { processed?: boolean };
        if (!payload.processed) {
            snackbar.value?.showError({
                message: getApiErrorMessageOrDefault(error, 'Unable to import settings bundle')
            });
        }
    }
}
</script>
