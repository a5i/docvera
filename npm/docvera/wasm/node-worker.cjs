'use strict';
// Node worker_threads RPC loop over the SAME core (pkg-node glue loads wasm from disk).
const { parentPort } = require('node:worker_threads');
const { createRequire } = require('node:module');
const nodeRequire = createRequire(__filename);
const wasm = nodeRequire('./node/docvera_wasm.js');

(async () => {
  const { createOpenChecker } = await import('./core.mjs');
  const open = createOpenChecker(async () => wasm);
  const checkers = new Map();
  const snapshots = new Map();
  let seq = 0;
  const post = message => parentPort.postMessage(message);
  parentPort.on('message', async message => {
    const { id, op, args } = message;
    try {
      if (op === 'open') {
        const checker = await open(args.options);
        const handle = `c${++seq}`;
        checkers.set(handle, checker);
        post({ id, ok: true, result: { handle } });
        return;
      }
      if (op === 'sqlQuery' || op === 'sqlClose') {
        const snapshot = snapshots.get(args.snap);
        if (!snapshot) throw new Error(`Unknown SQL snapshot handle: ${args.snap}`);
        if (op === 'sqlQuery') {
          post({ id, ok: true, result: await snapshot.querySql(...args.args) });
        } else {
          snapshots.delete(args.snap);
          await snapshot.close();
          post({ id, ok: true, result: null });
        }
        return;
      }
      const checker = args?.handle ? checkers.get(args.handle) : undefined;
      if (!checker) throw new Error(`Unknown checker handle: ${args?.handle}`);
      if (op === 'checkerClose') {
        checkers.delete(args.handle);
        await checker.close();
        post({ id, ok: true, result: null });
        return;
      }
      if (op === 'sqlSnapshot') {
        const snapshot = await checker.sqlSnapshot();
        const snap = `s${++seq}`;
        snapshots.set(snap, snapshot);
        post({ id, ok: true, result: { snap } });
        return;
      }
      post({ id, ok: true, result: await checker[op](...args.args) });
    } catch (error) {
      post({ id, ok: false, error: error instanceof Error ? error.message : String(error) });
    }
  });
  post({ id: 0, ok: true, result: 'ready' });
})();
