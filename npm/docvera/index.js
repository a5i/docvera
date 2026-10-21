'use strict';
const { resolve } = require('node:path');

const { existsSync } = require('node:fs');
const { platformName } = require('./platforms.cjs');

function loadNative() {
  const binary = resolve(__dirname, `docvera.${platformName()}.node`);
  // Do not hide a load/ABI error in a present platform binary with a fallback.
  if (existsSync(binary)) return require(binary);
  // Compatibility with older local builds; this file is not published.
  const local = resolve(__dirname, 'docvera_engine.node');
  if (existsSync(local)) return require(local);
  throw new Error(`Missing docvera native addon: ${binary}. In a source checkout, run npm run build.`);
}

const native = loadNative();

async function openWorkspace({ root = process.cwd() } = {}) {
  root = resolve(root);
  let closed = false;
  const active = new Set();
  function invoke(method, ...args) {
    if (closed) return Promise.reject(new Error('Workspace is closed'));
    const operation = native[method](root, ...args);
    active.add(operation);
    operation.then(() => active.delete(operation), () => active.delete(operation));
    return operation;
  }
  return {
    check: () => invoke('check'),
    index: ({ rebuild = false } = {}) => invoke('index', rebuild),
    querySql: sql => invoke('querySql', sql),
    references: ({ id, direction = 'outgoing', depth = 1 }) => invoke('references', id, direction, depth),
    search: options => invoke('search', options),
    extract: sectionId => invoke('extract', sectionId),
    async close() {
      closed = true;
      await Promise.allSettled([...active]);
    },
  };
}
/** Explicit reusable snapshot for editor buffers. Reopen after config/shapes/asset changes. */
async function openChecker({ root = process.cwd() } = {}) {
  const checker = await native.NativeChecker.open(resolve(root));
  let closed = false;
  const active = new Set();
  function invoke(method, ...args) {
    if (closed) return Promise.reject(new Error('Checker is closed'));
    const operation = checker[method](...args);
    active.add(operation);
    operation.then(() => active.delete(operation), () => active.delete(operation));
    return operation;
  }
  return {
    check: () => invoke('check'),
    checkDocument: (path, text) => invoke('checkDocument', path, text),
    checkWithText: (path, text) => invoke('checkWithText', path, text),
    updateSchema: (path, text) => invoke('updateSchema', path, text),
    updateDocument: (path, text) => invoke('updateDocument', path, text),
    removeDocument: path => invoke('removeDocument', path),
    async close() {
      closed = true;
      await Promise.allSettled([...active]);
      checker.close();
    },
  };
}
module.exports = { openWorkspace, openChecker };
