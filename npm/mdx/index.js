'use strict';
const { resolve } = require('node:path');
const native = require('./md_engine.node');

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
module.exports = { openWorkspace };
