# Prototype architecture and boundaries

## Shared core

```text
Markdown + docvera.toml + local schemas
                |
      discover / parse / Document IR
                |
      schema + lint + identity checks
                |
      complete catalog / link resolution
                |
      RDF projection + SHACL Core/SPARQL
                |
          CheckResult + Workspace
                |
   in-memory GlueSQL / BFS graph / extract
                |
    CLI       native Node API       WASM
```

- `crates/docvera-engine`: model, Markdown parser, schema semantics, workspace resolution, retrieval (GlueSQL/graph/extract).
- `crates/docvera-cli`: clap CLI, exit codes, human/JSON/JSONL presentation.
- `crates/docvera-node`: napi-rs async entry points invoking the same engine via blocking workers.
- `npm/docvera`: zero-dependency facade, TypeScript interfaces, native build, bundled WASM subpath, parity tests.
- `crates/docvera-engine/assets/workspace`: three typed documents and schemas; embedded by `docvera init`.
- `crates/docvera-cli/assets/contracts/v1.json`: prose descriptor template. CLI fills typed result/diagnostic field lists and full `jsonSchemas` from Rust types; the template alone is not the complete runtime contract.

The core deliberately remains one crate split into modules until contracts settle. It does not implement all eleven crates from the proposal. No database code is needed to run `check`.

`docvera-engine` Cargo features `sql` and `shacl` are enabled by default and can be disabled independently. A checker-only build carries neither GlueSQL nor RDF; requested SHACL fails closed when its feature is absent. The in-memory `CheckerContext::from_sources` constructor needs no filesystem. The core compiles for `wasm32-unknown-unknown` and ships as the `@a5i/docvera/wasm` npm subpath. See [embedded checker API and build instructions](checker.md) and [in-memory SQL and WASM](in-memory-sql.md).

Workspace configuration is canonical `docvera.toml`; `docvera.yaml` is a fallback, never merged. `policy.rs` is the Rudof adapter: it loads local `.ttl` files, projects documents and resolved edges into an ephemeral RDF graph, and maps results into existing diagnostics. No parallel custom assertion DSL or YAML/TOML-to-SHACL compiler exists. `check` is database-free; policy sources participate in the workspace fingerprint. Policy findings carry structured `shacl` report objects and YAML/TOML-parser source spans (RFC 6901 `sourceField` pointers) with conservative fallbacks; location extraction never changes YAML/TOML semantics. [SHACL semantics and limits](../crates/docvera-cli/assets/shacl.md) document the versioned URN vocabulary, provenance rules and tested subset.

## Deliberate changes from the design proposal

1. **Pure-Rust GlueSQL instead of SQLite/DuckDB.** The engine carries no bundled C library. Read-only SQL runs over an in-memory projection of the validated workspace (`sql.rs`); there is no persistent index to migrate or rebuild. The dialect differs from SQLite (structured `MAP`/`LIST` access via `column['key']`, no JSON functions/PRAGMAs).
2. **pulldown-cmark instead of comrak.** CommonMark/GFM event parsing provides source byte ranges, tables and fenced-code handling. The IR stores selected block nodes rather than a complete serializable AST.
3. **BFS graph traversal instead of recursive SQL.** Cycle-safe traversal handles incoming/outgoing references up to depth 32; output includes discovered nodes/edges, not all possible paths or dedicated cycle reports. The small graph is loaded into memory.
4. **Snapshot checks.** CLI commands and Node `openWorkspace()` re-read selected current files per operation. The explicit Rust `CheckerContext` / Node `openChecker` caches parsing and schema compilation; buffer overlays reparse only the edited document and rerun cross-document validation in memory. There is no automatic watcher or dependency-incremental SHACL evaluation. The workspace fingerprint covers normalized configuration (including parser/front-matter settings), loaded schemas, SHACL policies and engine version.
5. **One npm package with native prebuilds and WASM.** The release workflow stages a single `@a5i/docvera` package containing all four native platform binaries plus the Node/browser WASM runtime behind `./wasm`. Staged npm publishing requires one manual 2FA approval.

## In-memory projection instead of an index

There is no derived database. Each retrieval command loads the workspace, validates it and builds an in-memory projection: GlueSQL tables (`documents`, `sections`, `nodes`, `relations`, `diagnostics` with structured `MAP` `shacl` values), the relation graph, or section text. See [in-memory SQL](in-memory-sql.md) for the exact projection.

Document internal IDs are normalized relative paths; declared IDs can collide. Section IDs are `path#generated-anchor`, stable across content edits but not heading renames/reordering of duplicates. Section spans cover nested descendants; `text` contains **direct content only** to avoid duplicating descendant hits. Files with no headings produce no section rows.

Because every command re-reads current files there is no writer contention, stale snapshot or rebuild command. Large workspaces pay load/validate cost per command; the owned `CheckerContext`/`openChecker` baseline exists precisely for editor loops that would otherwise rescan repeatedly.

## Safety

- Schema selection and local path references are confined to the canonical workspace root; symlink discovery is disabled. Static path confinement is not a defense against a hostile process racing symlink replacement.
- JSON Schema remote/local external resource retrieval is disabled at compile time; internal references work.
- User SQL is restricted to exactly one read-only SELECT over the five public tables; writes, DDL, transactions and multi-statement strings are rejected, and the storage layer itself rejects mutations. There is no file/attach/pragma surface.
- At most 1,000 output rows are returned, checked after evaluation; there is **no execution deadline or memory ceiling**. SQL is trusted-input automation, not an OS-isolated sandbox for hostile workloads.
- Parse/config/filesystem failures become either document diagnostics or operation errors. Included-file read failures stop the operation instead of pretending the file was deleted.
- SHACL policies are workspace-trusted, local-only inputs, not a sandbox. Imports and SPARQL SERVICE/FROM clauses are rejected conservatively; resource limits/process isolation and performance baselines remain future work.
- No network access, external URL checks, arbitrary structure auto-fixing, or remote crawling.

## Contracts and remaining work

Result envelopes have `contractVersion: 1`. `docvera schema` is an intentional exception: it emits a bare JSON Schema 2020-12 with a versioned URN `$id`. `contracts.rs` generates config/document-envelope contracts for deserialization and CheckResult/Diagnostic contracts for serialization using `schemars` derives on the same Rust types used by Serde. `docvera contract` and JSON `guide` embed these under `jsonSchemas`; command prose/exit semantics still come from the descriptor. Rust semantic checks remain authoritative and are not automatically inferred. Query/graph/extract still use dynamic JSON values with parity tests; TypeScript facade interfaces remain hand-maintained. Migrating those dynamic results and generating TS declarations is deferred.

The prototype covers the breadth of PoC stages A–F, but not every proposed extension. Deferred: body `$ref`/alternatives, full SHACL conformance, SHACL resource isolation and benchmarks, general completeness/freshness states, transition validation, query-exposure settings, full GFM AST persistence, semantic empty-section analysis, advanced block/table schemas, dedicated cycle/path query APIs, multilingual stemming, code-specific ranking, combined retrieval endpoints, watch mode, external URL checks, CSV/table output, hosted schema registry, and automatic npm prebuild distribution.

## Verification

Rust integration tests exercise source validation, IR boundaries, schema changes, indexing edits/deletions, graph cycles and SQL restrictions. CLI integration tests exercise JSON purity, errors/exit codes, init non-overwrite, warnings-as-errors and the complete demo flow. Native Node tests call the binding directly and compare JSON contracts with CLI output, including edit/delete lifecycle behavior and close semantics.

GitHub Actions is configured for Linux x64/arm64, macOS x64/arm64 and Windows x64. Each runner runs Rust checks/tests, Node parity tests and builds release CLI/native npm artifacts. These jobs need to run in GitHub before those targets can be called verified. Local development verification is Linux x64.
