import type { Config } from 'jest';

const coverageGateEnabled = process.env['COVERAGE_GATE'] === '1';
const changedSfcCoverageEnabled = process.env['CHANGED_SFC_COVERAGE'] === '1';

const config: Config = {
    preset: 'ts-jest',
    testEnvironment: 'node',
    testTimeout: 300000,
    maxWorkers: 1,
    roots: ['<rootDir>/src', '<rootDir>/../../tests/web'],
    testMatch: changedSfcCoverageEnabled ? [
        '<rootDir>/../../tests/web/views/desktop/transactions/import/changedVueSfcCoverage.test.ts',
        '<rootDir>/../../tests/web/views/mobile/categoryListPageInteraction.test.ts'
    ] : coverageGateEnabled ? [
        '<rootDir>/../../tests/web/views/desktop/budgets/forecastDisplay.test.ts',
        '<rootDir>/../../tests/web/views/desktop/budgets/forecastRequest.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/checkDataMatching.test.ts',
        '<rootDir>/../../tests/web/models/imported_transaction.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importLifecycleSfc.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importPreviewUpdates.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importTransactionCheckDataActions.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importTransactionCheckDataServerDraftRehydrate.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importTransactionCheckDataSignalAdapter.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importTransactionCheckDataSignalHistoryMatrix.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importTransactionCheckDataSelectionAndPaging.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importTransactionCheckDataTemplateAndDisplayMatrix.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importPreview.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importPreviewIndex.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importPreviewTransaction.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importSignalSystem.red.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/llmSignalMemory.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/services.llmMemory.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/services.matchingCandidate.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/actionScope.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/decisionPreviewReplacement.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/decisionPreviewReplacementBridge.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/previewPageQuery.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/previewPageRequestCoordinator.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/selectionActionCoordinator.test.ts'
    ] : ['**/*.test.ts', '**/*.spec.ts'],
    moduleDirectories: ['node_modules', '<rootDir>/node_modules'],
    moduleNameMapper: {
        '^@/(.*)$': '<rootDir>/src/$1'
    },
    transform: {
        '^.+\\.vue$': '<rootDir>/scripts/jest-vue-sfc-transformer.cjs',
        '^.+\\.ts$': ['ts-jest', { tsconfig: '<rootDir>/tsconfig.jest.json' }]
    },
    moduleFileExtensions: ['vue', 'ts', 'js', 'json'],
    coverageReporters: ['lcov', 'json', 'text'],
    forceCoverageMatch: changedSfcCoverageEnabled ? [
        '**/src/views/desktop/transactions/import/ImportDialog.vue',
        '**/src/views/desktop/categories/ListPage.vue',
        '**/src/views/mobile/categories/ListPage.vue'
    ] : coverageGateEnabled ? [
        '**/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue',
        '**/src/views/mobile/transactions/ImportPreviewPage.vue'
    ] : ['**/tests/web/fixtures/VueCoverageHarness.vue'],
    collectCoverageFrom: changedSfcCoverageEnabled ? [
        'src/views/desktop/transactions/import/ImportDialog.vue',
        'src/views/desktop/categories/ListPage.vue',
        'src/views/mobile/categories/ListPage.vue'
    ] : coverageGateEnabled ? [
        'src/views/desktop/budgets/forecastDisplay.ts',
        'src/views/desktop/budgets/forecastRequest.ts',
        'src/models/import_matching.ts',
        'src/models/imported_transaction.ts',
        'src/models/imported_transaction/matching.ts',
        'src/views/desktop/transactions/import/check-data-matching/signalViewModel.ts',
        'src/views/desktop/transactions/import/check-data-matching/types.ts',
        'src/views/desktop/transactions/import/import-preview-index/mapping.ts',
        'src/views/desktop/transactions/import/import-preview-index/types.ts',
        'src/views/desktop/transactions/import/checkDataMatching.ts',
        'src/views/desktop/transactions/import/importPreview.ts',
        'src/views/desktop/transactions/import/importPreviewUpdates.ts',
        'src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue',
        'src/views/mobile/transactions/ImportPreviewPage.vue',
        'src/views/desktop/transactions/import/llmSignalMemory.ts',
        'src/views/desktop/transactions/import/actionScope.ts',
        'src/views/desktop/transactions/import/decisionPreviewReplacement.ts',
        'src/views/desktop/transactions/import/DecisionPreviewReplacementBridge.ts',
        'src/views/desktop/transactions/import/import-dialog/previewPageQuery.ts',
        'src/views/desktop/transactions/import/import-dialog/previewPageRequestCoordinator.ts',
        'src/views/desktop/transactions/import/selectionActionCoordinator.ts',
        'src/views/desktop/transactions/import/check-data-matching/historyRewrite.ts',
        'src/views/desktop/transactions/import/check-data-matching/signalViewModel.ts'
    ] : [
        'src/**/*.ts',
        '!src/**/*.d.ts'
    ],
    coverageDirectory: changedSfcCoverageEnabled ? '<rootDir>/coverage/changed-sfc' : undefined,
    coverageThreshold: coverageGateEnabled && !changedSfcCoverageEnabled ? {
        global: {
            branches: 91,
            functions: 91,
            lines: 91,
            statements: 91
        }
    } : undefined
};

export default config;
