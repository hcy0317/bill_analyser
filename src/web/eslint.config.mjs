import pluginVue from 'eslint-plugin-vue';
import vueTsEslintConfig from '@vue/eslint-config-typescript';

export default [
    ...pluginVue.configs['flat/essential'],
    ...vueTsEslintConfig(),
    {
        ignores: [
            'dist/**',
            '**/*.{js,jsx,cjs,mjs}'
        ]
    },
    {
        files: [
            '**/*.{vue,ts,tsx,mts,js,jsx,cjs,mjs}'
        ],
        rules: {
            // Keep whole-repo ESLint fast and usable for this legacy Vue app.
            // Type correctness is enforced by `vue-tsc --noEmit`; broad DTO/API
            // `any` usage is still reported as warning debt instead of blocking
            // the repository-wide lint command.
            '@typescript-eslint/no-explicit-any': 'warn',
            '@typescript-eslint/no-unused-vars': ['error', {
                argsIgnorePattern: '^_',
                varsIgnorePattern: '^_',
                caughtErrors: 'all',
                caughtErrorsIgnorePattern: '^_',
                ignoreRestSiblings: true,
            }],
            'vue/valid-v-slot': ['error', {
                allowModifiers: true
            }]
        }
    },
];
