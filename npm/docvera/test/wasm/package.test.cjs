'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { resolve } = require('node:path');
const { pathToFileURL } = require('node:url');
const { packedPackage } = require('./package-fixture.cjs');

test('installed single-package WASM subpath works through ESM and require without native addons', t => {
  const artifact = packedPackage();
  t.after(artifact.cleanup);
  const smokeUrl = pathToFileURL(resolve(__dirname, 'shared.mjs')).href;
  const result = spawnSync(process.execPath, ['--input-type=module', '-e', `
    import assert from 'node:assert/strict';
    import { createRequire } from 'node:module';
    import { smoke } from ${JSON.stringify(smokeUrl)};
    import * as wasm from '@a5i/docvera/wasm';
    assert.deepEqual(Object.keys(wasm).sort(), ['openChecker', 'openCheckerWorker']);
    await smoke(wasm);
    // The Node worker_threads backend runs the same suite off-thread.
    await smoke({ openChecker: wasm.openCheckerWorker });
    const require = createRequire(import.meta.url);
    await smoke(require('@a5i/docvera/wasm'));
    assert.throws(() => require('@a5i/docvera'), /Missing docvera native addon/);
    assert.equal(require('@a5i/docvera/package.json').name, '@a5i/docvera');
    console.log('Packed WASM ESM/CommonJS: PASS');
  `], { cwd: artifact.consumer, encoding: 'utf8', timeout: 180000 });
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /PASS/);
});
