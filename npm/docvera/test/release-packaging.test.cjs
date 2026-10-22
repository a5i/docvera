'use strict';
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join, resolve, dirname } = require('node:path');
const { spawnSync } = require('node:child_process');
const { runInNewContext } = require('node:vm');
const { validateRelease } = require('../scripts/validate-release.cjs');
const { platformName, binaryNames } = require('../platforms.cjs');
const manifest = require('../package.json');
const wasmFiles = require('../scripts/wasm-files.cjs');

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'docvera-release-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  writeFileSync(join(root, 'package.json'), JSON.stringify(manifest));
  for (const name of [...binaryNames, ...wasmFiles, 'index.js', 'index.d.ts', 'platforms.cjs', 'LICENSE']) {
    mkdirSync(dirname(join(root, name)), { recursive: true });
    writeFileSync(join(root, name), 'fixture');
  }
  return root;
}

test('release rejects the retired Intel macOS addon', t => {
  assert.equal(binaryNames.length, 4);
  const root = fixture(t);
  writeFileSync(join(root, 'docvera.darwin-x64.node'), 'obsolete Intel build');
  assert.throws(() => validateRelease(root), /four platform binaries/);
});

test('one release package contains all four platform binaries and no dependencies', t => {
  assert.equal(manifest.optionalDependencies, undefined);
  validateRelease(fixture(t));
});

test('release rejects missing/empty/unexpected binaries and old platform dependencies', async t => {
  for (const mutation of ['missing', 'empty', 'unexpected', 'dependencies']) {
    await t.test(mutation, t => {
      const root = fixture(t);
      const addon = join(root, binaryNames[0]);
      if (mutation === 'missing') rmSync(addon);
      if (mutation === 'empty') writeFileSync(addon, '');
      if (mutation === 'unexpected') writeFileSync(join(root, 'docvera.unknown.node'), 'fixture');
      if (mutation === 'dependencies') {
        writeFileSync(join(root, 'package.json'), JSON.stringify({ ...manifest, optionalDependencies: { '@a5i/docvera-linux-x64-gnu': manifest.version } }));
      }
      assert.throws(() => validateRelease(root));
    });
  }
});
test('release rejects missing/empty WASM files and broken exports', async t => {
  for (const mutation of ['missing', 'empty', 'exports']) {
    await t.test(mutation, t => {
      const root = fixture(t);
      const wasm = join(root, 'wasm/web/docvera_wasm_bg.wasm');
      if (mutation === 'missing') rmSync(wasm);
      if (mutation === 'empty') writeFileSync(wasm, '');
      if (mutation === 'exports') writeFileSync(join(root, 'package.json'), JSON.stringify({ ...manifest, exports: {} }));
      assert.throws(() => validateRelease(root));
    });
  }
});

test('npm file whitelist ships all binaries, but no legacy addon or platform package directories', { skip: process.platform === 'win32' }, t => {
  const root = fixture(t);
  writeFileSync(join(root, 'docvera_engine.node'), 'old local build');
  mkdirSync(join(root, 'pkg'));
  writeFileSync(join(root, 'pkg', 'unused.node'), 'obsolete platform package');
  const result = spawnSync('npm', ['pack', '--dry-run', '--json', '--ignore-scripts', '--offline'], { cwd: root, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  const files = JSON.parse(result.stdout)[0].files.map(file => file.path).sort();
  assert.deepEqual(files, [...binaryNames, ...wasmFiles, 'LICENSE', 'index.js', 'index.d.ts', 'platforms.cjs', 'package.json'].sort());
});

const loaderSource = readFileSync(resolve(__dirname, '../index.js'), 'utf8');
function exerciseLoader(platform, present, failNative = false) {
  const loaded = [];
  const root = resolve('/virtual/docvera');
  runInNewContext(loaderSource, {
    __dirname: root,
    module: { exports: {} },
    require(name) {
      if (name === './platforms.cjs') return { platformName: () => platform };
      if (name === 'node:fs') return { existsSync: file => present.includes(file.slice(root.length + 1)) };
      if (name.endsWith('.node')) {
        loaded.push(name);
        if (failNative) throw new Error('native ABI error');
        return {};
      }
      return require(name);
    },
  });
  return loaded;
}

test('loader selects the matching bundled binary for all four supported platforms', () => {
  for (const [os, arch] of [['linux', 'x64'], ['linux', 'arm64'], ['darwin', 'arm64'], ['win32', 'x64']]) {
    const platform = platformName(os, arch);
    const loaded = exerciseLoader(platform, binaryNames);
    assert.deepEqual(loaded, [resolve('/virtual/docvera', `docvera.${platform}.node`)]);
  }
  assert.throws(() => platformName('linux', 'riscv64'), /Unsupported docvera platform/);
  assert.throws(() => platformName('darwin', 'x64'), /Unsupported docvera platform/);
});

test('loader retains the old local fallback but does not mask native load errors', () => {
  assert.deepEqual(exerciseLoader('linux-x64-gnu', ['docvera_engine.node']), [resolve('/virtual/docvera', 'docvera_engine.node')]);
  assert.throws(() => exerciseLoader('linux-x64-gnu', []), /Missing docvera native addon/);
  assert.throws(() => exerciseLoader('linux-x64-gnu', [...binaryNames, 'docvera_engine.node'], true), /native ABI error/);
});

// Mock npm completely: never use login credentials or publish from tests.
for (const mode of ['dry-run', 'publish', 'existing']) {
  test(`bootstrap ${mode} touches ONLY @a5i/docvera`, { skip: process.platform === 'win32' }, t => {
    const root = mkdtempSync(join(tmpdir(), 'docvera-bootstrap-'));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const log = join(root, 'calls.jsonl');
    writeFileSync(log, '');
    const mock = `#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const args = process.argv.slice(2);
const command = args[0];
if (command === 'whoami') { console.log('test-user'); process.exit(0); }
if (command === 'view') {
  if (args[1] !== '@a5i/docvera@0.0.0') process.exit(2);
  if (process.env.BOOTSTRAP_MODE !== 'existing') process.exit(1);
  console.log(args.includes('deprecated') ? 'already deprecated' : '0.0.0');
  process.exit(0);
}
if (!['publish', 'deprecate'].includes(command)) process.exit(2);
const manifest = command === 'publish' ? JSON.parse(fs.readFileSync(path.join(args.at(-1), 'package.json'))) : null;
fs.appendFileSync(process.env.BOOTSTRAP_LOG, JSON.stringify({args, manifest}) + '\\n');
`;
    writeFileSync(join(root, 'npm'), mock, { mode: 0o755 });
    const script = resolve(__dirname, '../../../scripts/npm-trusted-publishing-bootstrap.sh');
    const result = spawnSync('bash', [script, ...(mode === 'dry-run' ? ['--dry-run'] : [])], {
      encoding: 'utf8',
      env: { ...process.env, PATH: `${root}:${process.env.PATH}`, BOOTSTRAP_MODE: mode, BOOTSTRAP_LOG: log },
    });
    assert.equal(result.status, 0, result.stdout + result.stderr);
    const calls = readFileSync(log, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse);
    if (mode === 'existing') {
      assert.match(result.stdout, /already on npm/);
      assert.deepEqual(calls, []);
      return;
    }
    const publishes = calls.filter(call => call.args[0] === 'publish');
    assert.equal(publishes.length, 1);
    assert.equal(publishes[0].manifest.name, '@a5i/docvera');
    assert.equal(publishes[0].manifest.version, '0.0.0');
    assert.equal(publishes[0].manifest.optionalDependencies, undefined);
    assert.equal(publishes[0].args.includes('--dry-run'), mode === 'dry-run');
    const deprecations = calls.filter(call => call.args[0] === 'deprecate');
    assert.equal(deprecations.length, mode === 'publish' ? 1 : 0);
    if (deprecations.length) assert.equal(deprecations[0].args[1], '@a5i/docvera@0.0.0');
  });
}
