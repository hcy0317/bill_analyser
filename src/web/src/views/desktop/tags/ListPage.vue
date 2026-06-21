<template src="./list/ListPage.template.html"></template>

<script setup lang="ts">import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import SettingsJsonImportExportButton from '@/components/desktop/SettingsJsonImportExportButton.vue';
import { ref, computed, useTemplateRef } from 'vue';
import { useI18n } from '@/locales/helpers.ts';
import { useTransactionTagsStore } from '@/stores/transactionTag.ts';
import { TransactionTag } from '@/models/transaction_tag.ts';
import {
    isNoAvailableTag,
    getAvailableTagCount
} from '@/lib/tag.ts';
import logger from '@/lib/logger.ts';

import {
    mdiRefresh,
    mdiPencilOutline,
    mdiCheck,
    mdiClose,
    mdiEyeOffOutline,
    mdiEyeOutline,
    mdiDeleteOutline,
    mdiDrag,
    mdiDotsVertical,
    mdiPound
} from '@mdi/js';

type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;

const { tt } = useI18n();

const transactionTagsStore = useTransactionTagsStore();

const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
const snackbar = useTemplateRef<SnackBarType>('snackbar');

const newTag = ref<TransactionTag | null>(null);
const editingTag = ref<TransactionTag>(TransactionTag.createNewTag());
const loading = ref<boolean>(true);
const updating = ref<boolean>(false);
const tagUpdating = ref<Record<string, boolean>>({});
const tagHiding = ref<Record<string, boolean>>({});
const tagRemoving = ref<Record<string, boolean>>({});
const displayOrderModified = ref<boolean>(false);
const showHidden = ref<boolean>(false);

const tags = computed<TransactionTag[]>(() => transactionTagsStore.allTransactionTags);
const noAvailableTag = computed<boolean>(() => isNoAvailableTag(tags.value, showHidden.value));
const availableTagCount = computed<number>(() => getAvailableTagCount(tags.value, showHidden.value));
const hasEditingTag = computed<boolean>(() => !!(newTag.value || (editingTag.value.id && editingTag.value.id !== '')));

function isTagModified(tag: TransactionTag): boolean {
    if (tag.id) {
        return editingTag.value.name !== '' && editingTag.value.name !== tag.name;
    } else {
        return tag.name !== '';
    }
}

function reload(): void {
    if (hasEditingTag.value) {
        return;
    }

    loading.value = true;

    transactionTagsStore.loadAllTags({
        force: true
    }).then(() => {
        loading.value = false;
        displayOrderModified.value = false;

        snackbar.value?.showMessage('Tag list has been updated');
    }).catch(error => {
        loading.value = false;

        if (error && error.isUpToDate) {
            displayOrderModified.value = false;
        }

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function add(): void {
    newTag.value = TransactionTag.createNewTag();
}

function edit(tag: TransactionTag): void {
    editingTag.value.id = tag.id;
    editingTag.value.name = tag.name;
}

function save(tag: TransactionTag): void {
    updating.value = true;
    tagUpdating.value[tag.id || ''] = true;

    transactionTagsStore.saveTag({
        tag: tag
    }).then(() => {
        updating.value = false;
        tagUpdating.value[tag.id || ''] = false;

        if (tag.id) {
            editingTag.value.id = '';
            editingTag.value.name = '';
        } else {
            newTag.value = null;
        }

        // **刷新标签列表，显示最新的编辑结果**
        transactionTagsStore.loadAllTags({
            force: true
        }).then(() => {
            snackbar.value?.showMessage('Tag saved successfully');
        }).catch(error => {
            logger.error('[ListPage] Failed to reload tags after save', error);
        });
    }).catch(error => {
        updating.value = false;
        tagUpdating.value[tag.id || ''] = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function cancelSave(tag: TransactionTag): void {
    if (tag.id) {
        editingTag.value.id = '';
        editingTag.value.name = '';
    } else {
        newTag.value = null;
    }
}

function saveSortResult(): void {
    if (!displayOrderModified.value) {
        return;
    }

    loading.value = true;

    transactionTagsStore.updateTagDisplayOrders().then(() => {
        loading.value = false;
        displayOrderModified.value = false;
    }).catch(error => {
        loading.value = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function hide(tag: TransactionTag, hidden: boolean): void {
    logger.debug(`[ListPage] Hiding tag: ${tag.id}, hidden: ${hidden}`);
    updating.value = true;
    tagHiding.value[tag.id] = true;

    transactionTagsStore.hideTag({
        tag: tag,
        hidden: hidden
    }).then(() => {
        logger.debug(`[ListPage] Tag hidden successfully: ${tag.id}`);
        updating.value = false;
        tagHiding.value[tag.id] = false;
    }).catch(error => {
        logger.error(`[ListPage] Failed to hide tag: ${tag.id}`, error);
        updating.value = false;
        tagHiding.value[tag.id] = false;

        if (!error.processed) {
            snackbar.value?.showError(error);
        }
    });
}

function remove(tag: TransactionTag): void {
    logger.debug(`[ListPage] Removing tag: ${tag.id}`);
    confirmDialog.value?.open('Are you sure you want to delete this tag?').then(() => {
        updating.value = true;
        tagRemoving.value[tag.id] = true;

        transactionTagsStore.deleteTag({
            tag: tag
        }).then(() => {
            logger.debug(`[ListPage] Tag removed successfully: ${tag.id}`);
            updating.value = false;
            tagRemoving.value[tag.id] = false;
        }).catch(error => {
            logger.error(`[ListPage] Failed to remove tag: ${tag.id}`, error);
            updating.value = false;
            tagRemoving.value[tag.id] = false;

            if (!error.processed) {
                snackbar.value?.showError(error);
            }
        });
    });
}

function onMove(event: { moved: { element: { id: string }; oldIndex: number; newIndex: number } }): void {
    if (!event || !event.moved) {
        return;
    }

    const moveEvent = event.moved;

    if (!moveEvent.element || !moveEvent.element.id) {
        snackbar.value?.showMessage('Unable to move tag');
        return;
    }

    transactionTagsStore.changeTagDisplayOrder({
        tagId: moveEvent.element.id,
        from: moveEvent.oldIndex,
        to: moveEvent.newIndex
    }).then(() => {
        displayOrderModified.value = true;
    }).catch(error => {
        snackbar.value?.showError(error);
    });
}

transactionTagsStore.loadAllTags({
    force: false
}).then(() => {
    loading.value = false;
}).catch(error => {
    loading.value = false;

    if (!error.processed) {
        snackbar.value?.showError(error);
    }
});

useExternalTemplateBindings(ConfirmDialog, SnackBar, SettingsJsonImportExportButton, ref, computed, useTemplateRef, useI18n, useTransactionTagsStore, TransactionTag, isNoAvailableTag, getAvailableTagCount, logger, mdiRefresh, mdiPencilOutline, mdiCheck, mdiClose, mdiEyeOffOutline, mdiEyeOutline, mdiDeleteOutline, mdiDrag, mdiDotsVertical, mdiPound, tt, transactionTagsStore, confirmDialog, snackbar, newTag, editingTag, loading, updating, tagUpdating, tagHiding, tagRemoving, displayOrderModified, showHidden, tags, noAvailableTag, availableTagCount, hasEditingTag, isTagModified, reload, add, edit, save, cancelSave, saveSortResult, hide, remove, onMove);
</script>

<style src="./list/ListPage.css"></style>
