# Executable document-governance use cases

These six offline workspaces translate representative rules from
[`demo/cases.md`](../../demo/cases.md) into today's schema validation, SHACL policies and SQLite reports.
They are examples **and** shared integration-test fixtures, not six new engine features.
The small [`docvera init` starter](../workspace) uses `docvera.toml`; document schemas remain YAML.

| Role | Workspace | Documents | Reports |
|---|---|---:|---|
| Product Owner | [Product traceability](product-traceability) | 6 | Missing epics; orphan requirements |
| QA Lead | [QA traceability](qa-traceability) | 6 | Release coverage/latest result; uncovered critical requirements |
| Platform Engineer | [Operational readiness](operational-readiness) | 4 | Missing operational docs/SLOs; overdue reviews |
| Software Architect | [Architecture governance](architecture-governance) | 5 | Layer violations; accepted ADRs without components |
| Project Manager | [Project governance](project-governance) | 4 | Documentation drift; incomplete milestone deliverables |
| DS Lead | [Design invariants](design-invariants) | 9 | Untracked stable patterns; SHACL cross-document diagnostics |

## Run an example

Build once from the repository root:

```sh
cargo build --locked -p docvera-cli
```

Then, for example (POSIX shell; Windows users can pass the SQL string directly):

```sh
root=examples/use-cases/qa-traceability
target/debug/docvera --root "$root" check
target/debug/docvera --root "$root" index
target/debug/docvera --root "$root" query sql "$(cat "$root/queries/coverage.sql")" --format json
```

All workspaces pass `check` with zero diagnostics. All baseline policy reports return
empty `rows` except QA coverage, which intentionally includes a normal-priority
uncovered requirement. Each README documents expected results and mutations.
Copy the workspace before trying edits if you want to keep the tracked examples pristine.

## Validation versus reporting

- **Validation today:** required metadata, conditional front-matter JSON Schema,
  section/content rules, unique IDs, typed outgoing references and cardinality; SHACL Core/SPARQL policies in the design-invariants workspace.
  `check` reads source without an index; errors exit 1.
- **Reporting today:** SQL over the last explicit indexed snapshot. Business-policy
  violations in the first five workspaces are rows, not `check` diagnostics; the design-invariants workspace explicitly enables SHACL validation. A successful SQL report exits 0 even
  when it finds violations. Re-run `index` after edits.
- **Deferred:** transition validation, full SHACL conformance/resource isolation,
  dedicated acyclic-relation APIs, named-query execution, live Jira/Confluence/CI import and external availability
  (`pass`/`fail`/`unknown`). None is simulated as a working engine feature.

The examples use standalone documents for entities needing IDs and relations.
Relation reports restrict edge kind and resolution status to avoid counting body
links or unrelated references. Date-based examples use a fixed evaluation date,
not the wall clock. Snapshot files do not establish live external existence,
accessibility, synchronization or release readiness.

These are deliberately small vertical slices, not exhaustive implementations of
every proposed rule: API/incident catalogs, defect policies, full release-readiness
aggregation and external snapshot TTL checks remain follow-up examples.

## Verification

Rust engine and CLI tests copy these directories into temporary workspaces using
`tests/support/mod.rs`; native Node tests copy the same tracked files. Developer
`.docvera` indexes are excluded. Tests cover all SQL files, exact baseline rows,
positive violation cases, schema errors, edits/deletions, indexed-snapshot semantics,
search/extraction, graph traversal and CLI/native parity.

```sh
cargo test --locked --workspace
cargo build --locked -p docvera-cli
npm --prefix npm/docvera run build
npm --prefix npm/docvera test
```
