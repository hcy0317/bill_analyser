import type { Config } from 'jest';

const config: Config = {
    preset: 'ts-jest',
    testEnvironment: 'node',
    maxWorkers: 1,
    roots: ['<rootDir>/src', '<rootDir>/../../tests/web'],
    testMatch: ['**/*.test.ts', '**/*.spec.ts'],
    moduleDirectories: ['node_modules', '<rootDir>/node_modules'],
    moduleNameMapper: {
        '^@/(.*)$': '<rootDir>/src/$1'
    },
    transform: {
        '^.+\\.ts$': ['ts-jest', { tsconfig: '<rootDir>/tsconfig.jest.json' }]
    },
    moduleFileExtensions: ['ts', 'js', 'json'],
    collectCoverageFrom: [
        'src/**/*.ts',
        '!src/**/*.d.ts'
    ]
};

export default config;
