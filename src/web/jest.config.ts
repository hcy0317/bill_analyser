import type { Config } from 'jest';

const coverageGateEnabled = process.env['COVERAGE_GATE'] === '1';

const config: Config = {
    preset: 'ts-jest',
    testEnvironment: 'node',
    testTimeout: 300000,
    maxWorkers: 1,
    roots: ['<rootDir>/src', '<rootDir>/../../tests/web'],
    testMatch: ['**/*.test.ts', '**/*.spec.ts'],
    setupFiles: ['<rootDir>/../../tests/web/setup/browserGlobals.ts'],
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
    forceCoverageMatch: [],
    collectCoverageFrom: coverageGateEnabled ? [
        'src/**/*.{ts,vue}',
        '!src/**/*.d.ts',
        '!src/{desktop-main,mobile-main,index-main}.ts',
        '!src/contracts/**/*.generated.ts'
    ] : [],
    coverageThreshold: coverageGateEnabled ? {
        global: {
            branches: 91,
            functions: 91,
            lines: 91,
            statements: 91
        }
    } : undefined
};

export default config;
