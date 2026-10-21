# Embedded checker and unsaved buffers

The Rust checker can run without SQLite, FTS or RDF. CLI `docvera check` and the existing Node `openWorkspace().check()` still load current files on every call. Editor integrations should use the explicit reusable context below instead.

## Rust API

```rust,no_run
use docvera_engine::CheckerContext;
use std::path::Path;

let mut checker = CheckerContext::load(Path::new("./workspace"))?;
let text = "---\nid: NOTE-1\n---\n# Unsaved heading\n";
let result = checker.check_document("docs/note.md", text)?;
// Inspect consequences for inbound references and workspace policies too:
let affected_workspace = checker.check_with_text("docs/note.md", text)?;
// Commit a buffer to the in-memory baseline, not to disk:
checker.update_document("docs/note.md", text.to_owned())?;
checker.remove_document("docs/deleted.md")?;
let result = checker.check()?;
# Ok::<(), anyhow::Error>(())
```

`load` reads configuration, source documents, non-Markdown asset identities and schemas once, and compiles the schemas and optional SHACL shapes. Only the edited document is reparsed and locally revalidated for an overlay; other documents reuse their parsed, locally validated state. Cross-document identity/reference checks and optional SHACL validation are rerun in memory over the resulting snapshot. This is **not** a dependency-incremental or bounded-latency SHACL engine.

- `check_document(path, text)` returns the same diagnostics as a workspace overlay check, restricted to `path`; it does not mutate the baseline. An existing document is replaced, including its old ID/type/anchors. A new unsaved path is supported.
- `check_with_text(path, text)` returns the complete workspace result with that one overlay, including affected diagnostics in other documents.
- `check()` validates the baseline snapshot. `update_document` and `remove_document` explicitly change that baseline without disk I/O. For multiple unsaved buffers, update the baseline with their current texts before checking.
- Paths are portable workspace-relative paths with `/`. `.` and safe `..` components normalize; absolute paths, backslashes, drive prefixes and workspace escapes are rejected. Schema selectors are relative to the document, mappings to the workspace.
- Includes, excludes and reserved skipped directories still apply. `check_document` returns `documents: 0` for an excluded path, otherwise `documents: 1`, even if validation is skipped for an untyped document.
- Recreate the context when configuration, shape files or asset identities change. For schema text changes use `update_schema(path, text)` (Node `updateSchema`) or recreate the context; changing a schema recompiles it and revalidates cached local documents. Invalid schema updates produce `schema.resolve` diagnostics, never stale success. `load` preloads mapped schemas and explicit selectors in current documents. Register a new unsaved selector's schema with `update_schema`, or include it in `from_sources`; checking itself never loads missing schema files.
- After construction, checking and document updates do not access the filesystem, create an index, or modify source files. Asset existence and symlink confinement reflect the captured snapshot, not subsequent filesystem changes.
- Invalid schemas are document `schema.resolve` diagnostics when selected. Invalid config, unreadable included documents, or SHACL setup/evaluation failures remain operation errors. Diagnostics retain deterministic ordering.

## Construction without a filesystem

Use `CheckerContext::from_sources(config, sources)` for a host-managed workspace, including WebAssembly. `config` is `docvera_engine::config::Config` (serde-deserializable); `sources` is `WorkspaceSources`:

| Field | Value |
|---|---|
| `documents` | `BTreeMap<String, String>`: document path → current source text |
| `schemas` | `BTreeMap<String, String>`: schema path → YAML/JSON envelope text |
| `shapes` | `BTreeMap<String, String>`: configured Turtle path → text; requires `shacl` feature |
| `assets` | `BTreeSet<String>`: existing non-Markdown asset paths; contents are unnecessary |

Schema and document keys normalize to workspace-relative paths; supply one entry per logical path. External resource retrieval by JSON Schema is disabled. Internal `$ref`/`$defs` work as in CLI checks. No database-backed ID catalogue is required: IDs, document types and section anchors come from the source documents in the snapshot.

## Node API

```js
const { openChecker } = require('./npm/docvera');
const checker = await openChecker({ root: './workspace' });
try {
  const result = await checker.checkDocument('docs/note.md', unsavedText);
  const affected = await checker.checkWithText('docs/note.md', unsavedText);
  await checker.updateDocument('docs/note.md', unsavedText);
  await checker.removeDocument('docs/deleted.md');
  const all = await checker.check();
} finally {
  await checker.close();
}
```

Unlike lazy `openWorkspace`, `openChecker` loads its snapshot before resolving. Native work runs on blocking workers and operations on a checker are serialized. Await baseline updates before dependent checks; do not rely on invocation order of concurrent operations. `close()` rejects new operations, waits for active work and releases the context. Reopen after config/shapes/assets change; update schema text explicitly with `updateSchema`.

Results use the existing `CheckResult` contract. Offsets are zero-based **UTF-8 byte offsets**, end exclusive; lines/columns are one-based Unicode scalar positions, not JavaScript UTF-16/LSP positions. An editor adapter must convert them against the same buffer revision. No LSP server, debounce, cancellation or filesystem watcher is provided by this API.

## Build features and WASM

`docvera-engine` keeps full functionality by default:

| Features | Capabilities |
|---|---|
| defaults (`index`, `shacl`) | Existing complete engine |
| `--no-default-features` | Parser, local schemas, lint, identities, links and typed relations; no SQLite/RDF dependencies |
| `--no-default-features --features index` | Add SQLite/FTS and retrieval, without SHACL |
| `--no-default-features --features shacl` | Add in-memory RDF policies, without SQLite |

A checker without the `shacl` feature **fails closed** if `[validation.shacl]` is configured; it never silently skips requested policies. `docvera-cli` and `docvera-node` currently use the default full engine.

```sh
cargo test --locked -p docvera-engine --no-default-features
cargo test --locked -p docvera-engine --no-default-features --features index
cargo test --locked -p docvera-engine --no-default-features --features shacl
rustup target add wasm32-unknown-unknown
cargo build --locked -p docvera-engine --no-default-features --target wasm32-unknown-unknown
```

The final command verifies compilation/code generation of the Rust checker core for WASM. Use `from_sources`, not filesystem loading, in a browser host. This is **not yet a packaged browser JavaScript binding or a browser runtime test**, and does not claim WASM support for the optional index/RDF stacks. Their absence from the checker dependency graph can be inspected with `cargo tree -p docvera-engine --no-default-features --edges normal`.

## Parsing and untyped files

See [schema configuration](schema.md#resolution-and-workspace) for `[parser]`, `[frontMatter]` and `[validation].untyped`. All entry points use the same settings. Parser options and front-matter settings participate in the index invalidation fingerprint. Source text is never normalized or rewritten: BOM, line endings and original bytes remain in the document and its hash.

pulldown-cmark is a crates.io dependency (`0.13`; the repository lockfile pins `0.13.4`). A Rust consumer controlling the root Cargo workspace can override it via `[patch.crates-io]`. Precompiled Node binaries require rebuilding to incorporate such a patch. Known upstream range defects are not repaired by this API; metadata is handled separately with parser-owned YAML/TOML locations and whole-front-matter fallbacks where a precise location is unavailable.
