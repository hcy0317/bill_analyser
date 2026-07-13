<template src="./list/ListPage.template.html"></template>

<script setup lang="ts">
import { useExternalTemplateBindings } from '@/lib/vue_external_template.ts';
import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import SettingsJsonImportExportButton from '@/components/desktop/SettingsJsonImportExportButton.vue';
import EditDialog from './list/dialogs/EditDialog.vue';
import PresetDialog from './list/dialogs/PresetDialog.vue';
import { useDesktopCategoryListPage } from './list/useCategoryListPage.ts';

const {
    tt,
    loading,
    primaryCategoryId,
    currentPrimaryCategory,
    navbar,
    confirmDialog,
    snackbar,
    editDialog,
    activeCategoryType,
    activeTab,
    updating,
    categoryHiding,
    categoryRemoving,
    displayOrderModified,
    cardMinHeight,
    alwaysShowNav,
    showNav,
    showHidden,
    showPresetDialog,
    primaryCategories,
    secondaryCategories,
    hasSubCategories,
    categories,
    noAvailableCategory,
    noCategory,
    availableCategoryCount,
    canAddSecondaryCategory,
    updateCardMinHeight,
    isCategorySupportSwitch,
    switchAllPrimaryCategories,
    switchPrimaryCategory,
    reload,
    addPrimaryCategory,
    addSecondaryCategory,
    edit,
    hide,
    remove,
    saveSortResult,
    onMove,
    onPresetCategorySaved,
    CategoryType,
    mdiRefresh,
    mdiMenu,
    mdiPencilOutline,
    mdiEyeOffOutline,
    mdiEyeOutline,
    mdiDeleteOutline,
    mdiDrag,
    mdiDotsVertical
} = useDesktopCategoryListPage();

useExternalTemplateBindings(ConfirmDialog, SnackBar, SettingsJsonImportExportButton, EditDialog, PresetDialog, useDesktopCategoryListPage, tt, loading, primaryCategoryId, currentPrimaryCategory, navbar, confirmDialog, snackbar, editDialog, activeCategoryType, activeTab, updating, categoryHiding, categoryRemoving, displayOrderModified, cardMinHeight, alwaysShowNav, showNav, showHidden, showPresetDialog, primaryCategories, secondaryCategories, hasSubCategories, categories, noAvailableCategory, noCategory, availableCategoryCount, canAddSecondaryCategory, updateCardMinHeight, isCategorySupportSwitch, switchAllPrimaryCategories, switchPrimaryCategory, reload, addPrimaryCategory, addSecondaryCategory, edit, hide, remove, saveSortResult, onMove, onPresetCategorySaved, CategoryType, mdiRefresh, mdiMenu, mdiPencilOutline, mdiEyeOffOutline, mdiEyeOutline, mdiDeleteOutline, mdiDrag, mdiDotsVertical);
</script>

<style src="./list/ListPage.css"></style>
