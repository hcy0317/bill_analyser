import { watch, type Ref } from 'vue';

import type {
    ImportTransactionCheckDataFilterMenuGroup,
    ImportTransactionDialogStep
} from './types.ts';

export interface ImportCheckDataFilterMenuOptions {
    currentStep: Ref<ImportTransactionDialogStep>;
    openedGroups: Ref<string[]>;
    showMenu: Ref<boolean>;
    getFilterMenus: () => ImportTransactionCheckDataFilterMenuGroup[];
    translate: (key: string) => string;
}

export function useImportCheckDataFilterMenu(options: ImportCheckDataFilterMenuOptions) {
    function isActiveCheckDataFilterGroup(summary?: string): boolean {
        return !!summary && summary !== options.translate('All');
    }

    function syncOpenedCheckDataFilterGroups(): void {
        const groups = options.getFilterMenus();
        if (groups.length < 1) {
            options.openedGroups.value = [];
            return;
        }

        const validTitles = new Set(groups.map(group => group.title));
        const retainedTitles = options.openedGroups.value.filter(title => validTitles.has(title));
        if (retainedTitles.length > 0) {
            options.openedGroups.value = retainedTitles;
            return;
        }

        const activeTitles = groups
            .filter(group => isActiveCheckDataFilterGroup(group.summary))
            .map(group => group.title);
        const firstGroup = groups[0];
        options.openedGroups.value = activeTitles.length > 0
            ? activeTitles
            : (firstGroup ? [firstGroup.title] : []);
    }

    watch(options.showMenu, visible => {
        if (visible) {
            syncOpenedCheckDataFilterGroups();
        }
    });

    watch(options.currentStep, step => {
        if (step !== 'checkData') {
            options.showMenu.value = false;
        }
    });

    watch(
        () => options.getFilterMenus().map(group => `${group.title}:${group.summary || ''}`).join('|'),
        () => {
            if (options.showMenu.value) {
                syncOpenedCheckDataFilterGroups();
            }
        }
    );

    return {
        isActiveCheckDataFilterGroup
    };
}
