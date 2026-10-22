# Experimental in-memory SQL and WebAssembly

The `sql` feature adds **GlueSQL 0.20.0** queries over a validated in-memory `Workspace`. It needs neither SQLite nor the filesystem, and it is the engine's only SQL path: the CLI, native npm API and WASM all use it. There is no full-text search.

## Rust embedded API

```toml
[dependencies]
docvera-engine = { version = "0.2.0", default-features = false, features = ["sql"] }
```

```rust,ignore
use docvera_engine::{CheckerContext, WorkspaceSources, config::Config, sql::SqlSnapshot};

// The host supplies config, Markdown and schemas. See docs/checker.md for their shape.
let checker = CheckerContext::from_sources(config, sources)?;
let workspace = checker.snapshot()?;
let mut sql = SqlSnapshot::from_workspace(&workspace)?;
let result = sql.query(
    "SELECT path, metadata['owner'] AS owner FROM documents ORDER BY path"
)?;
```

`CheckerContext::snapshot()` performs cross-document validation and returns an owned `Workspace` without disk I/O. `SqlSnapshot::from_workspace` copies its relational projection into private read-only storage. It is **not zero-copy** or a live view. Create another SQL snapshot after changing documents or schemas. Reuse a snapshot for multiple queries over the same revision; it can outlive both the Workspace and checker.

### Tables

Paths remain document identities; declared IDs are nullable and need not be unique in invalid workspaces.

| Table | Columns |
|---|---|
| `documents` | `internal_id`, `declared_id`, `type`, `path`, `metadata`, `hash`, `source` |
| `sections` | `section_id`, `document_id`, `heading`, `level`, `parent_id`, `ordinal`, `text`, `span` |
| `nodes` | `node_id`, `document_id`, `section_id`, `kind`, `language`, `span` |
| `relations` | `source_document_id`, `target_document_id`, `kind`, `reference`, `resolution_status`, `source_span` |
| `diagnostics` | `document_id`, `rule`, `severity`, `span`, `message`, `shacl` |

`metadata`, spans and non-null `shacl` are GlueSQL `MAP` values, not JSON-encoded strings. Nested metadata arrays are `LIST` values. Selecting them returns ordinary JSON objects/arrays. Nullable values return JSON `null`. Other columns are text except section `level` and `ordinal` (integers). Source spans retain the checker's UTF-8 byte-offset contract.

```sql
SELECT path, metadata['title'] AS title
FROM documents
ORDER BY path;

SELECT d.path, s.heading
FROM documents d JOIN sections s ON s.document_id = d.path
WHERE s.level = 1
ORDER BY d.path;

SELECT rule, COUNT(*) AS occurrences
FROM diagnostics
GROUP BY rule;
```

Results retain the envelope `{ "contractVersion": 1, "rows": [...] }`. This is the **GlueSQL dialect**, not SQLite compatibility: in particular, SQLite `json_extract`, `->>`, FTS functions and PRAGMAs are not portable. Existing SQLite SQL reports have not all been ported or validated against this prototype. Do not assume CTEs, window functions or other SQLite constructs are available without testing.

## JavaScript / WASM

WASM ships **inside the single `@a5i/docvera` npm package**, not as another published package:

```js
// Node default: native addon, filesystem workspace/checker API.
import { openChecker } from '@a5i/docvera';
// Explicit WASM: caller-supplied source text, usable in Node or browsers.
import { openChecker as openWasmChecker } from '@a5i/docvera/wasm';
```

The `./wasm` export selects a browser ES module or Node ESM/CommonJS loader. It does not import the native entry or require a `.node` file. Both WASM binaries and their loaders are in the same npm tarball as the four native platform binaries; consumers install only `@a5i/docvera`. No install script downloads runtime artifacts. The Rust [`docvera-wasm`](../crates/docvera-wasm) crate and its private npm build-tools manifest are internal build infrastructure, not separately published APIs. This change is implemented in the checkout/release workflow; it does not itself publish a new npm version.

### Build from the repository root

Node/npm, Rust and `wasm32-unknown-unknown` are required:

```sh
rustup target add wasm32-unknown-unknown
npm --prefix crates/docvera-wasm ci
npm --prefix npm/docvera run build:wasm
npm --prefix npm/docvera run test:wasm
npm --prefix npm/docvera run test:wasm:browser # CHROMIUM=/path/to/chromium overrides executable
```

`build:wasm` runs both release-mode `wasm-pack` builds with `--locked` and assembles their outputs into `npm/docvera/wasm/{node,web}`. It uses LTO and `--no-opt`, so Binaryen/wasm-opt is not required. wasm-pack, TypeScript and Playwright test tooling are pinned in the internal build-tools npm lockfile. Generated runtime files are ignored by Git. The release workflow merges a tested WASM artifact with native binaries into one package; release validation rejects missing/empty WASM files or broken subpath exports. Native-only `build`/`build:release` scripts remain available for native development and do not build WASM implicitly.

The browser test uses the pinned Playwright from `crates/docvera-wasm/node_modules` and defaults to its matching managed Chromium. Install it with `npm --prefix crates/docvera-wasm exec -- playwright install chromium` (use `install --with-deps chromium` to also install OS dependencies on CI). CI and release workflows install this browser explicitly and do not depend on the runner's system Chrome. For a local system browser, explicitly set `CHROMIUM` to its executable path or command name on `PATH`; there is no automatic system-browser fallback. The suite exercises both the in-thread adapter and a real browser module Worker, with a bounded timeout.

### Public API

Browser/bundler ES-module usage (the bundler/server must serve the included `.wasm` asset):

```js
import { openChecker } from '@a5i/docvera/wasm';

const checker = await openChecker({
  config: {
    version: 1,
    include: ['**/*.md'],
    schemas: {},
    validation: { untyped: 'skip' }
  },
  sources: {
    documents: {
      'note.md': '---\ntitle: Hello\nowner: team-a\n---\n# Hello\n'
    },
    schemas: {},
    assets: []
  }
});
let sql;
try {
  const check = await checker.check();
  // Convenience: project/query the current baseline, then release the temporary projection.
  console.log((await checker.querySql(
    "SELECT path, metadata['owner'] AS owner FROM documents"
  )).rows);
  // [{ path: 'note.md', owner: 'team-a' }]

  // Explicit copied revision, reusable for multiple queries:
  sql = await checker.sqlSnapshot();
  await checker.updateDocument('note.md', '# New revision\n');
  // `sql` still represents the OLD revision; checker.querySql() sees the new baseline.
  const oldRows = await sql.querySql('SELECT path FROM documents');
} finally {
  await sql?.close();
  await checker.close();
}
```

Node supports the same import, or `const { openChecker } = require('@a5i/docvera/wasm')`. `openChecker` initializes WASM once per loader/module context; consumers do not call generated `init()` themselves. Browsers without a bundler need an import map for `@a5i/docvera/wasm` pointing at the package's `wasm/index.mjs`, with its relative modules and WASM assets served alongside it.

`openCheckerWorker(options)` has the same signature and result shape, but executes all checks and SQL inside one shared Worker (browser module Worker, or Node `worker_threads` in Node runtimes). WASM is compiled once per page/thread; every `openCheckerWorker` call adds an independent checker handle with its own serialized queue and `close()`. Worker-level cancellation is host policy: terminate or dispose the host Worker; there is no per-operation cancellation or timeout inside the Worker.

In Node, the shared Worker terminates only after all checker **and SQL snapshot** handles close. Pending construction also keeps it alive. Browser Workers stay available for reuse.

Curated TypeScript declarations cover config, sources, checks, recursive JSON SQL values and lifecycle. The checker exposes `check`, `checkDocument`, `checkWithText`, `updateDocument`, `removeDocument`, `updateSchema`, `querySql`, `sqlSnapshot`, and `close`. `sqlSnapshot()` returns an independently owned `{ querySql, close }` handle. Public callers use **`close()`, not `.free()`**; generated raw bindings are internal.

Operations return Promises and are serialized in invocation order per checker/snapshot. A failed operation does not poison the queue. `close()` rejects new operations, waits for accepted work, then frees the WASM resource; repeated closes are safe. SQL snapshots can outlive the checker and must be closed independently. The in-thread adapters execute synchronous WASM on the calling thread; for off-thread execution use `openCheckerWorker` (see below).

Input sources accept `documents`, `schemas`, `shapes` as path → text objects and `assets` as an array of existing asset paths. WASM has no SHACL support: a config requesting SHACL fails closed. All data/config is supplied by the caller, unlike native `openChecker({ root })`; there is no WASM `openWorkspace({ root })`. JS objects/arrays/null are used instead of `Map`/`undefined`, and operation failures reject with JS `Error` objects.

## Safety and prototype boundaries

- Exactly one query plan is accepted. DML, DDL, transaction commands and SHOW statements are rejected; storage mutation methods independently reject writes.
- Duplicate output labels are rejected; use unique aliases for joins.
- Results above 1000 rows are rejected **after execution**, not during it. Use `LIMIT`, but do not treat it as an execution-cost guarantee.
- No query timeout, instruction budget, memory ceiling or full sandbox is implemented. Cross joins, sorting, aggregates or large expressions can still consume significant CPU/memory. Only run trusted queries. `openCheckerWorker` runs everything inside a shared Worker (browser module Worker or Node `worker_threads`), so terminating/replacing the host Worker is the only cancellation path; a main-thread timer cannot interrupt a synchronous WASM call.
- Storage uses table scans and allocates projected values. Snapshot creation includes validation and copies source text. Browser compilation and small functional tests do not establish acceptable performance on large workspaces.
- No SQLite/FTS/RDF dependencies in the standalone WASM build. Build it with `-p docvera-wasm`, not `--workspace --target wasm32-unknown-unknown`, which would also select native-only packages/features.

## Verification

```sh
cargo test --locked -p docvera-engine --no-default-features --features sql
cargo clippy --locked -p docvera-wasm -- -D warnings
cargo build --locked -p docvera-wasm --target wasm32-unknown-unknown
cargo test --locked --workspace
npm --prefix npm/docvera run build:wasm
npm --prefix npm/docvera run test:wasm
npm --prefix npm/docvera run test:wasm:browser
cargo tree -p docvera-wasm --edges normal
```

Native tests cover structured values, JOIN, aggregation, empty tables/workspaces, snapshot independence, invalid queries, write rejection, duplicate labels and the row cap. Package tests create an actual npm tarball, install it offline and remove native addons; the same public API suite then runs through Node ESM, the Node `worker_threads` backend, CommonJS, and the browser export in real Chromium via Playwright. It covers schema checking/overlays/updates, SQL joins/metadata/diagnostics, null/plain-object serialization, snapshot isolation, queued-operation order, error recovery, idempotent close and SHACL fail-closed behavior. TypeScript checks exercise the installed declarations under NodeNext (ESM/CommonJS) and Bundler resolution. Release-packaging regression tests require both WASM payloads alongside all four native binaries.
