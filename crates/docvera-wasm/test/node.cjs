(async () => {
  const { smoke } = await import('./shared.mjs');
  smoke(require('../pkg-node/docvera_wasm.js'));
  console.log('Node WASM smoke: PASS');
})().catch(error => { console.error(error); process.exitCode = 1; });
