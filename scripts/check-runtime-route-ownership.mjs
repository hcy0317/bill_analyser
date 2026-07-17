import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');
const httpSourceRoot = path.join(repoRoot, 'src', 'backend', 'http');
const rootRouterSource = 'src/backend/http/router.rs';
const rootRouterFactory = 'build_router';
const ownedBusinessMethods = ['get', 'post', 'put', 'delete', 'patch'];
// CORS preflight registrations are transport plumbing, not business-route ownership records.
const intentionallyExcludedMethods = ['options'];
const graphPath = path.join(
  repoRoot,
  'src',
  'backend',
  'http',
  'live_route_graph.generated.json',
);
const ownershipPath = path.join(
  repoRoot,
  'src',
  'web',
  'src',
  'contracts',
  'rustRouteOwnership.manifest.generated.json',
);
const forbiddenLegacy = [
  routeKey('PUT', '/api/bills/import/learning-rules/{rule_id}'),
];

function routeKey(method, pattern) {
  return `${method} ${pattern}`;
}

function recordFromKey(key) {
  const separator = key.indexOf(' ');
  return {
    method: key.slice(0, separator),
    pattern: key.slice(separator + 1),
  };
}

function normalizePattern(pattern) {
  return pattern
    .replace(/\/\*([A-Za-z_][A-Za-z0-9_]*)/g, '/{*$1}')
    .replace(/:([A-Za-z_][A-Za-z0-9_]*)/g, '{$1}');
}

function productionSource(relativePath) {
  const source = fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
  const testBoundary = source.indexOf('#[cfg(test)]');
  return testBoundary === -1 ? source : source.slice(0, testBoundary);
}

function matchingDelimiter(source, openIndex, openChar, closeChar) {
  let depth = 0;
  let quote = null;
  let escaped = false;
  let lineComment = false;
  let blockComment = false;
  for (let index = openIndex; index < source.length; index += 1) {
    const char = source[index];
    const next = source[index + 1];
    if (lineComment) {
      if (char === '\n') {
        lineComment = false;
      }
      continue;
    }
    if (blockComment) {
      if (char === '*' && next === '/') {
        blockComment = false;
        index += 1;
      }
      continue;
    }
    if (quote !== null) {
      if (escaped) {
        escaped = false;
      } else if (char === '\\') {
        escaped = true;
      } else if (char === quote) {
        quote = null;
      }
      continue;
    }
    if (char === '/' && next === '/') {
      lineComment = true;
      index += 1;
    } else if (char === '/' && next === '*') {
      blockComment = true;
      index += 1;
    } else if (char === '"' || char === "'") {
      quote = char;
    } else if (char === openChar) {
      depth += 1;
    } else if (char === closeChar) {
      depth -= 1;
      if (depth === 0) {
        return index;
      }
    }
  }
  throw new Error(`Unbalanced ${openChar}${closeChar} expression near byte ${openIndex}`);
}

function matchingParen(source, openIndex) {
  return matchingDelimiter(source, openIndex, '(', ')');
}

function readQuotedString(source, quoteIndex) {
  let escaped = false;
  for (let index = quoteIndex + 1; index < source.length; index += 1) {
    const char = source[index];
    if (escaped) {
      escaped = false;
    } else if (char === '\\') {
      escaped = true;
    } else if (char === '"') {
      const literal = source.slice(quoteIndex, index + 1);
      return { value: JSON.parse(literal), endIndex: index };
    }
  }
  throw new Error(`Unterminated route string near byte ${quoteIndex}`);
}

function walkRustSources(directory) {
  return fs.readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const absolutePath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        return walkRustSources(absolutePath);
      }
      return entry.isFile() && entry.name.endsWith('.rs') ? [absolutePath] : [];
    });
}

function relativeSourcePath(absolutePath) {
  return path.relative(repoRoot, absolutePath).replaceAll('\\', '/');
}

function functionBody(relativePath, functionName) {
  const source = productionSource(relativePath);
  const matcher = new RegExp(`\\bfn\\s+${functionName}\\s*\\(`, 'g');
  const matches = [...source.matchAll(matcher)];
  if (matches.length !== 1) {
    throw new Error(
      `${relativePath} must define exactly one ${functionName}(...) function; found ${matches.length}`,
    );
  }
  const openParen = source.indexOf('(', matches[0].index);
  const closeParen = matchingParen(source, openParen);
  const openBrace = source.indexOf('{', closeParen + 1);
  if (openBrace === -1) {
    throw new Error(`${relativePath} has no body for ${functionName}(...)`);
  }
  const closeBrace = matchingDelimiter(source, openBrace, '{', '}');
  return source.slice(openBrace + 1, closeBrace);
}

function findFactorySource(functionName) {
  const candidates = walkRustSources(httpSourceRoot)
    .map(relativeSourcePath)
    .filter((relativePath) => {
      const source = productionSource(relativePath);
      return new RegExp(`\\bfn\\s+${functionName}\\s*\\(`).test(source);
    });
  if (candidates.length !== 1) {
    throw new Error(
      `Assembled router factory ${functionName}(...) must have exactly one production definition; found ${JSON.stringify(candidates)}`,
    );
  }
  return candidates[0];
}

function localRouterVariables(source) {
  return new Set(
    [...source.matchAll(/\blet\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*Router::new\s*\(\s*\)/g)]
      .map((match) => match[1]),
  );
}

function extractMergedFactories(source, relativePath) {
  const localRouters = localRouterVariables(source);
  const factories = [];
  let cursor = 0;
  while (cursor < source.length) {
    const marker = source.indexOf('.merge', cursor);
    if (marker === -1) {
      break;
    }
    let openIndex = marker + '.merge'.length;
    while (/\s/.test(source[openIndex] ?? '')) {
      openIndex += 1;
    }
    if (source[openIndex] !== '(') {
      cursor = openIndex;
      continue;
    }
    const closeIndex = matchingParen(source, openIndex);
    const expression = source.slice(openIndex + 1, closeIndex).trim();
    const factoryMatch = expression.match(/^([A-Za-z_][A-Za-z0-9_]*)\s*\(\s*\)$/);
    if (factoryMatch) {
      factories.push(factoryMatch[1]);
    } else if (!localRouters.has(expression)) {
      throw new Error(
        `${relativePath} has an untraceable .merge(${expression}); use a direct factory call or a local Router::new() chain`,
      );
    }
    cursor = closeIndex + 1;
  }
  return factories;
}

function assertSupportedComposition(source, relativePath) {
  for (const method of ['nest', 'nest_service', 'route_service']) {
    if (new RegExp(`\\.${method}\\s*\\(`).test(source)) {
      throw new Error(
        `${relativePath} uses unsupported .${method}(...); extend the live-route graph parser before assembling it`,
      );
    }
  }
}

function extractRoutes(relativePath, source = productionSource(relativePath)) {
  const routes = [];
  let cursor = 0;
  while (cursor < source.length) {
    const marker = source.indexOf('.route', cursor);
    if (marker === -1) {
      break;
    }
    let openIndex = marker + '.route'.length;
    while (/\s/.test(source[openIndex] ?? '')) {
      openIndex += 1;
    }
    if (source[openIndex] !== '(') {
      cursor = openIndex;
      continue;
    }
    let quoteIndex = openIndex + 1;
    while (/\s/.test(source[quoteIndex] ?? '')) {
      quoteIndex += 1;
    }
    if (source[quoteIndex] !== '"') {
      throw new Error(`${relativePath} has a non-literal route path near byte ${marker}`);
    }
    const routeString = readQuotedString(source, quoteIndex);
    const closeIndex = matchingParen(source, openIndex);
    const expression = source.slice(routeString.endIndex + 1, closeIndex);
    const pattern = normalizePattern(routeString.value);
    if (pattern.startsWith('/api/')) {
      for (const unsupported of ['any', 'any_service', 'connect', 'head', 'on', 'on_service', 'trace']) {
        if (new RegExp(`(?:\\b|\\.)${unsupported}\\s*\\(`).test(expression)) {
          throw new Error(
            `${relativePath} uses unsupported ${unsupported}(...) routing for ${pattern}; extend the live-route graph parser first`,
          );
        }
      }
      let methodCount = 0;
      for (const method of ownedBusinessMethods) {
        if (new RegExp(`(?:\\b|\\.)${method}\\s*\\(`).test(expression)) {
          routes.push({ method: method.toUpperCase(), pattern });
          methodCount += 1;
        }
      }
      const hasOnlyExcludedMethod = intentionallyExcludedMethods.some(
        (method) => new RegExp(`(?:\\b|\\.)${method}\\s*\\(`).test(expression),
      );
      if (methodCount === 0 && !hasOnlyExcludedMethod) {
        throw new Error(
          `${relativePath} has no recognized HTTP method for ${pattern}; extend the live-route graph parser first`,
        );
      }
    }
    cursor = closeIndex + 1;
  }
  return routes;
}

function sortedUniqueRecords(records) {
  return [...new Set(records.map(({ method, pattern }) => routeKey(method, pattern)))]
    .sort()
    .map(recordFromKey);
}

function deriveLiveGraph() {
  const generatedFrom = [];
  const visitedFactories = new Set();
  const routes = [];

  function visitFactory(factoryName, relativePath) {
    if (visitedFactories.has(factoryName)) {
      return;
    }
    visitedFactories.add(factoryName);
    const body = functionBody(relativePath, factoryName);
    assertSupportedComposition(body, relativePath);
    generatedFrom.push(relativePath);
    routes.push(...extractRoutes(relativePath, body));
    for (const nestedFactory of extractMergedFactories(body, relativePath)) {
      visitFactory(nestedFactory, findFactorySource(nestedFactory));
    }
  }

  visitFactory(rootRouterFactory, rootRouterSource);
  return {
    generatedFrom,
    routes: sortedUniqueRecords(routes),
  };
}

function liveOwnership() {
  const manifest = JSON.parse(fs.readFileSync(ownershipPath, 'utf8'));
  if (!Array.isArray(manifest.routes)) {
    throw new Error('Generated ownership manifest has no routes array');
  }
  return sortedUniqueRecords(
    manifest.routes.filter(
      (route) => route.state === 'rust_owned_verified' || route.state === 'retired',
    ),
  );
}

function routeSet(records) {
  return new Set(records.map(({ method, pattern }) => routeKey(method, pattern)));
}

function validateRouteSets(liveRecords, ownedRecords) {
  const live = routeSet(liveRecords);
  const owned = routeSet(ownedRecords);
  const missing = [...owned].filter((key) => !live.has(key)).sort();
  const extra = [...live].filter((key) => !owned.has(key)).sort();
  const legacy = forbiddenLegacy.filter((key) => owned.has(key)).sort();
  if (missing.length || extra.length || legacy.length) {
    const error = new Error(
      `runtime route ownership mismatch\nmissing=${JSON.stringify(missing)}\nextra=${JSON.stringify(extra)}\nlegacy=${JSON.stringify(legacy)}`,
    );
    error.details = { missing, extra, legacy };
    throw error;
  }
}

function assertGeneratedGraphIsCurrent(derived) {
  if (!fs.existsSync(graphPath)) {
    throw new Error(
      `${path.relative(repoRoot, graphPath)} is missing; run node scripts/check-runtime-route-ownership.mjs --refresh`,
    );
  }
  const current = JSON.parse(fs.readFileSync(graphPath, 'utf8'));
  if (JSON.stringify(current) !== JSON.stringify(derived)) {
    throw new Error(
      `${path.relative(repoRoot, graphPath)} is stale; run node scripts/check-runtime-route-ownership.mjs --refresh`,
    );
  }
}

function writeGraph(graph) {
  fs.writeFileSync(graphPath, `${JSON.stringify(graph, null, 2)}\n`, 'utf8');
}

function fixtureRecords(mode, live, owned) {
  const fixtureLive = live.map((route) => ({ ...route }));
  const fixtureOwned = owned.map((route) => ({ ...route }));
  if (mode === 'missing') {
    fixtureOwned.push({ method: 'GET', pattern: '/api/__fixture_missing_from_live__' });
  } else if (mode === 'extra') {
    const index = fixtureOwned.findIndex(
      (route) => route.method === 'GET' && route.pattern === '/api/bills/import/configs',
    );
    if (index === -1) {
      throw new Error('extra fixture anchor is missing');
    }
    fixtureOwned.splice(index, 1);
  } else if (mode === 'legacy') {
    fixtureOwned.push({
      method: 'PUT',
      pattern: '/api/bills/import/learning-rules/{rule_id}',
    });
  } else {
    throw new Error(`Unknown negative fixture: ${mode}`);
  }
  return { live: fixtureLive, owned: fixtureOwned };
}

function selfTest() {
  const live = [
    { method: 'GET', pattern: '/api/bills/import/configs' },
    { method: 'PUT', pattern: '/api/learning/rules/{rule_id}' },
  ];
  validateRouteSets(live, live);
  for (const mode of ['missing', 'extra', 'legacy']) {
    let rejected = false;
    try {
      const fixture = fixtureRecords(mode, live, live);
      validateRouteSets(fixture.live, fixture.owned);
    } catch (error) {
      rejected = Boolean(error.details?.[mode]?.length);
    }
    if (!rejected) {
      throw new Error(`${mode} fixture did not fail the checker`);
    }
  }

  const discovered = extractMergedFactories(
    'let local = Router::new().route("/api/local", get(handler)); Router::new().merge(local).merge(extra_router())',
    'merge-discovery-fixture.rs',
  );
  if (discovered.length !== 1 || discovered[0] !== 'extra_router') {
    throw new Error(`merge discovery missed an assembled factory: ${JSON.stringify(discovered)}`);
  }
  for (const fixture of [
    'Router::new().merge(prebuilt_router)',
    'Router::new().nest("/api", nested_router)',
  ]) {
    let rejected = false;
    try {
      assertSupportedComposition(fixture, 'unsupported-composition-fixture.rs');
      extractMergedFactories(fixture, 'unsupported-composition-fixture.rs');
    } catch {
      rejected = true;
    }
    if (!rejected) {
      throw new Error(`unsupported router composition did not fail closed: ${fixture}`);
    }
  }

  let unsupportedMethodRejected = false;
  try {
    extractRoutes(
      'unsupported-method-fixture.rs',
      'Router::new().route("/api/hidden", any(hidden_handler))',
    );
  } catch {
    unsupportedMethodRejected = true;
  }
  if (!unsupportedMethodRejected) {
    throw new Error('unsupported route method composition did not fail closed');
  }
  const optionsOnly = extractRoutes(
    'options-exclusion-fixture.rs',
    'Router::new().route("/api/cors-preflight", options(cors_handler))',
  );
  if (optionsOnly.length !== 0) {
    throw new Error('OPTIONS transport plumbing must stay outside business-route ownership');
  }
}

function main() {
  if (process.argv.includes('--self-test')) {
    selfTest();
    return;
  }
  const derived = deriveLiveGraph();
  if (process.argv.includes('--refresh')) {
    writeGraph(derived);
  }
  assertGeneratedGraphIsCurrent(derived);
  const owned = liveOwnership();
  const negativeIndex = process.argv.indexOf('--negative');
  if (negativeIndex !== -1) {
    const fixture = fixtureRecords(process.argv[negativeIndex + 1], derived.routes, owned);
    validateRouteSets(fixture.live, fixture.owned);
    return;
  }
  validateRouteSets(derived.routes, owned);
}

main();
