// Browser/bundler entry. No Node builtins and no native addon import.
import { createOpenChecker } from './core.mjs';
import { createOpenCheckerWorker } from './worker-client.mjs';

export const openCheckerWorker = createOpenCheckerWorker();
let loading;
async function load() {
  if (!loading) {
    loading = import('./web/docvera_wasm.mjs').then(async wasm => {
      await wasm.default();
      return wasm;
    }).catch(error => { loading = undefined; throw error; });
  }
  return loading;
}
export const openChecker = createOpenChecker(load);
