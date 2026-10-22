'use strict';
// Keep require() available without making browser modules depend on Node builtins.
exports.openChecker = async options => {
  const { openChecker } = await import('./node.mjs');
  return openChecker(options);
};
exports.openCheckerWorker = async options => {
  const { openCheckerWorker } = await import('./node.mjs');
  return openCheckerWorker(options);
};
