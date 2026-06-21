import { ref, computed } from 'vue';

import { useI18n } from '@/locales/helpers.ts';

import { useUserStore } from '@/stores/user.ts';

import {
    SETTINGS_BUNDLE_DATA_MANAGEMENT_ENTRIES,
    type DataStatisticsResponse,
    type DisplayDataStatistics,
    type DisplaySettingsBundleDataManagementEntry,
} from '@/models/data_management.ts';

/** 中文说明：提供桌面/移动用户数据管理页共享统计、导出格式和敏感清理选项。 */
export function useDataManagementPageBase() {
    const { tt, formatNumberToLocalizedNumerals } = useI18n();

    const userStore = useUserStore();

    const dataStatistics = ref<DataStatisticsResponse | null>(null);

    const normalizeStatisticValue = (value: string | undefined): string => {
        const parsedValue = Number.parseInt(value ?? '0', 10);
        return formatNumberToLocalizedNumerals(Number.isNaN(parsedValue) ? 0 : parsedValue);
    };

    const displayDataStatistics = computed<DisplayDataStatistics | null>(() => {
        if (!dataStatistics.value) {
            return null;
        }

        return {
            totalTransactionCount: normalizeStatisticValue(dataStatistics.value.totalTransactionCount),
            totalAccountCount: normalizeStatisticValue(dataStatistics.value.totalAccountCount),
            totalTransactionCategoryCount: normalizeStatisticValue(dataStatistics.value.totalTransactionCategoryCount),
            totalTransactionTagCount: normalizeStatisticValue(dataStatistics.value.totalTransactionTagCount),
            totalTransactionPictureCount: normalizeStatisticValue(dataStatistics.value.totalTransactionPictureCount),
            totalTransactionTemplateCount: normalizeStatisticValue(dataStatistics.value.totalTransactionTemplateCount),
            totalScheduledTransactionCount: normalizeStatisticValue(dataStatistics.value.totalScheduledTransactionCount)
        };
    });

    const settingsBundleDataManagementEntries = computed<DisplaySettingsBundleDataManagementEntry[]>(() => (
        SETTINGS_BUNDLE_DATA_MANAGEMENT_ENTRIES.map(entry => ({
            ...entry,
            title: tt(entry.titleKey),
            description: tt(entry.descriptionKey),
        }))
    ));

    function getExportFileName(fileExtension: string): string {
        const nickname = userStore.currentUserNickname;

        if (nickname) {
            return tt('dataExport.exportFilename', {
                nickname: nickname
            }) + '.' + fileExtension;
        }

        return tt('dataExport.defaultExportFilename') + '.' + fileExtension;
    }

    return {
        // 状态
        dataStatistics,
        // 计算状态
        displayDataStatistics,
        settingsBundleDataManagementEntries,
        // 函数
        getExportFileName
    }
}
