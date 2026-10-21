# Prototype architecture and boundaries

## Shared core

```text
Markdown + mdx.toml + local schemas
                |
      discover / parse / Document IR
                |
      schema + lint + identity checks
                |
      complete catalog / link resolution
                |
      RDF projection + SHACL Core/SPARQL
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
- `docs/contracts/v1.json`: prose descriptor template. CLI fills typed result/diagnostic field lists and full `jsonSchemas` from Rust types; the template alone is not the complete runtime contract.

The core deliberately remains one crate split into modules until contracts settle. It does not implement all eleven crates from the proposal. No database code is needed to run `check`.

`md-engine` Cargo features `index` and `shacl` are enabled by default and can be disabled independently. A checker-only build carries neither SQLite nor RDF; requested SHACL fails closed when its feature is absent. The in-memory `CheckerContext::from_sources` constructor needs no filesystem. The core compiles for `wasm32-unknown-unknown` without default features; a browser JS package/runtime validation is separate future work. See [embedded checker API and build instructions](checker.md).

Workspace configuration is canonical `mdx.toml`; `mdx.yaml` is a fallback, never merged. `policy.rs` is the Rudof adapter: it loads local `.ttl` files, projects documents and resolved edges into an ephemeral RDF graph, and maps results into existing diagnostics. No parallel custom assertion DSL or YAML/TOML-to-SHACL compiler exists. `check` remains index-free; policy sources participate in the index invalidation fingerprint. Policy findings carry structured `shacl` report objects and YAML/TOML-parser source spans (RFC 6901 `sourceField` pointers) with conservative fallbacks; location extraction never changes YAML/TOML semantics. [SHACL semantics and limits](shacl.md) document the versioned URN vocabulary, provenance rules and tested subset.

## Deliberate changes from the design proposal

1. **SQLite + FTS5 instead of DuckDB.** Bundled SQLite supplies JSON operators, transactions and BM25 without runtime extension downloads. The prototype exercises the complete local cycle before committing to DuckDB/FTS version integration. Query syntax is SQLite, not DuckDB. Storage/query/search functions are isolated in `index.rs`, but this is not yet a fully swappable backend trait.
2. **pulldown-cmark instead of comrak.** CommonMark/GFM event parsing provides source byte ranges, tables and fenced-code handling. The IR stores selected block nodes rather than a complete serializable AST.
3. **BFS graph traversal instead of recursive SQL.** Cycle-safe traversal handles incoming/outgoing references up to depth 32; output includes discovered nodes/edges, not all possible paths or dedicated cycle reports. The small graph is loaded into memory.
4. **Snapshot checks, incremental writes.** CLI `check`/`index` and Node `openWorkspace().check()` load selected current files and validate all dependencies. The explicit Rust `CheckerContext` / Node `openChecker` caches parsing and schema compilation; buffer overlays reparse only the edited document and rerun cross-document validation in memory. There is no automatic watcher or dependency-incremental SHACL evaluation. Only changed source hashes are rewritten in the content/FTS index when the schema fingerprint is unchanged; relations/diagnostics are replaced on each indexing batch. The deterministic fingerprint covers normalized configuration (including parser/front-matter settings), loaded schemas, SHACL policies and engine version.
5. **Local native package, not published prebuild infrastructure.** CI creates one npm archive per runner with its native binary. There is no hosted release, npm publication, automatic optional-platform package selection, or supported Windows arm64 target yet. Consumers can use the artifact matching their platform or build locally. Linux artifacts depend on the runner's libc baseline; Alpine/musl is not covered.

## Index and snapshots

Derived index: `.mdx/index.sqlite3`; disposable, excluded from Git. Tables: `documents`, `sections`, `nodes`, `relations`, `diagnostics` (structured `shacl` JSON column on policy findings, nullable, additive migration); private FTS and engine metadata tables are not exposed through arbitrary user SQL.

Document internal IDs are normalized relative paths; declared IDs can collide. Section IDs are `path#generated-anchor`, stable across content edits but not heading renames/reordering of duplicates. Section spans cover nested descendants; searchable `text` contains **direct content only** to avoid duplicating descendant hits. Title and heading fields are weighted 3 and 2 relative to body 1. Code remains part of raw searchable body; there is not yet a separate code-only field. Files with no headings produce no searchable section rows. Selected-metadata exposure controls are not implemented.

Each index update uses one immediate SQLite transaction for document/section/node/FTS deltas and full relation/diagnostic replacement. Concurrent writers serialize via SQLite with a five-second busy timeout; this is a local single-writer model, not a service. Do not concurrently edit source or run independent indexing writers if you require a particular source snapshot: scanning occurs before acquiring the DB write transaction. There is no global filesystem snapshot or watcher.

Queries/search/extract use read-only connections and the last explicit successful indexed snapshot. `extract` uses the source stored in that snapshot, not a possibly changed disk file. Graph reads currently use multiple statements and are intended for use between index operations, not concurrently changing service workloads. Rebuilding forces all current document rows and cleans deletions; incompatible/corrupt database files may require removing `.mdx` first.

## Safety

- Schema selection and local path references are confined to the canonical workspace root; symlink discovery is disabled. Static path confinement is not a defense against a hostile process racing symlink replacement.
- JSON Schema remote/local external resource retrieval is disabled at compile time; internal references work.
- User SQL runs on a read-only connection with a deny-by-default authorizer. Only public tables, SELECT/recursive query actions and an explicit function allowlist are admitted. No attach/detach, pragmas, extensions or arbitrary filesystem functions.
- A VM progress handler interrupts queries after roughly two seconds; at most 1,000 output rows are returned. Large SQL strings, intermediate values, permitted function results and source files are **not memory-bounded**. This is not an OS-isolated sandbox for hostile workloads.
- Parse/config/filesystem failures become either document diagnostics or operation errors. Included-file read failures stop the operation instead of pretending the file was deleted.
- SHACL policies are workspace-trusted, local-only inputs, not a sandbox. Imports and SPARQL SERVICE/FROM clauses are rejected conservatively; resource limits/process isolation and performance baselines remain future work.
- No network access, external URL checks, arbitrary structure auto-fixing, or remote crawling.

## Contracts and remaining work

Result envelopes have `contractVersion: 1`. `mdx schema` is an intentional exception: it emits a bare JSON Schema 2020-12 with a versioned URN `$id`. `contracts.rs` generates config/document-envelope contracts for deserialization and CheckResult/Diagnostic/IndexResult contracts for serialization using `schemars` derives on the same Rust types used by Serde. `mdx contract` and JSON `guide` embed these under `jsonSchemas`; command prose/exit semantics still come from the descriptor. Rust semantic checks remain authoritative and are not automatically inferred. Query/search/graph/extract still use dynamic JSON values with parity tests; TypeScript facade interfaces remain hand-maintained. Migrating those dynamic results and generating TS declarations is deferred.

The prototype covers the breadth of PoC stages A–F, but not every proposed extension. Deferred: body `$ref`/alternatives, full SHACL conformance, SHACL resource isolation and benchmarks, general completeness/freshness states, transition validation, query-exposure settings, full GFM AST persistence, semantic empty-section analysis, advanced block/table schemas, dedicated cycle/path query APIs, multilingual stemming, code-specific ranking, combined search/graph query endpoint, watch mode, external URL checks, CSV/table output, hosted schema registry, and automatic npm prebuild distribution.

## Verification

Rust integration tests exercise source validation, IR boundaries, schema changes, indexing edits/deletions, graph cycles and SQL restrictions. CLI integration tests exercise JSON purity, errors/exit codes, init non-overwrite, warnings-as-errors and the complete demo flow. Native Node tests call the binding directly and compare JSON contracts with CLI output, including edit/delete lifecycle behavior and close semantics.

GitHub Actions is configured for Linux x64/arm64, macOS x64/arm64 and Windows x64. Each runner runs Rust checks/tests, Node parity tests and builds release CLI/native npm artifacts. These jobs need to run in GitHub before those targets can be called verified. Local development verification is Linux x64.
