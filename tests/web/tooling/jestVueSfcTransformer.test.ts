import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

interface VueTransformerResult {
    code: string;
    map: object;
}

interface VueTransformer {
    getCacheKey(
        sourceText: string,
        sourcePath: string,
        options: { configString: string; instrument: boolean }
    ): string;
    process(
        sourceText: string,
        sourcePath: string,
        options: { configString: string; instrument: boolean }
    ): VueTransformerResult;
}

const webRoot = path.resolve(__dirname, '../../../src/web');
const transformer = jest.requireActual<VueTransformer>(path.join(
    webRoot,
    'scripts/jest-vue-sfc-transformer.cjs'
));
const nodeFs = jest.requireActual<typeof fs>('node:fs');
const productionSourceRoot = path.join(webRoot, 'src');

const temporaryDirectories: string[] = [];

function createTemporaryDirectory(parent: string, prefix: string): string {
    const directory = fs.mkdtempSync(path.join(parent, prefix));
    temporaryDirectories.push(directory);
    return directory;
}

function cacheKey(sourceText: string, sourcePath: string): string {
    return transformer.getCacheKey(sourceText, sourcePath, {
        configString: '{}',
        instrument: false
    });
}

function createRecursiveAliasFixture() {
    const fixtureDirectory = createTemporaryDirectory(
        webRoot,
        '.jest-vue-transformer-cache-'
    );
    const sourcePath = path.join(fixtureDirectory, 'Component.vue');
    const dependencyPath = path.join(fixtureDirectory, 'types.ts');
    const nestedDependencyPath = path.join(fixtureDirectory, 'nested.ts');
    const aliasPath = `@/${path.relative(
        productionSourceRoot,
        dependencyPath
    ).replace(/\\/g, '/')}`.replace(/\.ts$/, '');
    const sourceText = [
        '<script setup lang="ts">',
        `import type { FixtureValue } from '${aliasPath}';`,
        'const value: FixtureValue = { nested: { label: \'fixture\' } };',
        '</script>',
        '<template><span>{{ value.nested.label }}</span></template>'
    ].join('\n');

    fs.writeFileSync(
        dependencyPath,
        "import type { NestedValue } from './nested';\n" +
            'export interface FixtureValue { nested: NestedValue; }\n'
    );
    fs.writeFileSync(
        nestedDependencyPath,
        'export interface NestedValue { label: string; }\n'
    );

    return {
        fixtureDirectory,
        nestedDependencyPath,
        sourcePath,
        sourceText
    };
}

function collectProductionSourceFiles(directory: string): string[] {
    return fs.readdirSync(directory, { withFileTypes: true })
        .flatMap(entry => {
            const absolutePath = path.join(directory, entry.name);
            return entry.isDirectory()
                ? collectProductionSourceFiles(absolutePath)
                : [absolutePath];
        })
        .filter(filePath => ['.ts', '.vue'].includes(path.extname(filePath)));
}

afterEach(() => {
    jest.restoreAllMocks();
    while (temporaryDirectories.length > 0) {
        const directory = temporaryDirectories.pop();
        if (directory) {
            fs.rmSync(directory, { force: true, recursive: true });
        }
    }
});

const importedTypeSfcPaths = [
    'src/components/desktop/NumberInput.vue',
    'src/components/charts/PieChart.vue',
    'src/components/desktop/ScheduleFrequencySelect.vue',
    'src/components/desktop/TwoColumnSelect.vue',
    'src/components/desktop/TrendsChart.vue',
    'src/views/desktop/budgets/components/BudgetHistoryLegend.vue'
];

describe.each(importedTypeSfcPaths)('Vue SFC transformer: %s', relativePath => {
    it.each([false, true])(
        'compiles production SFCs with imported types (instrument=%s)',
        instrument => {
            const sourcePath = path.join(webRoot, relativePath);
            const sourceText = fs.readFileSync(sourcePath, 'utf8');

            const transformed = transformer.process(sourceText, sourcePath, {
                configString: '{}',
                instrument
            });

            expect(transformed.code).toContain('exports.default');
            expect(transformed.map).toBeDefined();
        }
    );
});

describe('Vue SFC transformer cache key', () => {
    it('changes when a directly imported local type changes', () => {
        const fixtureDirectory = createTemporaryDirectory(
            webRoot,
            '.jest-vue-transformer-cache-'
        );
        const sourcePath = path.join(fixtureDirectory, 'Component.vue');
        const dependencyPath = path.join(fixtureDirectory, 'types.ts');
        const sourceText = [
            '<script lang="ts">',
            "import type { FixtureValue } from './types';",
            'const value: FixtureValue = { label: \'fixture\' };',
            'export default { data: () => ({ value }) };',
            '</script>',
            '<template><span>{{ value.label }}</span></template>'
        ].join('\n');

        fs.writeFileSync(
            dependencyPath,
            'export interface FixtureValue { label: string; }\n'
        );
        const initialKey = cacheKey(sourceText, sourcePath);

        fs.writeFileSync(
            dependencyPath,
            'export interface FixtureValue { label: string; enabled: boolean; }\n'
        );

        expect(cacheKey(sourceText, sourcePath)).not.toBe(initialKey);
    });

    it('changes when a recursive dependency reached through the source alias changes', () => {
        const {
            nestedDependencyPath,
            sourcePath,
            sourceText
        } = createRecursiveAliasFixture();
        const initialKey = cacheKey(sourceText, sourcePath);

        fs.writeFileSync(
            nestedDependencyPath,
            'export interface NestedValue { label: string; count: number; }\n'
        );

        expect(cacheKey(sourceText, sourcePath)).not.toBe(initialKey);
    });

    it('does not invalidate files captured by a concurrent production source scan', () => {
        const {
            fixtureDirectory,
            sourcePath,
            sourceText
        } = createRecursiveAliasFixture();
        cacheKey(sourceText, sourcePath);
        const capturedSourceFiles = collectProductionSourceFiles(
            productionSourceRoot
        );

        fs.rmSync(fixtureDirectory, { force: true, recursive: true });

        expect(() => {
            for (const capturedSourceFile of capturedSourceFiles) {
                fs.readFileSync(capturedSourceFile, 'utf8');
            }
        }).not.toThrow();
    });

    it('does not read dependencies outside the web root or from packages', () => {
        const fixtureDirectory = createTemporaryDirectory(
            webRoot,
            '.jest-vue-transformer-cache-'
        );
        const outsideDirectory = createTemporaryDirectory(
            os.tmpdir(),
            'bill-analyser-jest-vue-transformer-'
        );
        const sourcePath = path.join(fixtureDirectory, 'Component.vue');
        const outsideDependencyPath = path.join(outsideDirectory, 'outside.ts');
        const packageDependencyPath = path.join(
            fixtureDirectory,
            'node_modules/cache-key-fixture/index.d.ts'
        );
        const outsideSpecifier = path.relative(
            fixtureDirectory,
            outsideDependencyPath
        ).replace(/\\/g, '/');
        const sourceText = [
            '<script setup lang="ts">',
            `import type { OutsideValue } from '${outsideSpecifier}';`,
            "import type { PackageValue } from 'cache-key-fixture';",
            'const value = {} as OutsideValue & PackageValue;',
            '</script>',
            '<template><span>{{ value }}</span></template>'
        ].join('\n');

        fs.mkdirSync(path.dirname(packageDependencyPath), { recursive: true });
        fs.writeFileSync(
            outsideDependencyPath,
            'export interface OutsideValue { first: string; }\n'
        );
        fs.writeFileSync(
            packageDependencyPath,
            'export interface PackageValue { first: string; }\n'
        );
        const readFileSpy = jest.spyOn(nodeFs, 'readFileSync');
        const initialKey = cacheKey(sourceText, sourcePath);

        fs.writeFileSync(
            outsideDependencyPath,
            'export interface OutsideValue { second: number; }\n'
        );
        fs.writeFileSync(
            packageDependencyPath,
            'export interface PackageValue { second: number; }\n'
        );
        const nextKey = cacheKey(sourceText, sourcePath);
        const readPaths = readFileSpy.mock.calls.map(([filePath]) => (
            fs.realpathSync(String(filePath))
        ));

        expect(nextKey).toBe(initialKey);
        expect(readPaths).not.toContain(fs.realpathSync(outsideDependencyPath));
        expect(readPaths).not.toContain(fs.realpathSync(packageDependencyPath));
    });
});
