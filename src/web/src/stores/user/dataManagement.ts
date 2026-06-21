import { KnownFileType } from '@/core/file.ts';
import type {
    ExportTransactionDataRequest,
    DataStatisticsResponse
} from '@/models/data_management.ts';

import logger from '@/lib/logger.ts';
import services from '@/lib/services.ts';

/** 中文说明：把后端新旧统计字段归一成字符串计数，缺失或异常值统一显示为 0。 */
function normalizeStatisticsCount(result: Record<string, string | number | undefined>, primaryKey: string, fallbackKey?: string): string {
    const rawValue = result[primaryKey] ?? (fallbackKey ? result[fallbackKey] : undefined);

    if (typeof rawValue === 'number' && Number.isFinite(rawValue)) {
        return String(rawValue);
    }

    if (typeof rawValue === 'string' && rawValue.trim()) {
        return rawValue.trim();
    }

    return '0';
}

/** 中文说明：创建用户数据统计与导出动作，统一处理统计字段兼容和导出 content-type 校验。 */
export function createUserDataManagementActions() {
    function getUserDataStatistics(): Promise<DataStatisticsResponse> {
        return new Promise((resolve, reject) => {
            services.getUserDataStatistics().then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to retrieve user statistics data' });
                    return;
                }

                const result = data.result as unknown as Record<string, string | number | undefined>;

                resolve({
                    totalTransactionCount: normalizeStatisticsCount(result, 'totalTransactionCount', 'billCount'),
                    totalAccountCount: normalizeStatisticsCount(result, 'totalAccountCount', 'accountCount'),
                    totalTransactionCategoryCount: normalizeStatisticsCount(result, 'totalTransactionCategoryCount', 'categoryCount'),
                    totalTransactionTagCount: normalizeStatisticsCount(result, 'totalTransactionTagCount', 'tagCount'),
                    totalTransactionPictureCount: normalizeStatisticsCount(result, 'totalTransactionPictureCount', 'pictureCount'),
                    totalTransactionTemplateCount: normalizeStatisticsCount(result, 'totalTransactionTemplateCount', 'templateCount'),
                    totalScheduledTransactionCount: normalizeStatisticsCount(result, 'totalScheduledTransactionCount', 'scheduledTransactionCount')
                });
            }).catch(error => {
                logger.error('failed to retrieve user statistics data', error);

                if (error.response && error.response.data && error.response.data.message) {
                    reject({ error: error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve user statistics data' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function getExportedUserData(fileType: string, req?: ExportTransactionDataRequest): Promise<Blob> {
        return new Promise((resolve, reject) => {
            services.getExportedUserData(fileType, req).then(response => {
                if (response && response.headers) {
                    const contentType = response.headers['content-type']?.toString() || '';

                    if (fileType === 'csv' && !KnownFileType.CSV.isSameType(contentType)) {
                        reject({ message: 'Unable to retrieve exported user data' });
                        return;
                    } else if (fileType === 'tsv' && !KnownFileType.TSV.isSameType(contentType)) {
                        reject({ message: 'Unable to retrieve exported user data' });
                        return;
                    }
                }

                const blob = new Blob([response.data], { type: response.headers['content-type'] });
                resolve(blob);
            }).catch(error => {
                logger.error('failed to retrieve user statistics data', error);

                if (error.response && KnownFileType.TXT.isSameType(error.response.headers['content-type']) && error.response && error.response.data) {
                    reject({ message: 'error.' + error.response.data });
                } else if (!error.processed) {
                    reject({ message: 'Unable to retrieve exported user data' });
                } else {
                    reject(error);
                }
            });
        });
    }

    return {
        getUserDataStatistics,
        getExportedUserData
    };
}
