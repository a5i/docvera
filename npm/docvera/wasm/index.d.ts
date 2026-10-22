import type { PortableChecker, QueryResult, SqlSnapshot, WorkspaceConfig, WorkspaceSources } from '../index.js';
export type { CheckResult, Span, Diagnostic, JsonValue, QueryResult, SqlSnapshot, WorkspaceConfig, WorkspaceSources } from '../index.js';
/** Promise-based API; work is synchronous WASM on the calling thread, not a worker. */
export interface WasmChecker extends PortableChecker {
  /** Project/query the current baseline; a temporary SQL snapshot is freed after each query. */
  querySql(sql: string): Promise<QueryResult>;
  /** Copy a validated SQL revision for reuse. Caller must close it independently. */
  sqlSnapshot(): Promise<SqlSnapshot>;
}

/** Initializes WASM once per JS module context. No filesystem/native addon is used.
 * Calls on each checker/snapshot are serialized in invocation order.
 * close() rejects new operations, waits for queued work, then frees WASM memory.
 * In-thread variant: synchronous WASM runs on the calling thread; for
 * off-thread browser/Node execution use openCheckerWorker.
 */
/** Worker variant of the same API: all WASM work runs inside one shared Worker
 * (browser module Worker; Node `worker_threads` in Node runtimes).
 * WASM compiles once per page/thread; each call adds an independent checker
 * handle with its own serialized queue and close(). close() releases that
 * checker's memory and waits for its accepted work. Node terminates the shared
 * Worker only after all checkers and SQL snapshots close; browsers reuse it.
 */
export function openCheckerWorker(options: {
  config: WorkspaceConfig;
  sources: WorkspaceSources;
}): Promise<WasmChecker>;
export function openChecker(options: {
  config: WorkspaceConfig;
  sources: WorkspaceSources;
}): Promise<WasmChecker>;
