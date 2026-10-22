// Shared transport for browser module Workers and Node worker_threads.
// References include pending opens and SQL snapshots, not just live checkers.
const METHODS = ['check', 'checkDocument', 'checkWithText', 'updateSchema', 'updateDocument', 'removeDocument', 'querySql'];

export function createOpenCheckerWorker(Worker = globalThis.Worker) {
  let session;
  const isNode = typeof process !== 'undefined' && process.versions?.node;

  function acquire() {
    if (!session || session.failed) {
      const state = { references: 0 };
      session = state;
      state.transport = (async () => {
        const pending = new Map();
        let worker, nextId = 1, failure;
        const ready = new Promise((resolve, reject) => pending.set(0, { resolve, reject }));
        const onMessage = event => {
          const { id, ok, result, error } = event.data ?? event;
          const entry = pending.get(id);
          if (!entry) return;
          pending.delete(id);
          if (ok) entry.resolve(result); else entry.reject(new Error(error));
        };
        const onError = event => {
          state.failed = true;
          failure = new Error(event.message || 'Worker failed');
          for (const entry of pending.values()) entry.reject(failure);
          pending.clear();
        };
        if (isNode) {
          const { Worker: NodeWorker } = await import('node:worker_threads');
          worker = new NodeWorker(new URL('./node-worker.cjs', import.meta.url));
          worker.on('message', onMessage);
          worker.on('error', onError);
          worker.on('exit', code => onError({ message: `Worker exited (${code})` }));
        } else {
          worker = new Worker(new URL('./worker.mjs', import.meta.url), { type: 'module' });
          worker.addEventListener('message', onMessage);
          worker.addEventListener('error', onError);
        }
        try { await ready; } catch (error) { worker.terminate(); throw error; }
        return {
          call(op, args) {
            if (failure) return Promise.reject(failure);
            return new Promise((resolve, reject) => {
              const id = nextId++;
              pending.set(id, { resolve, reject });
              try { worker.postMessage({ id, op, args }); }
              catch (error) { pending.delete(id); reject(error); }
            });
          },
          dispose: () => worker.terminate(),
        };
      })().catch(error => { state.failed = true; throw error; });
    }
    const state = session;
    state.references += 1;
    let released = false;
    return {
      transport: state.transport,
      async release() {
        if (released) return;
        released = true;
        state.references -= 1;
        // Reserve references before async construction so an in-flight open or
        // sqlSnapshot cannot race disposal by the last closing checker.
        if (state.references === 0 && (isNode || state.failed)) {
          if (session === state) session = undefined;
          try { await (await state.transport).dispose(); } catch { /* startup failed */ }
        }
      },
    };
  }

  return async function openCheckerWorker(options) {
    if (!options || !options.config || !options.sources) {
      throw new TypeError('openCheckerWorker requires { config, sources }');
    }
    const lease = acquire();
    let call, handle;
    try {
      ({ call } = await lease.transport);
      ({ handle } = await call('open', { options }));
    } catch (error) { await lease.release(); throw error; }
    let checkerClosed = false, closing;
    const guard = fn => (...args) => {
      if (checkerClosed) return Promise.reject(new Error('Checker is closed'));
      return fn(...args);
    };
    const api = {
      close() {
        if (!closing) {
          checkerClosed = true;
          closing = call('checkerClose', { handle }).finally(() => lease.release());
        }
        return closing;
      },
    };
    for (const method of METHODS) {
      api[method] = guard((...args) => call(method, { handle, args }));
    }
    api.sqlSnapshot = guard(async () => {
      const snapshotLease = acquire();
      let snap;
      try { ({ snap } = await call('sqlSnapshot', { handle })); }
      catch (error) { await snapshotLease.release(); throw error; }
      let snapClosed = false, snapClosing;
      return {
        querySql: sql => snapClosed
          ? Promise.reject(new Error('SQL snapshot is closed'))
          : call('sqlQuery', { snap, args: [sql] }),
        close() {
          if (!snapClosing) {
            snapClosed = true;
            snapClosing = call('sqlClose', { snap }).finally(() => snapshotLease.release());
          }
          return snapClosing;
        },
      };
    });
    return api;
  };
}
