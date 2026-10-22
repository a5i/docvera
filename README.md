# docvera / Markdown Knowledge Engine

A cross-platform **Rust prototype** that turns Git-managed Markdown into a validated, typed, queryable local knowledge base. The same engine powers the `docvera` CLI and a native Node.js addon; Node does not spawn the CLI.

**Implemented:** YAML/JSON schemas, section-tree validation, lint warnings, ID uniqueness, typed metadata relations, SHACL Core/SPARQL cross-document policies, Markdown links/anchors, read-only GlueSQL over the current workspace, graph traversal and original Markdown extraction.

**Prototype choices:** pure-Rust GlueSQL instead of SQLite/DuckDB, and pulldown-cmark instead of comrak. No database server, bundled C library or downloadable extensions. See [architecture and limitations](docs/architecture.md), [schema language](crates/docvera-cli/assets/schema.md), and [v1 command contracts](crates/docvera-cli/assets/contracts/v1.json). The [original proposal](docs/input/markdown_knowledge_engine_poc_en-2.md) is a roadmap, not a claim that every proposed capability is implemented.

## Install

Released versions are published from a `v*` tag by the release workflow:

```sh
cargo install --locked docvera-cli # CLI from crates.io
npm install @a5i/docvera           # native Node.js addon (prebuilt per platform)
```

CLI binaries for Linux x64/arm64, macOS arm64 and Windows x64 are attached
to each [GitHub Release](https://github.com/a5i/docvera/releases) with
SHA-256 checksums. Like `eregex`, the single `@a5i/docvera` npm package contains
prebuilt `.node` binaries for all four platforms; its loader selects the matching
binary at runtime. No separate platform packages or optional dependencies are
needed. Building locally still works as shown in the [Node.js API](#nodejs-api) section.
Intel macOS is no longer built or tested in CI and has no prebuilt native addon; use the explicit `@a5i/docvera/wasm` entry for the portable in-memory API.

## Install from source

Install the CLI into `~/.cargo/bin` (assumed to be on `PATH`):

```sh
cargo install --locked --path crates/docvera-cli
docvera --version   # docvera 0.1.1
```

Updating is the same command re-run over the existing installation. A local build without installing works too: `cargo build --locked --release -p docvera-cli`, then use `target/release/docvera` (Windows: `target\release\docvera.exe`).

Requirements: Rust 1.89 or newer (current stable recommended). No C/C++ compiler is needed: the engine is pure Rust, including its GlueSQL query layer. Crates are downloaded once from the pinned `Cargo.lock`, including the Rudof SHACL engine. The native Node.js addon is published to npm as `@a5i/docvera` with all prebuilt platform binaries in one package; to build it from source instead, see the Node.js API section below.

## Migrating from md-schema / mdx

This is a breaking rename with no old-name aliases. Install the new `docvera` CLI;
rename workspace `mdx.toml` / `mdx.yaml` to `docvera.toml` / `docvera.yaml`.
Update scripts to use `docvera`, native parity overrides to `DOCVERA_BIN`, and
Node imports to `@a5i/docvera` (local source package: `npm/docvera`). Rust
packages are now `docvera-engine`, `docvera-cli`, and `docvera-node`.
Replace `urn:mdx:` with `urn:docvera:` in SHACL policies and schema identifiers.
There is no derived database file to rebuild. Remove any leftover `.mdx` or
`.docvera` directory from older versions. Markdown documents and document schemas
otherwise keep their existing format.

## Try the entire cycle

Prerequisites: see [Install from source](#install-from-source). Run the full cycle without installing:

```sh
cargo build --locked --release -p docvera-cli
cargo run --locked --bin docvera -- --root demo init
cargo run --locked --bin docvera -- --root demo check
cargo run --locked --bin docvera -- --root demo query sql "SELECT declared_id,type FROM documents ORDER BY path" --format json
cargo run --locked --bin docvera -- --root demo graph incoming FEATURE-0042 --depth 2
cargo run --locked --bin docvera -- --root demo extract "docs/feature-0042.md#security"
```

Or run the binary directly: `target/release/docvera` (Windows: `target\release\docvera.exe`). `init` creates a small workspace containing an ADR, a Feature and a Test Plan. It refuses to overwrite existing files. The tracked version is in [`examples/workspace`](crates/docvera-engine/assets/workspace).

### Exercise failures and updates

1. Rename `## Context` in `demo/docs/adr-0001.md`: `check` reports unexpected/missing sections and exits **1**.
2. Restore it; replace `OAuth` with `OpenID` in the feature and re-run the SQL query: the change is visible immediately.
3. Delete the feature: inbound references become diagnostics; SQL no longer lists the document.
4. Run `check --format json` to get structured errors with source spans.
Markdown remains authoritative; there is no derived content to regenerate.

Every command reads current files; there is **no derived snapshot or database**. `query`/`graph`/`extract` project the validated workspace in memory. Use `check` as the CI gate; it is the same validation that feeds SQL diagnostics.

## Role-based examples

The six [executable use-case workspaces](examples/use-cases/README.md) cover product
traceability, project governance with offline snapshots, architecture governance,
QA coverage, operational readiness and SHACL design invariants. Each includes schemas, Markdown documents,
SQL reports and expected results, and doubles as an integration-test fixture.
They distinguish current validation from reporting-only policies and deferred features.

## Describe document structure

Schemas are YAML or JSON. Front matter uses real JSON Schema; the body uses a small, deliberately separate section DSL:

```yaml
schemaVersion: 1
documentType: guide
frontMatter:
  type: object
  required: [id, title]
  properties:
    id: { type: string }
    title: { type: string }
  additionalProperties: false
identity: { field: frontMatter.id, uniqueWithin: workspace }
body:
  sections:
    - titleEquals: frontMatter.title  # one H1, matching metadata
      children:
        sections:
          - title: Installation     # mandatory H2
            nonEmpty: true
            children:
              sections:
                - title: Linux      # optional H3
                  min: 0
                  content:
                    - { kind: code, language: bash, min: 1 }
          - title: Usage            # mandatory H2, after Installation
```

Map files to schemas in `docvera.toml`, or set `$schema` in a document's front matter. Explicit selectors are relative to the document; mappings are relative to the workspace. The reserved `$schema` selector is removed before metadata validation, so `additionalProperties: false` works normally.

## Add cross-document policies

Keep configuration in `docvera.toml` and standard SHACL rules in separate Turtle files:

```toml
[validation.shacl]
shapes = ["policies/relations.ttl", "policies/lifecycle.ttl"]
```

`check` builds an in-memory RDF projection from current documents and resolved references, runs pinned Rudof SHACL Native validation (Core plus tested inline SPARQL), and maps violations back to Markdown diagnostics. No persistent RDF store or database is required. See [policy examples, RDF vocabulary and limits](crates/docvera-cli/assets/shacl.md) and the runnable [design-invariants workspace](examples/use-cases/design-invariants).

`docvera init` and tracked examples use TOML. Legacy `docvera.yaml` remains a migration fallback; having both configs is an error. Local document schemas remain YAML/JSON. There is no custom `constraints` assertion DSL.

## Node.js API

Published to npm as [`@a5i/docvera`](https://www.npmjs.com/package/@a5i/docvera). A single package ships all supported platform binaries and selects the matching addon at runtime, without additional npm dependencies. To build the addon locally instead, Node.js 22+ and the Rust/C toolchain are required:

```sh
npm --prefix npm/docvera run build
```

```js
const { openWorkspace } = require('@a5i/docvera');

async function main() {
  const engine = await openWorkspace({ root: './demo' });
  try {
    const check = await engine.check();
    const rows = await engine.querySql(
      "SELECT declared_id, metadata['status'] AS status FROM documents WHERE type = 'adr'"
    );
    const graph = await engine.references({ id: 'FEATURE-0042', direction: 'incoming', depth: 2 });
    console.log({ check, rows, graph });
  } finally {
    await engine.close();
  }
}
main().catch(console.error);
```

File and validation work runs on Rust blocking workers, not the Node event loop. API methods return the same versioned JSON objects as the CLI. TypeScript declarations are included. `openWorkspace` is lazy: every operation re-reads current files. `close()` rejects new operations and waits for active work.

For unsaved editor buffers, use `openChecker({ root })` and `checkDocument(path, text)` to work against an owned in-memory baseline instead of rescanning disk. Rust also exposes `CheckerContext::from_sources` for a fully in-memory workspace. See [checker API, snapshot semantics, parser/front-matter options and checker-only/WASM builds](docs/checker.md).

Browser/WASM bindings ship in the same npm package as **`@a5i/docvera/wasm`**. Use `await openChecker({ config, sources })` with caller-supplied Markdown/schema text; WASM initialization and resource cleanup are handled by the Promise-based API. The engine's `sql` feature also exposes a Rust API. See [API, build instructions and limitations](docs/in-memory-sql.md).

## CLI contracts

```sh
docvera --help
docvera guide                     # complete offline manual with examples + schema/SHACL references
docvera schema config > docvera.config.schema.json      # generated from Rust/Serde types
docvera schema document > docvera.document.schema.json  # envelope/section DSL contract
docvera schema check-result                        # serialized checker result contract
docvera guide --format json       # same manual and command contracts for AI agents
docvera contract --format json
docvera check --warnings-as-errors --format json
```

- `--root` and `--format` are global and can appear before/after a subcommand.
- `json`: one versioned result object (except `schema`, which emits a bare JSON Schema); `jsonl`: the same envelope on one compact line.
- Exit **0**: success; **1**: validation failed; **2**: operational/usage error.
- Lint and SHACL warnings do not fail checks unless `--warnings-as-errors` is set.
- SQL: GlueSQL dialect over the current workspace, five public tables (`documents`, `sections`, `nodes`, `relations`, `diagnostics`), structured `MAP`/`LIST` values via `metadata['key']`, writes/multiple statements rejected, 1,000-row cap (checked after evaluation; no execution deadline). Treat SQL as trusted input, **not a memory-isolated adversarial sandbox**.
- Search text is a literal phrase, not an FTS expression. Results have positive relevance scores, source spans and deterministic tie-breaking.
- Spans use zero-based UTF-8 byte offsets (exclusive end); line/column coordinates are one-based Unicode scalar positions.

## Develop and verify

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked -p docvera-cli
npm --prefix npm/docvera run build
npm --prefix npm/docvera test
```

The Rust integration suite covers valid/invalid structure, metadata, duplicate YAML keys, duplicate IDs, typed relations, anchors, Unicode/CRLF, schema invalidation, updates/deletions, SQL restrictions and graph cycles. Node tests compare CLI/native results across the full lifecycle.

GitHub Actions runs these checks on Linux x64/arm64, macOS x64/arm64 and Windows x64, then builds release CLI/native npm artifacts. These are CI targets, not a claim that remote CI has already passed. Local validation was performed on Linux x64. No npm dependencies are needed for the facade or tests.

Supply-chain checks — RustSec advisories (`cargo audit`), plus cargo-deny advisories/licenses/bans/sources — run locally with:

```sh
cargo audit          # config: .cargo/audit.toml
cargo deny check     # config: deny.toml
```

Both enforce deny-by-default with narrowly-scoped, reason-annotated ignores for advisories currently unfixable upstream (quick-xml 0.37 DoS pair pinned by oxigraph/rudof; two unmaintained transitive crates inside `rudof_rdf`). When upgrading `rudof`/`oxigraph`, remove matching ignore entries and re-run. CI runs the same two commands on dependency changes and weekly (`.github/workflows/audit.yml`) to catch advisories published after the code last changed.
