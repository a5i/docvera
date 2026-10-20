# md-schema / Markdown Knowledge Engine

A cross-platform **Rust prototype** that turns Git-managed Markdown into a validated, typed, queryable local knowledge base. The same engine powers the `mdx` CLI and a native Node.js addon; Node does not spawn the CLI.

**Implemented:** YAML/JSON schemas, section-tree validation, lint warnings, ID uniqueness, typed metadata relations, Markdown links/anchors, incremental index writes, read-only SQL, graph traversal, section-level BM25 search and original Markdown extraction.

**Prototype choices:** SQLite + bundled FTS5 instead of DuckDB, and pulldown-cmark instead of comrak. No database server or downloadable extensions. See [architecture and limitations](docs/architecture.md), [schema language](docs/schema.md), and [v1 command contracts](docs/contracts/v1.json). The [original proposal](docs/input/markdown_knowledge_engine_poc_en-2.md) is a roadmap, not a claim that every proposed capability is implemented.

## Try the entire cycle

Install a current stable Rust toolchain and a C/C++ compiler (SQLite is compiled from bundled source). Windows: Visual Studio Build Tools with the C++ workload. macOS: Xcode command-line tools. Linux: GCC/Clang and standard build tools.

```sh
cargo build --locked --release -p md-cli
cargo run --locked --bin mdx -- --root demo init
cargo run --locked --bin mdx -- --root demo check
cargo run --locked --bin mdx -- --root demo index
cargo run --locked --bin mdx -- --root demo query sql "SELECT declared_id,type FROM documents ORDER BY path" --format json
cargo run --locked --bin mdx -- --root demo graph incoming FEATURE-0042 --depth 2
cargo run --locked --bin mdx -- --root demo search OAuth --type feature --section Security
cargo run --locked --bin mdx -- --root demo extract "docs/feature-0042.md#security"
```

Or run the binary directly: `target/release/mdx` (Windows: `target\release\mdx.exe`). `init` creates a small workspace containing an ADR, a Feature and a Test Plan. It refuses to overwrite existing files. The tracked version is in [`examples/workspace`](examples/workspace).

### Exercise failures and updates

1. Rename `## Context` in `demo/docs/adr-0001.md`: `check` reports unexpected/missing sections and exits **1**.
2. Restore it; replace `OAuth` with `OpenID` in the feature. Run `index`: one document is updated; search for `OpenID` finds the changed section.
3. Delete the feature and run `index`: the document and its search entries disappear, inbound references become diagnostics.
4. Run `check --format json` to get structured errors with source spans.
5. Run `index --rebuild` at any time to regenerate derived content. Markdown remains authoritative.

`check` reads current files and requires **no database**. `query`, `graph`, `search` and `extract` read the **last indexed snapshot**; index explicitly after changes. `index` retains invalid documents so diagnostics and duplicates remain inspectable; use `check` as the CI gate.

## Role-based examples

The five [executable use-case workspaces](examples/use-cases/README.md) cover product
traceability, project governance with offline snapshots, architecture governance,
QA coverage and operational readiness. Each includes schemas, Markdown documents,
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

Map files to schemas in `mdx.yaml`, or set `$schema` in a document's front matter. Explicit selectors are relative to the document; mappings are relative to the workspace. The reserved `$schema` selector is removed before metadata validation, so `additionalProperties: false` works normally.

## Node.js API

Requires Node.js 22+ and the Rust/C toolchain **for the current local build**. There is no published npm package or automatic prebuilt-platform loader yet.

```sh
npm --prefix npm/mdx run build
```

```js
const { openWorkspace } = require('./npm/mdx');

async function main() {
  const engine = await openWorkspace({ root: './demo' });
  try {
    const check = await engine.check();
    await engine.index();
    const rows = await engine.querySql('SELECT declared_id,type FROM documents ORDER BY path');
    const hits = await engine.search({ text: 'OAuth', documentType: 'feature' });
    const graph = await engine.references({ id: 'FEATURE-0042', direction: 'incoming', depth: 2 });
    console.log({ check, rows, hits, graph });
  } finally {
    await engine.close();
  }
}
main().catch(console.error);
```

CPU and file/database work runs on Rust blocking workers, not the Node event loop. API methods return the same versioned JSON objects as the CLI. TypeScript declarations are included. Construction is lazy; operations open their own database connections. `close()` rejects new operations and waits for active work.

## CLI contracts

```sh
mdx --help
mdx contract --format json
mdx check --warnings-as-errors --format json
```

- `--root` and `--format` are global and can appear before/after a subcommand.
- `json`: one versioned result object; `jsonl`: the same envelope on one compact line.
- Exit **0**: success; **1**: validation failed; **2**: operational/usage error.
- Lint warnings do not fail checks unless `--warnings-as-errors` is set.
- SQL: SQLite syntax, five public tables (`documents`, `sections`, `nodes`, `relations`, `diagnostics`), allowlisted functions, no writes/ATTACH/PRAGMA/extension loading, 1,000-row cap and approximately two-second VM deadline. This is defense in depth, **not a memory-isolated adversarial SQL sandbox**.
- Search text is a literal phrase, not an FTS expression. Results have positive relevance scores, source spans and deterministic tie-breaking.
- Spans use zero-based UTF-8 byte offsets (exclusive end); line/column coordinates are one-based Unicode scalar positions.

## Develop and verify

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked -p md-cli
npm --prefix npm/mdx run build
npm --prefix npm/mdx test
```

The Rust integration suite covers valid/invalid structure, metadata, duplicate YAML keys, duplicate IDs, typed relations, anchors, Unicode/CRLF, schema invalidation, updates/deletions, SQL restrictions and graph cycles. Node tests compare CLI/native results across the full lifecycle.

GitHub Actions runs these checks on Linux x64/arm64, macOS x64/arm64 and Windows x64, then builds release CLI/native npm artifacts. These are CI targets, not a claim that remote CI has already passed. Local validation was performed on Linux x64. No npm dependencies are needed for the facade or tests.
