'use strict';
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { spawnSync } = require('node:child_process');
const wasmFiles = require('../../scripts/wasm-files.cjs');

// Exercise the artifact consumers install, not source-relative imports.
function packedPackage() {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'docvera-wasm-package-'));
  const cleanup = () => fs.rmSync(directory, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  function npm(args, cwd) {
    const result = spawnSync(process.execPath, [process.env.npm_execpath, ...args], { cwd, encoding: 'utf8' });
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error(result.stdout + result.stderr);
    return result.stdout;
  }
  try {
    if (!process.env.npm_execpath) throw new Error('Run through npm run test:wasm or test:wasm:browser');
    const packed = JSON.parse(npm(['pack', '--json', '--ignore-scripts', '--offline', '--pack-destination', directory], path.resolve(__dirname, '../..')))[0];
    const files = packed.files.map(file => file.path);
    for (const name of wasmFiles) {
      if (!files.includes(name)) throw new Error(`npm tarball is missing ${name}; run npm run build:wasm`);
    }
    const consumer = path.join(directory, 'consumer');
    fs.mkdirSync(consumer);
    fs.writeFileSync(path.join(consumer, 'package.json'), JSON.stringify({ private: true, type: 'module' }));
    npm(['install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund', '--package-lock=false', path.join(directory, packed.filename)], consumer);
    const installed = path.join(consumer, 'node_modules/@a5i/docvera');
    // Remove every native addon: the WASM export must not load or require any of them.
    for (const name of fs.readdirSync(installed)) if (name.endsWith('.node')) fs.rmSync(path.join(installed, name));
    return { directory, consumer, installed, files, cleanup };
  } catch (error) { cleanup(); throw error; }
}
module.exports = { packedPackage };
