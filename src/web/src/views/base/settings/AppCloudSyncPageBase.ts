export {
    ALL_APPLICATION_CLOUD_SETTINGS,
    type ApplicationCloudSettingItem,
    type CategorizedApplicationCloudSettingItems
} from './app-cloud-sync/cloudSettingCatalog.ts';

import { useAppCloudSyncSelection } from './app-cloud-sync/useAppCloudSyncSelection.ts';

/** 中文说明：保留应用设置云同步 shared base 的原导出入口，内部委托到功能文件夹实现。 */
export function useAppCloudSyncBase() {
    return useAppCloudSyncSelection();
}
