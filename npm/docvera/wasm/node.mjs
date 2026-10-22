import { createOpenChecker } from './core.mjs';
let loading;
async function load() {
  if (!loading) {
    loading = import('./node/docvera_wasm.js').then(module => module.default)
      .catch(error => { loading = undefined; throw error; });
  }
  return loading;
}
import { createOpenCheckerWorker } from './worker-client.mjs';

export const openCheckerWorker = createOpenCheckerWorker();
export const openChecker = createOpenChecker(load);
