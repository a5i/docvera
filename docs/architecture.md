# Prototype architecture and boundaries

## Shared core

```text
Markdown + mdx.yaml + local schemas
                |
      discover / parse / Document IR
                |
      schema + lint + identity checks
                |
      complete catalog / link resolution
                |
          CheckResult + graph
                |
      atomic SQLite/FTS5 indexed snapshot
                |
       SQL / BFS graph / BM25 / extract
                |
          CLI       native Node API
```

- `crates/md-engine`: model, Markdown parser, schema semantics, workspace resolution, index and retrieval.
- `crates/md-cli`: clap CLI, exit codes, human/JSON/JSONL presentation.
- `crates/md-node`: napi-rs async entry points invoking the same engine via blocking workers.
- `npm/mdx`: zero-dependency facade, TypeScript interfaces, local native build, parity tests.
- `examples/workspace`: three typed documents and schemas; embedded by `mdx init`.
- `docs/contracts/v1.json`: versioned command/result descriptor; not a complete OpenAPI/JSON Schema document.

The core deliberately remains one crate split into modules until contracts settle. It does not implement all eleven crates from the proposal. No database code is needed to run `check`.

## Deliberate changes from the design proposal

1. **SQLite + FTS5 instead of DuckDB.** Bundled SQLite supplies JSON operators, transactions and BM25 without runtime extension downloads. The prototype exercises the complete local cycle before committing to DuckDB/FTS version integration. Query syntax is SQLite, not DuckDB. Storage/query/search functions are isolated in `index.rs`, but this is not yet a fully swappable backend trait.
2. **pulldown-cmark instead of comrak.** CommonMark/GFM event parsing provides source byte ranges, tables and fenced-code handling. The IR stores selected block nodes rather than a complete serializable AST.
3. **BFS graph traversal instead of recursive SQL.** Cycle-safe traversal handles incoming/outgoing references up to depth 32; output includes discovered nodes/edges, not all possible paths or dedicated cycle reports. The small graph is loaded into memory.
4. **Coarse validation, incremental writes.** Every `check`/`index` rescans and reparses selected files and validates all dependencies. Only changed source hashes are rewritten in the content/FTS index when the schema fingerprint is unchanged. Relations/diagnostics are replaced on every indexing batch. Schema/config/version changes invalidate all documents; the current fingerprint also depends on selected schema occurrences, so adding/deleting documents can force a wider refresh. This trades speed for correct inbound-reference invalidation in a small PoC.
5. **Local native package, not published prebuild infrastructure.** CI creates one npm archive per runner with its native binary. There is no hosted release, npm publication, automatic optional-platform package selection, or supported Windows arm64 target yet. Consumers can use the artifact matching their platform or build locally. Linux artifacts depend on the runner's libc baseline; Alpine/musl is not covered.

## Index and snapshots

Derived index: `.mdx/index.sqlite3`; disposable, excluded from Git. Tables: `documents`, `sections`, `nodes`, `relations`, `diagnostics`; private FTS and engine metadata tables are not exposed through arbitrary user SQL.

Document internal IDs are normalized relative paths; declared IDs can collide. Section IDs are `path#generated-anchor`, stable across content edits but not heading renames/reordering of duplicates. Section spans cover nested descendants; searchable `text` contains **direct content only** to avoid duplicating descendant hits. Title and heading fields are weighted 3 and 2 relative to body 1. Code remains part of raw searchable body; there is not yet a separate code-only field. Files with no headings produce no searchable section rows. Selected-metadata exposure controls are not implemented.

Each index update uses one immediate SQLite transaction for document/section/node/FTS deltas and full relation/diagnostic replacement. Concurrent writers serialize via SQLite with a five-second busy timeout; this is a local single-writer model, not a service. Do not concurrently edit source or run independent indexing writers if you require a particular source snapshot: scanning occurs before acquiring the DB write transaction. There is no global filesystem snapshot or watcher.

Queries/search/extract use read-only connections and the last explicit successful indexed snapshot. `extract` uses the source stored in that snapshot, not a possibly changed disk file. Graph reads currently use multiple statements and are intended for use between index operations, not concurrently changing service workloads. Rebuilding forces all current document rows and cleans deletions; incompatible/corrupt database files may require removing `.mdx` first.

## Safety

- Schema selection and local path references are confined to the canonical workspace root; symlink discovery is disabled. Static path confinement is not a defense against a hostile process racing symlink replacement.
- JSON Schema remote/local external resource retrieval is disabled at compile time; internal references work.
- User SQL runs on a read-only connection with a deny-by-default authorizer. Only public tables, SELECT/recursive query actions and an explicit function allowlist are admitted. No attach/detach, pragmas, extensions or arbitrary filesystem functions.
- A VM progress handler interrupts queries after roughly two seconds; at most 1,000 output rows are returned. Large SQL strings, intermediate values, permitted function results and source files are **not memory-bounded**. This is not an OS-isolated sandbox for hostile workloads.
- Parse/config/filesystem failures become either document diagnostics or operation errors. Included-file read failures stop the operation instead of pretending the file was deleted.
- No network access, external URL checks, arbitrary structure auto-fixing, or remote crawling.

## Contracts and remaining work

Every successful result envelope has `contractVersion: 1`. Check results are typed Rust structs; query/search/graph/extract currently use JSON values with parity tests. The TypeScript facade interfaces and the command descriptor are **hand-maintained**, not generated from Rust schema types. Migrating all dynamic results to Rust structs and generating schemas/declarations is a next hardening step.

The prototype covers the breadth of PoC stages A–F, but not every proposed extension. Deferred: body `$ref`/alternatives, arbitrary cross-field/cross-document assertion language, reciprocal relation constraints, query-exposure settings, full GFM AST persistence, semantic empty-section analysis, advanced block/table schemas, dedicated cycle/path query APIs, multilingual stemming, code-specific ranking, combined search/graph query endpoint, watch mode, external URL checks, CSV/table output, hosted schema registry, and automatic npm prebuild distribution.

## Verification

Rust integration tests exercise source validation, IR boundaries, schema changes, indexing edits/deletions, graph cycles and SQL restrictions. CLI integration tests exercise JSON purity, errors/exit codes, init non-overwrite, warnings-as-errors and the complete demo flow. Native Node tests call the binding directly and compare JSON contracts with CLI output, including edit/delete lifecycle behavior and close semantics.

GitHub Actions is configured for Linux x64/arm64, macOS x64/arm64 and Windows x64. Each runner runs Rust checks/tests, Node parity tests and builds release CLI/native npm artifacts. These jobs need to run in GitHub before those targets can be called verified. Local development verification is Linux x64.
