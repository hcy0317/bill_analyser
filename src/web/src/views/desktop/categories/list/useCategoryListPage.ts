import { VNavigationDrawer } from 'vuetify/components/VNavigationDrawer';

import ConfirmDialog from '@/components/desktop/ConfirmDialog.vue';
import SnackBar from '@/components/desktop/SnackBar.vue';
import EditDialog from './dialogs/EditDialog.vue';

import { ref, computed, useTemplateRef, watch, nextTick } from 'vue';
import { useDisplay } from 'vuetify';

import { useI18n } from '@/locales/helpers.ts';
import { useCategoryListPageBase } from '@/views/base/categories/CategoryListPageBase.ts';

import { useTransactionCategoriesStore } from '@/stores/transactionCategory.ts';

import { CategoryType } from '@/core/category.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';

import {
    isNoAvailableCategory,
    getAvailableCategoryCount
} from '@/lib/category.ts';
import { getNavSideBarOuterHeight } from '@/lib/ui/desktop.ts';

import {
    mdiRefresh,
    mdiMenu,
    mdiPencilOutline,
    mdiEyeOffOutline,
    mdiEyeOutline,
    mdiDeleteOutline,
    mdiDrag,
    mdiDotsVertical
} from '@mdi/js';

type ConfirmDialogType = InstanceType<typeof ConfirmDialog>;
type SnackBarType = InstanceType<typeof SnackBar>;
type EditDialogType = InstanceType<typeof EditDialog>;
type SnackBarError = Parameters<SnackBarType['showError']>[0];

// 中文说明：收拢桌面分类列表页的状态、导入导出入口、编辑弹窗回调与显示顺序保存流程。
// 维护重点：页面 facade 只负责暴露模板绑定；这里仍复用现有 store/service 合同，不改变分类编辑或导入导出行为。
export function useDesktopCategoryListPage() {
    const display = useDisplay();
    const { tt } = useI18n();
    const { loading, primaryCategoryId, currentPrimaryCategory } = useCategoryListPageBase();

    const transactionCategoriesStore = useTransactionCategoriesStore();

    const navbar = useTemplateRef<VNavigationDrawer>('navbar');
    const confirmDialog = useTemplateRef<ConfirmDialogType>('confirmDialog');
    const snackbar = useTemplateRef<SnackBarType>('snackbar');
    const editDialog = useTemplateRef<EditDialogType>('editDialog');

    const activeCategoryType = ref<CategoryType>(CategoryType.Expense);
    const activeTab = ref<string>('categoryPage');
    const updating = ref<boolean>(false);
    const categoryHiding = ref<Record<string, boolean>>({});
    const categoryRemoving = ref<Record<string, boolean>>({});
    const displayOrderModified = ref<boolean>(false);
    const cardMinHeight = ref<number>(680);
    const alwaysShowNav = ref<boolean>(display.mdAndUp.value);
    const showNav = ref<boolean>(display.mdAndUp.value);
    const showHidden = ref<boolean>(false);
    const showPresetDialog = ref<boolean>(false);

    const primaryCategories = computed<TransactionCategory[]>(() => {
        if (!transactionCategoriesStore.allTransactionCategories || !transactionCategoriesStore.allTransactionCategories[activeCategoryType.value]) {
            return [];
        }

        return transactionCategoriesStore.allTransactionCategories[activeCategoryType.value] ?? [];
    });

    const secondaryCategories = computed<TransactionCategory[]>(() => {
        if (!transactionCategoriesStore.allTransactionCategoriesMap || !transactionCategoriesStore.allTransactionCategoriesMap[primaryCategoryId.value]) {
            return [];
        }

        return transactionCategoriesStore.allTransactionCategoriesMap[primaryCategoryId.value]?.subCategories ?? [];
    });

    const hasSubCategories = computed<boolean>(() => {
        return !primaryCategoryId.value || primaryCategoryId.value === '' || primaryCategoryId.value === '0';
    });

    const categories = computed<TransactionCategory[]>(() => {
        if (hasSubCategories.value) {
            return primaryCategories.value;
        } else {
            return secondaryCategories.value;
        }
    });

    const noAvailableCategory = computed<boolean>(() => isNoAvailableCategory(categories.value, showHidden.value));
    const noCategory = computed<boolean>(() => categories.value.length < 1);
    const availableCategoryCount = computed<number>(() => getAvailableCategoryCount(categories.value, showHidden.value));
    const canAddSecondaryCategory = computed<boolean>(() => !!currentPrimaryCategory.value && primaryCategoryId.value !== '0');

    function updateCardMinHeight(): void {
        nextTick(() => {
            if (navbar.value && navbar.value.$el && navbar.value.$el.nextElementSibling) {
                const navbarHeight = getNavSideBarOuterHeight(navbar.value.$el.nextElementSibling);
                cardMinHeight.value = Math.max(navbarHeight, 680);
            }
        });
    }

    function isCategorySupportSwitch(category: TransactionCategory): boolean {
        if (!category || category.hidden) {
            return false;
        }

        return !category.parentId || category.parentId === '' || category.parentId === '0';
    }

    function switchAllPrimaryCategories(): void {
        primaryCategoryId.value = '0';
        updateCardMinHeight();
    }

    function switchPrimaryCategory(category: TransactionCategory): void {
        if (!category || category.hidden) {
            return;
        }

        if (!category.parentId || category.parentId === '' || category.parentId === '0') {
            primaryCategoryId.value = category.id;
        }

        updateCardMinHeight();
    }

    function reload(force: boolean): void {
        loading.value = true;

        transactionCategoriesStore.loadAllCategories({
            force: force
        }).then(() => {
            loading.value = false;
            displayOrderModified.value = false;

            if (force) {
                snackbar.value?.showMessage('Category list has been updated');
            }

            updateCardMinHeight();
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

    function openCreateCategoryDialog(options: {
        parentId: string;
        type: CategoryType;
        color?: TransactionCategory['color'];
        icon?: string;
    }): void {
        editDialog.value?.open({
            type: options.type,
            parentId: options.parentId,
            color: options.color,
            icon: options.icon
    }).then((result: { message?: string } | null | undefined) => {
            if (result && result.message) {
                snackbar.value?.showMessage(result.message);
            }

            updateCardMinHeight();
    }).catch((error: SnackBarError) => {
            if (error) {
                snackbar.value?.showError(error);
            }
        });
    }

    function addPrimaryCategory(): void {
        openCreateCategoryDialog({
            type: activeCategoryType.value,
            parentId: '0'
        });
    }

    function addSecondaryCategory(): void {
        if (!currentPrimaryCategory.value) {
            return;
        }

        openCreateCategoryDialog({
            type: activeCategoryType.value,
            parentId: currentPrimaryCategory.value.id,
            color: currentPrimaryCategory.value.color,
            icon: currentPrimaryCategory.value.icon
        });
    }

    function addCategoryByCurrentSelection(): void {
        if (canAddSecondaryCategory.value) {
            addSecondaryCategory();
            return;
        }

        addPrimaryCategory();
    }

    function edit(category: TransactionCategory): void {
        editDialog.value?.open({
            id: category.id,
            currentCategory: category
    }).then((result: { message?: string } | null | undefined) => {
            if (result && result.message) {
                snackbar.value?.showMessage(result.message);
            }

            if (transactionCategoriesStore.transactionCategoryListStateInvalid) {
                reload(true);
            }

            updateCardMinHeight();
    }).catch((error: SnackBarError) => {
            if (error) {
                snackbar.value?.showError(error);
            }
        });
    }

    function hide(category: TransactionCategory, hidden: boolean): void {
        updating.value = true;
        categoryHiding.value[category.id] = true;

        transactionCategoriesStore.hideCategory({
            category: category,
            hidden: hidden
        }).then(() => {
            updating.value = false;
            categoryHiding.value[category.id] = false;

            updateCardMinHeight();
        }).catch(error => {
            updating.value = false;
            categoryHiding.value[category.id] = false;

            if (!error.processed) {
                snackbar.value?.showError(error);
            }
        });
    }

    function remove(category: TransactionCategory): void {
        confirmDialog.value?.open('Are you sure you want to delete this category?').then(() => {
            updating.value = true;
            categoryRemoving.value[category.id] = true;

            transactionCategoriesStore.deleteCategory({
                category: category
            }).then(() => {
                updating.value = false;
                categoryRemoving.value[category.id] = false;

                updateCardMinHeight();
            }).catch(error => {
                updating.value = false;
                categoryRemoving.value[category.id] = false;

                if (!error.processed) {
                    snackbar.value?.showError(error);
                }
            });
        });
    }

    function saveSortResult(): void {
        if (!displayOrderModified.value) {
            return;
        }

        loading.value = true;

        transactionCategoriesStore.updateCategoryDisplayOrders({
            type: activeCategoryType.value,
            parentId: primaryCategoryId.value
        }).then(() => {
            loading.value = false;
            displayOrderModified.value = false;
        }).catch(error => {
            loading.value = false;

            if (!error.processed) {
                snackbar.value?.showError(error);
            }
        });
    }

    function onMove(event: { moved: { element: { id: string }, oldIndex: number, newIndex: number } }): void {
        if (!event || !event.moved) {
            return;
        }

        const moveEvent = event.moved;

        if (!moveEvent.element || !moveEvent.element.id) {
            snackbar.value?.showMessage('Unable to move category');
            return;
        }

        transactionCategoriesStore.changeCategoryDisplayOrder({
            categoryId: moveEvent.element.id,
            from: moveEvent.oldIndex,
            to: moveEvent.newIndex
        }).then(() => {
            displayOrderModified.value = true;
        }).catch(error => {
            snackbar.value?.showError(error);
        });
    }

    function onPresetCategorySaved(e: { message: string }): void {
        if (e && e.message) {
            snackbar.value?.showMessage(e.message);
            reload(false);
        }
    }

    watch(() => display.mdAndUp.value, (newValue) => {
        alwaysShowNav.value = newValue;

        if (!showNav.value) {
            showNav.value = newValue;
        }
    });

    reload(false);

    return {
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
        addCategoryByCurrentSelection,
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
    };
}
