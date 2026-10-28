#!/usr/bin/env node
// Compatibility entry point for the original interface regression suite.
const path = require('node:path');
const {spawnSync} = require('node:child_process');
const test = path.join(__dirname, '../tests/ui/original-ui.test.cjs');
try { require.resolve('jsdom', {paths:[path.dirname(test)]}); }
catch (_) { console.error('Install UI test dependencies first: npm ci --prefix tests/ui'); process.exit(1); }
const result = spawnSync(process.execPath, [test], {stdio:'inherit'});
process.exit(result.status ?? 1);
