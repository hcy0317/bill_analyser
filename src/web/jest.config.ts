import type { Config } from 'jest';

const coverageGateEnabled = process.env['COVERAGE_GATE'] === '1';

const config: Config = {
    preset: 'ts-jest',
    testEnvironment: 'node',
    testTimeout: 300000,
    maxWorkers: 1,
    roots: ['<rootDir>/src', '<rootDir>/../../tests/web'],
    testMatch: coverageGateEnabled ? [
        '<rootDir>/../../tests/web/views/desktop/budgets/forecastDisplay.test.ts',
        '<rootDir>/../../tests/web/views/desktop/budgets/forecastRequest.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/checkDataMatching.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/importPreview.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/llmSignalMemory.test.ts',
        '<rootDir>/../../tests/web/views/desktop/transactions/import/services.llmMemory.test.ts'
    ] : ['**/*.test.ts', '**/*.spec.ts'],
    moduleDirectories: ['node_modules', '<rootDir>/node_modules'],
    moduleNameMapper: {
        '^@/(.*)$': '<rootDir>/src/$1'
    },
    transform: {
        '^.+\\.ts$': ['ts-jest', { tsconfig: '<rootDir>/tsconfig.jest.json' }]
    },
    moduleFileExtensions: ['ts', 'js', 'json'],
    collectCoverageFrom: coverageGateEnabled ? [
        'src/views/desktop/budgets/forecastDisplay.ts',
        'src/views/desktop/budgets/forecastRequest.ts',
        'src/views/desktop/transactions/import/checkDataMatching.ts',
        'src/views/desktop/transactions/import/importPreview.ts',
        'src/views/desktop/transactions/import/llmSignalMemory.ts'
    ] : [
        'src/**/*.ts',
        '!src/**/*.d.ts'
    ],
    coverageThreshold: coverageGateEnabled ? {
        global: {
            branches: 90,
            functions: 90,
            lines: 90,
            statements: 90
        }
    } : undefined
};

export default config;
