import { ref, computed } from 'vue';

import { useI18n } from '@/locales/helpers.ts';

import { useUserStore } from '@/stores/user.ts';

import type { DataStatisticsResponse, DisplayDataStatistics } from '@/models/data_management.ts';

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
        // 函数
        getExportFileName
    }
}
