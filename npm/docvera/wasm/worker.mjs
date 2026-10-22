// Worker-side RPC loop: the SAME core runs here, off the browser UI thread.
import init, { Checker } from './web/docvera_wasm.mjs';
import { createOpenChecker } from './core.mjs';

await init();

const open = createOpenChecker(async () => ({ Checker }));
const checkers = new Map();
const snapshots = new Map();
let seq = 0;

function post(message) { self.postMessage(message); }

self.addEventListener('message', async event => {
  const { id, op, args } = event.data;
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
        const result = await snapshot.querySql(...args.args);
        post({ id, ok: true, result });
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
    const result = await checker[op](...args.args);
    post({ id, ok: true, result });
  } catch (error) {
    post({ id, ok: false, error: error instanceof Error ? error.message : String(error) });
  }
});

// The client waits for this handshake before sending any requests.
post({ id: 0, ok: true, result: 'ready' });
