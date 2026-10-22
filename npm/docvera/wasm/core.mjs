// Shared lifecycle for Node/browser. Promises serialize work; they do NOT move it off-thread.
function managed(resource, label, methods) {
  let closed = false;
  let tail = Promise.resolve();
  let closing;
  const invoke = operation => {
    if (closed) return Promise.reject(new Error(`${label} is closed`));
    const result = tail.then(operation);
    tail = result.then(() => undefined, () => undefined);
    return result;
  };
  return {
    ...methods(invoke, resource),
    close() {
      if (!closing) {
        closed = true;
        closing = tail.then(() => resource.free());
      }
      return closing;
    },
  };
}

function wrapSnapshot(snapshot) {
  return managed(snapshot, 'SQL snapshot', (invoke, raw) => ({
    querySql: sql => invoke(() => raw.query(sql)),
  }));
}

export function createOpenChecker(load) {
  return async function openChecker(options) {
    if (!options || !options.config || !options.sources) {
      throw new TypeError('WASM openChecker requires { config, sources }; filesystem roots are not supported');
    }
    const wasm = await load();
    const raw = new wasm.Checker(options.config, options.sources);
    return managed(raw, 'Checker', invoke => ({
      check: () => invoke(() => raw.check()),
      checkDocument: (path, text) => invoke(() => raw.checkDocument(path, text)),
      checkWithText: (path, text) => invoke(() => raw.checkWithText(path, text)),
      updateDocument: (path, text) => invoke(() => raw.updateDocument(path, text)),
      removeDocument: path => invoke(() => raw.removeDocument(path)),
      updateSchema: (path, text) => invoke(() => raw.updateSchema(path, text)),
      // Convenience query against the baseline at this operation's queue position.
      querySql: sql => invoke(() => {
        const snapshot = raw.sqlSnapshot();
        try { return snapshot.query(sql); } finally { snapshot.free(); }
      }),
      // Reuse a copied revision for multiple queries; it can outlive this checker.
      sqlSnapshot: () => invoke(() => wrapSnapshot(raw.sqlSnapshot())),
    }));
  };
}
