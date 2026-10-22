'use strict';
const { copyFileSync, mkdirSync, readFileSync, statSync } = require('node:fs');
const { resolve, join } = require('node:path');
const root = resolve(__dirname, '..');
const crate = resolve(root, '../../crates/docvera-wasm');
const manifest = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
for (const [target, out, extension] of [['node', 'pkg-node', 'js'], ['web', 'pkg-web', 'mjs']]) {
  const pkg = JSON.parse(readFileSync(join(crate, out, 'package.json'), 'utf8'));
  if (pkg.version !== manifest.version) throw new Error(`WASM/npm version mismatch: ${pkg.version} != ${manifest.version}`);
  const files = [
    ['docvera_wasm.js', `docvera_wasm.${extension}`],
    ['docvera_wasm_bg.wasm', 'docvera_wasm_bg.wasm'],
  ];
  // Check all source artifacts before writing the target directory.
  for (const [source] of files) {
    if (!statSync(join(crate, out, source)).size) throw new Error(`Empty WASM build output: ${out}/${source}`);
  }
  const destination = join(root, 'wasm', target);
  mkdirSync(destination, { recursive: true });
  for (const [source, name] of files) copyFileSync(join(crate, out, source), join(destination, name));
}
copyFileSync(resolve(root, '../../LICENSE'), join(root, 'LICENSE'));
console.log('Assembled Node/web WASM into @a5i/docvera/wasm (no separate package).');
