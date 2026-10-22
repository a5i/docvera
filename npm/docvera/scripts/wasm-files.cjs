'use strict';
// Exact runtime payload merged into the single @a5i/docvera release artifact.
module.exports = [
  'wasm/core.mjs', 'wasm/index.mjs', 'wasm/index.d.ts', 'wasm/node.mjs', 'wasm/node.cjs',
  'wasm/worker.mjs', 'wasm/worker-client.mjs',
  'wasm/node/docvera_wasm.js', 'wasm/node/docvera_wasm_bg.wasm',
  'wasm/web/docvera_wasm.mjs', 'wasm/web/docvera_wasm_bg.wasm',
];
