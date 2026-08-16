// 导入预览匹配信号 facade：保留旧导入路径，具体实现按功能拆到 check-data-matching 子域。
// investment: null, 由 signalViewModel 固定保持投资识别不进入可操作信号 UI。
export * from './check-data-matching/types.ts';
export * from './check-data-matching/context.ts';
export * from './check-data-matching/historyRewrite.ts';
export * from './check-data-matching/signalFilter.ts';
export * from './check-data-matching/signalStatus.ts';
export * from './check-data-matching/signalViewModel.ts';
export * from './check-data-matching/previewStateAdapter.ts';
