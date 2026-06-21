import { KnownFileType } from '@/core/file.ts';
import type {
    SettingsBundleImportResult,
    SettingsBundleExportAuth,
    SettingsBundleSectionKey
} from '@/models/data_management.ts';

import logger from '@/lib/logger.ts';
import services from '@/lib/services.ts';

export function createUserSettingsBundleActions() {
    function getExportedSettingsBundle(): Promise<Blob> {
        return new Promise((resolve, reject) => {
            services.getExportedSettingsBundle().then(response => {
                const contentType = response.headers['content-type']?.toString() || KnownFileType.JSON.contentType;
                const blob = new Blob([response.data], { type: contentType });
                resolve(blob);
            }).catch(error => {
                logger.error('failed to retrieve exported settings bundle', error);

                if (!error.processed) {
                    reject({ message: 'Unable to retrieve exported settings bundle' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function getExportedSettingsBundleSection(
        sectionKey: SettingsBundleSectionKey,
        auth?: SettingsBundleExportAuth
    ): Promise<Blob> {
        return new Promise((resolve, reject) => {
            services.getExportedSettingsBundleSection(sectionKey, auth).then(response => {
                const contentType = response.headers['content-type']?.toString() || KnownFileType.JSON.contentType;
                const blob = new Blob([response.data], { type: contentType });
                resolve(blob);
            }).catch(error => {
                logger.error('failed to retrieve exported settings section', error);

                if (!error.processed) {
                    reject({ message: 'Unable to retrieve exported settings bundle' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function previewImportSettingsBundle(bundle: unknown): Promise<SettingsBundleImportResult> {
        return new Promise((resolve, reject) => {
            services.previewImportSettingsBundle(bundle).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to preview settings bundle import' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to preview settings bundle import', error);

                if (!error.processed) {
                    reject({ message: 'Unable to preview settings bundle import' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function previewImportSettingsBundleSection(
        sectionKey: SettingsBundleSectionKey,
        bundle: unknown
    ): Promise<SettingsBundleImportResult> {
        return new Promise((resolve, reject) => {
            services.previewImportSettingsBundleSection(sectionKey, bundle).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to preview settings bundle import' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to preview settings section import', error);

                if (!error.processed) {
                    reject({ message: 'Unable to preview settings bundle import' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function importSettingsBundle(bundle: unknown): Promise<SettingsBundleImportResult> {
        return new Promise((resolve, reject) => {
            services.importSettingsBundle(bundle).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to import settings bundle' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to import settings bundle', error);

                if (!error.processed) {
                    reject({ message: 'Unable to import settings bundle' });
                } else {
                    reject(error);
                }
            });
        });
    }

    function importSettingsBundleSection(
        sectionKey: SettingsBundleSectionKey,
        bundle: unknown
    ): Promise<SettingsBundleImportResult> {
        return new Promise((resolve, reject) => {
            services.importSettingsBundleSection(sectionKey, bundle).then(response => {
                const data = response.data;

                if (!data || !data.success || !data.result) {
                    reject({ message: 'Unable to import settings bundle' });
                    return;
                }

                resolve(data.result);
            }).catch(error => {
                logger.error('failed to import settings section', error);

                if (!error.processed) {
                    reject({ message: 'Unable to import settings bundle' });
                } else {
                    reject(error);
                }
            });
        });
    }

    return {
        getExportedSettingsBundle,
        getExportedSettingsBundleSection,
        previewImportSettingsBundle,
        previewImportSettingsBundleSection,
        importSettingsBundle,
        importSettingsBundleSection
    };
}
