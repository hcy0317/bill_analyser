const crypto = require('node:crypto');
const nodeFs = require('node:fs');
const path = require('node:path');

const {
    compileScript,
    compileTemplate,
    parse,
    registerTS,
    version: vueCompilerVersion
} = require('@vue/compiler-sfc');
const { createInstrumenter } = require('istanbul-lib-instrument');
const { SourceMapConsumer, SourceMapGenerator } = require('source-map-js');
const ts = require('typescript');

const TRANSFORMER_VERSION = '6';
const WEB_ROOT = path.resolve(__dirname, '..');
const WEB_SOURCE_ROOT = path.join(WEB_ROOT, 'src');
const TYPESCRIPT_EXTENSIONS = new Set([
    ts.Extension.Ts,
    ts.Extension.Tsx,
    ts.Extension.Dts,
    ts.Extension.Mts,
    ts.Extension.Dmts,
    ts.Extension.Cts,
    ts.Extension.Dcts
].filter(Boolean));
const CACHE_MODULE_RESOLUTION_OPTIONS = Object.freeze({
    baseUrl: WEB_ROOT,
    moduleResolution: ts.ModuleResolutionKind.Node10,
    paths: {
        '@/*': ['src/*']
    }
});

registerTS(() => ts);

function normalizePath(filePath) {
    return filePath.replace(/\\/g, '/');
}

function resolveCompilerFilePath(filePath) {
    const normalizedFilePath = normalizePath(filePath);
    if (normalizedFilePath.startsWith('@/')) {
        return path.resolve(WEB_SOURCE_ROOT, normalizedFilePath.slice(2));
    }

    if (path.isAbsolute(filePath)) {
        return path.resolve(filePath);
    }

    return path.resolve(WEB_ROOT, filePath);
}

function isWithinWebRoot(filePath) {
    const relativePath = path.relative(WEB_ROOT, filePath);
    return relativePath === '' || (
        relativePath !== '..' &&
        !relativePath.startsWith(`..${path.sep}`) &&
        !path.isAbsolute(relativePath)
    );
}

function resolveReadableFile(filePath) {
    const resolvedPath = resolveCompilerFilePath(filePath);
    if (!isWithinWebRoot(resolvedPath) || !nodeFs.existsSync(resolvedPath)) {
        return undefined;
    }

    try {
        const realPath = nodeFs.realpathSync.native(resolvedPath);
        return isWithinWebRoot(realPath) ? realPath : undefined;
    } catch {
        return undefined;
    }
}

const compilerFileSystem = Object.freeze({
    directoryExists(filePath) {
        const readablePath = resolveReadableFile(filePath);
        if (!readablePath) {
            return false;
        }

        try {
            return nodeFs.statSync(readablePath).isDirectory();
        } catch {
            return false;
        }
    },
    fileExists(filePath) {
        const readablePath = resolveReadableFile(filePath);
        if (!readablePath) {
            return false;
        }

        try {
            return nodeFs.statSync(readablePath).isFile();
        } catch {
            return false;
        }
    },
    readFile(filePath) {
        const readablePath = resolveReadableFile(filePath);
        if (!readablePath) {
            return undefined;
        }

        try {
            return nodeFs.readFileSync(readablePath, 'utf8');
        } catch {
            return undefined;
        }
    },
    realpath(filePath) {
        return resolveReadableFile(filePath) ?? resolveCompilerFilePath(filePath);
    },
    getCurrentDirectory() {
        return WEB_ROOT;
    },
    useCaseSensitiveFileNames: ts.sys.useCaseSensitiveFileNames
});

function isLocalModuleSpecifier(moduleSpecifier) {
    return moduleSpecifier === '.' ||
        moduleSpecifier === '..' ||
        moduleSpecifier.startsWith('./') ||
        moduleSpecifier.startsWith('../') ||
        moduleSpecifier.startsWith('@/');
}

function findTypeScriptModuleSpecifiers(sourceText) {
    return ts.preProcessFile(sourceText, true, true).importedFiles
        .map(importedFile => importedFile.fileName)
        .filter(isLocalModuleSpecifier);
}

function findSfcTypeScriptModuleSpecifiers(sourceText, sourcePath) {
    const { descriptor } = parse(sourceText, {
        filename: normalizePath(sourcePath),
        sourceMap: false
    });
    const scriptBlocks = [descriptor.script, descriptor.scriptSetup]
        .filter(Boolean);
    const moduleSpecifiers = scriptBlocks.flatMap(scriptBlock => [
        ...(scriptBlock.src && isLocalModuleSpecifier(scriptBlock.src)
            ? [scriptBlock.src]
            : []),
        ...findTypeScriptModuleSpecifiers(scriptBlock.content)
    ]);

    return [...new Set(moduleSpecifiers)].sort();
}

function resolveTypeScriptDependency(moduleSpecifier, containingFile) {
    const resolution = ts.resolveModuleName(
        moduleSpecifier,
        containingFile,
        CACHE_MODULE_RESOLUTION_OPTIONS,
        compilerFileSystem
    ).resolvedModule;
    if (!resolution || !TYPESCRIPT_EXTENSIONS.has(resolution.extension)) {
        return undefined;
    }

    return resolveReadableFile(resolution.resolvedFileName);
}

function collectTypeScriptDependencies(sourceText, sourcePath) {
    const resolvedSourcePath = resolveCompilerFilePath(sourcePath);
    if (!isWithinWebRoot(resolvedSourcePath)) {
        return [];
    }

    const dependencies = new Map();
    const visitedPaths = new Set();
    const visitModuleSpecifiers = (moduleSpecifiers, containingFile) => {
        for (const moduleSpecifier of moduleSpecifiers) {
            const dependencyPath = resolveTypeScriptDependency(
                moduleSpecifier,
                containingFile
            );
            if (!dependencyPath) {
                continue;
            }

            const normalizedDependencyPath = normalizePath(dependencyPath);
            const visitedPath = ts.sys.useCaseSensitiveFileNames
                ? normalizedDependencyPath
                : normalizedDependencyPath.toLowerCase();
            if (visitedPaths.has(visitedPath)) {
                continue;
            }
            visitedPaths.add(visitedPath);

            const dependencySource = compilerFileSystem.readFile(dependencyPath);
            if (dependencySource === undefined) {
                continue;
            }
            dependencies.set(
                normalizePath(path.relative(WEB_ROOT, dependencyPath)),
                dependencySource
            );
            visitModuleSpecifiers(
                findTypeScriptModuleSpecifiers(dependencySource),
                dependencyPath
            );
        }
    };

    visitModuleSpecifiers(
        findSfcTypeScriptModuleSpecifiers(sourceText, resolvedSourcePath),
        resolvedSourcePath
    );

    return [...dependencies.entries()].sort(([leftPath], [rightPath]) => (
        leftPath.localeCompare(rightPath)
    ));
}

function lineBreakCount(value) {
    return (value.match(/\n/g) ?? []).length;
}

function formatCompilerErrors(sourcePath, errors) {
    const details = errors.map(error => {
        if (typeof error === 'string') {
            return error;
        }

        return error.message;
    }).join('\n');

    return new Error(`Unable to compile Vue SFC ${sourcePath}:\n${details}`);
}

function addMappings(generator, rawMap, generatedLineOffset, sourcePath) {
    if (!rawMap) {
        return;
    }

    const consumer = new SourceMapConsumer(rawMap);
    consumer.eachMapping(mapping => {
        if (mapping.originalLine == null || mapping.originalColumn == null) {
            return;
        }

        generator.addMapping({
            generated: {
                line: mapping.generatedLine + generatedLineOffset,
                column: mapping.generatedColumn
            },
            original: {
                line: mapping.originalLine,
                column: mapping.originalColumn
            },
            source: sourcePath,
            name: mapping.name ?? undefined
        });
    });
}

function buildSfcModule(sourceText, sourcePath) {
    const normalizedSourcePath = normalizePath(sourcePath);
    const { descriptor, errors } = parse(sourceText, {
        filename: normalizedSourcePath,
        sourceMap: true
    });

    if (errors.length > 0) {
        throw formatCompilerErrors(normalizedSourcePath, errors);
    }

    const scopeHash = crypto.createHash('sha256')
        .update(normalizedSourcePath)
        .digest('hex')
        .slice(0, 8);
    const scopeId = `data-v-${scopeHash}`;
    const sourceMap = new SourceMapGenerator({ file: normalizedSourcePath });
    const isTypeScript = [descriptor.script?.lang, descriptor.scriptSetup?.lang]
        .some(lang => lang === 'ts' || lang === 'tsx');
    let generatedCode = '';

    const appendCompiledBlock = (code, map) => {
        const lineOffset = lineBreakCount(generatedCode);
        addMappings(sourceMap, map, lineOffset, normalizedSourcePath);
        generatedCode += code;
        if (!generatedCode.endsWith('\n')) {
            generatedCode += '\n';
        }
    };

    let scriptBindings;
    if (descriptor.script || descriptor.scriptSetup) {
        const script = compileScript(descriptor, {
            id: scopeHash,
            genDefaultAs: '__sfc__',
            sourceMap: true,
            fs: compilerFileSystem
        });
        scriptBindings = script.bindings;
        appendCompiledBlock(script.content, script.map);
    } else {
        generatedCode += 'const __sfc__ = {};\n';
    }

    if (descriptor.template) {
        const template = compileTemplate({
            source: descriptor.template.content,
            filename: normalizedSourcePath,
            id: scopeHash,
            scoped: descriptor.styles.some(style => style.scoped),
            inMap: descriptor.template.map,
            compilerOptions: {
                bindingMetadata: scriptBindings,
                isTS: isTypeScript
            }
        });

        if (template.errors.length > 0) {
            throw formatCompilerErrors(normalizedSourcePath, template.errors);
        }

        appendCompiledBlock(template.code, template.map);
        generatedCode += '__sfc__.render = render;\n';
    }

    if (descriptor.styles.some(style => style.scoped)) {
        generatedCode += `__sfc__.__scopeId = ${JSON.stringify(scopeId)};\n`;
    }

    generatedCode += 'export default __sfc__;\n';
    sourceMap.setSourceContent(normalizedSourcePath, sourceText);

    return {
        code: generatedCode,
        map: sourceMap.toJSON()
    };
}

function composeSourceMaps(transpiledMap, sfcMap, sourcePath, sourceText) {
    const normalizedSourcePath = normalizePath(sourcePath);
    const transpiledConsumer = new SourceMapConsumer(transpiledMap);
    const sfcConsumer = new SourceMapConsumer(sfcMap);
    const composedMap = new SourceMapGenerator({ file: normalizedSourcePath });

    transpiledConsumer.eachMapping(mapping => {
        if (mapping.originalLine == null || mapping.originalColumn == null) {
            return;
        }

        const original = sfcConsumer.originalPositionFor({
            line: mapping.originalLine,
            column: mapping.originalColumn,
            bias: SourceMapConsumer.GREATEST_LOWER_BOUND
        });
        if (original.line == null || original.column == null) {
            return;
        }

        composedMap.addMapping({
            generated: {
                line: mapping.generatedLine,
                column: mapping.generatedColumn
            },
            original: {
                line: original.line,
                column: original.column
            },
            source: normalizedSourcePath,
            name: original.name ?? mapping.name ?? undefined
        });
    });
    composedMap.setSourceContent(normalizedSourcePath, sourceText);

    return composedMap.toJSON();
}

function processSfc(sourceText, sourcePath, options) {
    const compiled = buildSfcModule(sourceText, sourcePath);
    const transpiled = ts.transpileModule(compiled.code, {
        fileName: normalizePath(sourcePath),
        reportDiagnostics: true,
        compilerOptions: {
            esModuleInterop: true,
            inlineSources: true,
            module: ts.ModuleKind.CommonJS,
            sourceMap: true,
            target: ts.ScriptTarget.ES2020
        }
    });
    const errors = (transpiled.diagnostics ?? []).filter(diagnostic => (
        diagnostic.category === ts.DiagnosticCategory.Error
    ));
    if (errors.length > 0) {
        const host = {
            getCanonicalFileName: fileName => fileName,
            getCurrentDirectory: () => globalThis.process.cwd(),
            getNewLine: () => '\n'
        };
        throw new Error(ts.formatDiagnostics(errors, host));
    }

    const transpiledMap = JSON.parse(transpiled.sourceMapText);
    const code = transpiled.outputText.replace(/\n?\/\/# sourceMappingURL=.*\s*$/, '');

    const transformed = {
        code,
        map: composeSourceMaps(
            transpiledMap,
            compiled.map,
            sourcePath,
            sourceText
        )
    };
    if (!options.instrument) {
        return transformed;
    }

    const instrumenter = createInstrumenter({
        compact: false,
        coverageGlobalScope: 'globalThis',
        coverageGlobalScopeFunc: false,
        produceSourceMap: true
    });
    const instrumentedCode = instrumenter.instrumentSync(
        transformed.code,
        normalizePath(sourcePath),
        transformed.map
    );

    return {
        code: instrumentedCode,
        map: instrumenter.lastSourceMap()
    };
}

function getCacheKey(sourceText, sourcePath, options) {
    const hash = crypto.createHash('sha256')
        .update(TRANSFORMER_VERSION)
        .update(vueCompilerVersion)
        .update(sourcePath)
        .update(sourceText)
        .update(options.configString)
        .update(options.instrument ? 'instrument' : 'plain');
    for (const [dependencyPath, dependencySource] of collectTypeScriptDependencies(
        sourceText,
        sourcePath
    )) {
        hash.update('\0dependency\0')
            .update(dependencyPath)
            .update('\0')
            .update(dependencySource);
    }

    return hash.digest('hex');
}

module.exports = {
    canInstrument: true,
    getCacheKey,
    process: processSfc
};
