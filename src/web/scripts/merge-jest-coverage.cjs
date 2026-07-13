const fs = require('node:fs');
const path = require('node:path');

const { createCoverageMap } = require('istanbul-lib-coverage');
const { createContext } = require('istanbul-lib-report');
const reports = require('istanbul-reports');

const root = path.resolve(__dirname, '..');
const mainDirectory = path.join(root, 'coverage');
const supplementalDirectory = path.join(root, 'coverage', 'changed-sfc');
const mainCoverage = JSON.parse(fs.readFileSync(path.join(mainDirectory, 'coverage-final.json'), 'utf8'));
const supplementalCoverage = JSON.parse(
    fs.readFileSync(path.join(supplementalDirectory, 'coverage-final.json'), 'utf8')
);
const coverageMap = createCoverageMap(mainCoverage);
coverageMap.merge(supplementalCoverage);

fs.writeFileSync(
    path.join(mainDirectory, 'coverage-final.json'),
    JSON.stringify(coverageMap.toJSON())
);
const context = createContext({ dir: mainDirectory, coverageMap });
reports.create('lcovonly').execute(context);
