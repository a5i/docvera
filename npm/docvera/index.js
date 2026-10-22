'use strict';
const { resolve } = require('node:path');
const { existsSync } = require('node:fs');
const { platformName } = require('./platforms.cjs');
function loadNative() {
  const binary = resolve(__dirname, `docvera.${platformName()}.node`);
  if (existsSync(binary)) return require(binary);
  const local = resolve(__dirname, 'docvera_engine.node');
  if (existsSync(local)) return require(local);
  throw new Error(`Missing docvera native addon: ${binary}. In a source checkout, run npm run build.`);
}
const native = loadNative();
function managed(raw, label, methods) {
  let closed = false, tail = Promise.resolve(), closing;
  const invoke = operation => {
    if (closed) return Promise.reject(new Error(`${label} is closed`));
    const result = tail.then(operation);
    tail = result.then(() => undefined, () => undefined);
    return result;
  };
  return { ...methods(invoke), close() {
    if (!closing) { closed = true; closing = tail.then(() => raw.close()); }
    return closing;
  } };
}
function snapshot(raw) {
  return managed(raw, 'SQL snapshot', invoke => ({ querySql: sql => invoke(() => raw.querySql(sql)) }));
}
/** Lazy workspace view: every operation re-reads current files, like the CLI. */
async function openWorkspace(options = {}) {
  const root = resolve(options.root ?? process.cwd());
  let closed = false, tail = Promise.resolve(), closing;
  const invoke = operation => {
    if (closed) return Promise.reject(new Error('Workspace is closed'));
    const result = tail.then(async () => {
      const raw = await native.NativeChecker.open(root);
      try { return await operation(raw); } finally { raw.close(); }
    });
    tail = result.then(() => undefined, () => undefined);
    return result;
  };
  return {
    check: () => invoke(raw => raw.check()),
    querySql: sql => invoke(raw => raw.querySql(sql)),
    references: ({ id, direction = 'outgoing', depth = 1 }) => invoke(raw => raw.references(id, direction, depth)),
    extract: sectionId => invoke(raw => raw.extract(sectionId)),
    close() {
      if (!closing) { closed = true; closing = tail; }
      return closing;
    },
  };
}
/** Owned reusable baseline for editor buffers; reopen after config/schema/asset changes. */
async function openChecker(options = {}) {
  const { root = process.cwd(), config, sources } = options;
  const raw = config !== undefined || sources !== undefined
    ? await native.NativeChecker.fromSources(config, sources)
    : await native.NativeChecker.open(resolve(root));
  return managed(raw, 'Checker', invoke => ({
    check: () => invoke(() => raw.check()),
    checkDocument: (path, text) => invoke(() => raw.checkDocument(path, text)),
    checkWithText: (path, text) => invoke(() => raw.checkWithText(path, text)),
    updateSchema: (path, text) => invoke(() => raw.updateSchema(path, text)),
    updateDocument: (path, text) => invoke(() => raw.updateDocument(path, text)),
    removeDocument: path => invoke(() => raw.removeDocument(path)),
    querySql: sql => invoke(() => raw.querySql(sql)),
    references: ({ id, direction = 'outgoing', depth = 1 }) => invoke(() => raw.references(id, direction, depth)),
    extract: sectionId => invoke(() => raw.extract(sectionId)),
    sqlSnapshot: () => invoke(async () => snapshot(await raw.sqlSnapshot())),
  }));
}
module.exports = { openWorkspace, openChecker };
