const crypto = require('node:crypto');

const {
    compileScript,
    compileTemplate,
    parse,
    version: vueCompilerVersion
} = require('@vue/compiler-sfc');
const { createInstrumenter } = require('istanbul-lib-instrument');
const { SourceMapConsumer, SourceMapGenerator } = require('source-map-js');
const ts = require('typescript');

const TRANSFORMER_VERSION = '4';

function normalizePath(filePath) {
    return filePath.replace(/\\/g, '/');
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
            sourceMap: true
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
    return crypto.createHash('sha256')
        .update(TRANSFORMER_VERSION)
        .update(vueCompilerVersion)
        .update(sourcePath)
        .update(sourceText)
        .update(options.configString)
        .update(options.instrument ? 'instrument' : 'plain')
        .digest('hex');
}

module.exports = {
    canInstrument: true,
    getCacheKey,
    process: processSfc
};
