# Markdown Knowledge Engine — Proof of Concept

Proof of Concept \| Product & Technical Specification \| 20 October 2026

Status: design proposal. Schema DSL, CLI commands, and programming examples describe proposed PoC contracts, not already implemented capabilities.

# 1. Business objectives: what, why, and for whom

## 1.1 Problem

Markdown is convenient for people and Git, but large documentation repositories become inconsistent: document structures diverge, metadata is incomplete, IDs collide, references break, and relevant information is difficult to retrieve programmatically. Conventional Markdown linters do not enforce document types, cross-document relationships, or structured queries over content.

## 1.2 Product value

A local-first, agent-friendly engine transforms Markdown files into a validated, typed, queryable knowledge base without moving the source of truth out of Git-managed files. The same core capabilities are exposed through a Rust API, CLI, and npm package for Node.js.

| Business goal    | PoC capability                                                     | Why it matters                                                   |
|------------------|--------------------------------------------------------------------|------------------------------------------------------------------|
| Standardization  | Validate YAML front matter and section hierarchy                   | Consistent ADRs, RFCs, API documents, and feature specifications |
| Integrity        | Check IDs, links, target document types and anchors                | Fewer broken or contradictory references                         |
| Knowledge access | SQL, graph traversal, full-text search over documents and sections | Tools and agents retrieve specific facts instead of entire files |
| Automation       | CLI, Node.js API, JSON/JSONL output                                | Reusable contracts for scripts and CI                            |
| Local-first      | Markdown stays in Git; index can be rebuilt                        | Portable and infrastructure-light                                |

## 1.3 Users and scenarios

- Developer: validate specifications before merge, with file/line/column diagnostics.

- Technical lead: list accepted ADRs associated with a feature and its dependencies.

- AI agent: inspect machine-readable command contracts, retrieve JSON fragments, and verify references.

- Node.js automation: index and query directly, without spawning a CLI or parsing stdout.

## 1.4 Measurable PoC acceptance

- Fixture repository with several document types (ADR, Feature, Test Plan).

- Successful and failing schema checks, including cross-file ID collisions.

- Detection of missing links, incorrect target types, and missing anchors.

- SQL query over metadata/sections, graph traversal, and BM25 search over sections.

- Equivalent typed results from the CLI JSON output and Node.js API.

- An edit updates affected index entries; a deletion invalidates inbound references.

## 1.5 Out of scope

Arbitrary structure auto-fixing; remote multi-user servers or collaborative editing; embeddings/vector search; full GQL or SQL/PGQ compatibility; unrestricted external crawling; hosted schema registry; and every Markdown dialect. Architectural extension points should remain available.

# 2. How it works: document model and schemas

## 2.1 Unified document model

Each file consists of YAML front matter and a Markdown body. Parsing builds a Document IR containing path, internal technical key, declared document ID and type, metadata, logical sections, AST nodes, code blocks, tables, links, source spans, and diagnostics. A section begins at its heading and ends before the next heading of the same or higher level.

## 2.2 Schema resolution

PoC precedence: explicit $schema in front matter → workspace glob-to-schema mapping → “schema unresolved” error when the file requires a schema. Workspace settings define the root, included files, schema directories, document types, excludes, and index location. Schemas are versioned; remote references are disabled by default in the PoC.

## 2.3 Key schema elements

| Schema element | Contract                                                                                        |
|----------------|-------------------------------------------------------------------------------------------------|
| frontMatter    | JSON Schema: required/optional fields, types, enum, patterns, uniqueItems, additionalProperties |
| body           | Section order, nesting, cardinality, permitted Markdown nodes and content                       |
| relations      | Link source (metadata or body), target type, min/max, ID/path/anchor resolution                 |
| constraints    | Workspace uniqueness and cross-field / cross-document assertions                                |
| lint           | Formatting/style rules separate from schema failures                                            |
| queryExposure  | Optional fields, sections, and relations exposed to indexing and search                         |

## 2.4 Example Markdown document

```markdown
---
$schema: ./schemas/adr.schema.yaml
id: ADR-0001
title: Local-first storage
status: accepted
related: [FEATURE-0042]
tags: [architecture, offline]
---

# Local-first storage

## Context
Existing data must remain available offline.

## Decision
Store authoritative content in local files.

## Consequences
Index and synchronization become derived concerns.
```

## 2.5 Conceptual document schema

```yaml
schemaVersion: 1
documentType: adr
frontMatter:
  type: object
  required: [id, title, status]
  properties:
    id: { type: string, pattern: '^ADR-[0-9]{4}$' }
    title: { type: string, minLength: 5 }
    status: { enum: [proposed, accepted, deprecated] }
    related:
      type: array
      items: { type: string }
    tags:
      type: array
      uniqueItems: true
      items: { type: string }
  additionalProperties: false
identity:
  field: frontMatter.id
  uniqueWithin: documentType
body:
  titleEquals: frontMatter.title
  orderedSections:
    - { title: Context, required: true }
    - { title: Decision, required: true }
    - { title: Consequences, required: true }
relations:
  - from: frontMatter.related
    targetType: feature
    mustExist: true
```

The identity, titleEquals, orderedSections, and relations keys are proposed extensions, not JSON Schema keywords. Their semantics and diagnostics must be specified. Front matter $schema selects the document schema; JSON Schema $ref remains reserved for schema composition.

## 2.6 Links, identity, and validation

- Resolve relative paths, heading anchors, and logical document IDs; external URL reachability is optional and separate.

- Validate typed relations: presence, cardinality, target document type, and optional reciprocal relations.

- Check declared ID uniqueness within a workspace, directory, or document type. Use an independent internal key so duplicates remain indexable and reportable.

- Report malformed YAML, schema mismatches, unresolved references, and broken constraints as diagnostics with file, span, rule, and severity.

- Scan the workspace initially for global uniqueness; subsequently invalidate only affected files and reverse dependencies.

## 2.7 Processing pipeline

Discover files → parse YAML/Markdown → construct Document IR → validate local schema → update catalog → resolve links and uniqueness → check graph constraints → update search index → report diagnostics. Markdown files remain authoritative; DuckDB is a disposable derived index. Content hashes and schema versions determine invalidation.

# 3. Search, queries, and structured extraction

## 3.1 Query interfaces

| Interface        | Capability                                                          | Output                        |
|------------------|---------------------------------------------------------------------|-------------------------------|
| SQL              | Filter front matter, join documents/sections/relations, aggregate   | Rows / objects                |
| Graph API        | Incoming/outgoing references, cycles, dependency chains, traversal  | Nodes, edges, paths           |
| Full-text search | Title/heading/body/code searches, BM25 ranking                      | Ranked hits with source spans |
| Combined queries | Full-text results filtered by SQL properties and graph neighborhood | Contextual knowledge slices   |

## 3.2 Logical index

documents(internal_id, declared_id, type, path, metadata, hash); sections(section_id, document_id, heading, level, parent_id, ordinal, text, span); nodes(node_id, section_id, kind, data, span); relations(source_document_id, target_document_id, kind, source_span, resolution_status); diagnostics(document_id, rule, severity, span, message). Search rows store searchable text independently from the full source.

## 3.3 Section-level full-text retrieval

Index at least each logical section, plus document title and selected metadata. A hit carries declared ID, section path, snippet, BM25 score, file path and start/end positions. Distinguish headings, prose and code. In the PoC, title/heading/body fields are sufficient; callers may retrieve the original Markdown source range by section ID.

## 3.4 Illustrative CLI

```bash
mdx check --format json

mdx index --rebuild

mdx query sql "SELECT declared_id, metadata->>'status' FROM documents" --format json

mdx graph incoming ADR-0001 --depth 2 --format json

mdx search "OAuth" --type adr --section Security --format json
```

## 3.5 Example JSON search response

```json
{
  "results": [
    {
      "documentId": "ADR-0001",
      "section": "Decision",
      "path": "docs/adr/0001.md",
      "location": {
        "line": 12,
        "column": 1
      },
      "score": 4.21,
      "snippet": "Use OAuth 2.0 with PKCE."
    }
  ]
}
```

The score is illustrative. Search uses relevance ordering; SQL only guarantees a specified order with ORDER BY. JSON results should have versioned contracts. CLI log messages and warnings must not contaminate machine-readable stdout.

## 3.6 Query and search implementation constraints

Verify DuckDB FTS capabilities and incremental updates against exact engine/extension versions. Rebuilding the FTS index after a changed batch is acceptable in the PoC. Keep md-search behind an interface to permit Tantivy later. Implement the first graph traversal layer with recursive SQL CTEs; DuckPGQ remains optional pending compatibility verification. Treat user-provided SQL as untrusted and expose a deliberately constrained read-only query surface.

# 4. Technology stack and integrations

| Layer            | Rust technology                        | Notes / verification                                                      |
|------------------|----------------------------------------|---------------------------------------------------------------------------|
| CLI and contract | argx (evaluate) or clap + schemars     | clap_schema is deprecated; CLI schemas are distinct from document schemas |
| Types / JSON     | serde, serde_json, schemars            | Typed API/results and serialization                                       |
| Markdown parser  | comrak                                 | CommonMark/GFM AST and source positions                                   |
| YAML parser      | serde-saphyr                           | Duplicate-key handling for front matter                                   |
| Schema validator | jsonschema                             | JSON Schema reuse plus custom document constraints                        |
| Storage / SQL    | duckdb-rs + DuckDB                     | Embedded derived index; confirm DuckDB 2.0 binding compatibility          |
| Graph            | recursive CTE; petgraph                | No mandatory separate graph database                                      |
| FTS              | DuckDB FTS; optional Tantivy           | Validate version and extension matrix                                     |
| Indexing         | ignore, globset, blake3, rayon, notify | Discovery, hashing, parallelism, watch mode                               |
| Diagnostics      | miette, thiserror, tracing             | File spans, errors, instrumentation                                       |
| Output           | serde_json, csv, tabled                | Consistent JSON, JSONL, CSV, human tables                                 |
| Node.js / npm    | napi-rs, napi-derive, @napi-rs/cli     | Native addon with generated TypeScript declarations                       |
| Testing          | insta, snapbox, proptest               | Snapshots, CLI contracts, property testing                                |

## 4.1 Node.js integration: native library, not a CLI wrapper

Publish a scoped npm package (placeholder: @example/mdx) with a thin TypeScript facade calling the shared Rust md-engine through napi-rs and Node-API. Platform-native prebuilt packages cover supported Windows, Linux and macOS combinations (verify x64/arm64 coverage in CI). Rust is not required on consumer machines with supported prebuilds. Do not make child_process CLI invocation the default binding mechanism.

## 4.2 Proposed TypeScript API

```typescript
import { openWorkspace } from '@example/mdx';



const engine = await openWorkspace({ root: process.cwd() });

await engine.index({ incremental: true });

const diagnostics = await engine.check();

const rows = await engine.querySql(

"SELECT declared_id, type FROM documents WHERE type = 'adr'"

);

const incoming = await engine.references({ id: 'ADR-0001', direction: 'incoming' });

const hits = await engine.search({ text: 'OAuth', documentType: 'adr' });

await engine.close();
```

This is a proposed API, not implemented code. CPU-intensive and I/O-heavy operations must not block the Node.js event loop. TypeScript declarations and runtime structures should be generated from stable Rust contracts. Validation, indexing and query logic live only in the shared Rust core.

## 4.3 Proposed Cargo workspace

```text
crates/

md-model/ md-parser/ md-schema/ md-validator/

md-references/ md-index/ md-query/ md-search/

md-engine/ md-cli/ md-node/

npm/

mdx/ # JS loader, declarations, package metadata
```

## 4.4 PoC stages and acceptance criteria

| Stage          | Deliverable                             | Acceptance criterion                                            |
|----------------|-----------------------------------------|-----------------------------------------------------------------|
| A. Schema      | Front matter + H1/H2/section validation | Required, optional, ordering, and type diagnostics              |
| B. Relations   | ID registry and link resolver           | Detect duplicate ID, missing path/anchor, incorrect target type |
| C. Catalog     | DuckDB derived index                    | Full rebuild and delta updates after edit/delete                |
| D. Search      | SQL, graph traversal, BM25              | JSON results with source locations                              |
| E. Client APIs | CLI and Node.js addon                   | Equivalent typed contracts and results                          |
| F. QA          | Fixtures and golden tests               | Supported Windows/macOS/Linux targets pass CI                   |

## 4.5 Risks and open decisions

- Pin and test DuckDB 2.0, Rust bindings, DuckPGQ and FTS extensions together; do not assume universal compatibility.

- Specify exact section-schema grammar: strict order, optional/repeated sections, nesting and additional-heading policy.

- Define ID/path/URI resolution semantics, collision handling and heading-slug policy.

- Choose supported Markdown dialects and front matter parsing rules.

- Evaluate stemming/tokenization for multilingual prose, code tokens and phrases.

- Use one index writer, with explicit read coordination; DuckDB is not designed as a multi-process transactional service.

- Restrict untrusted SQL access to read-only operations and disallow dangerous external file/network functions.

# 5. Reference documentation and version checks

clap_schema — https://docs.rs/crate/clap_schema/latest

Argx — https://docs.rs/argx

NAPI-RS — https://napi.rs/docs/introduction/getting-started

Comrak — https://docs.rs/comrak/

Rust jsonschema — https://docs.rs/jsonschema/

DuckDB — https://duckdb.org/docs/

DuckDB FTS — https://duckdb.org/docs/stable/core_extensions/full_text_search.html

Tantivy — https://github.com/quickwit-oss/tantivy
