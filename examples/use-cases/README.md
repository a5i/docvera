# Executable document-governance use cases

These five offline workspaces translate representative rules from
[`demo/cases.md`](../../demo/cases.md) into today's schema DSL and SQLite reports.
They are examples **and** shared integration-test fixtures, not five new engine features.
The small [`mdx init` starter](../workspace) remains unchanged.

| Role | Workspace | Documents | Reports |
|---|---|---:|---|
| Product Owner | [Product traceability](product-traceability) | 6 | Missing epics; orphan requirements |
| QA Lead | [QA traceability](qa-traceability) | 6 | Release coverage/latest result; uncovered critical requirements |
| Platform Engineer | [Operational readiness](operational-readiness) | 4 | Missing operational docs/SLOs; overdue reviews |
| Software Architect | [Architecture governance](architecture-governance) | 5 | Layer violations; accepted ADRs without components |
| Project Manager | [Project governance](project-governance) | 4 | Documentation drift; incomplete milestone deliverables |

## Run an example

Build once from the repository root:

```sh
cargo build --locked -p md-cli
```

Then, for example (POSIX shell; Windows users can pass the SQL string directly):

```sh
root=examples/use-cases/qa-traceability
target/debug/mdx --root "$root" check
target/debug/mdx --root "$root" index
target/debug/mdx --root "$root" query sql "$(cat "$root/queries/coverage.sql")" --format json
```

All workspaces pass `check` with zero diagnostics. All baseline policy reports return
empty `rows` except QA coverage, which intentionally includes a normal-priority
uncovered requirement. Each README documents expected results and mutations.
Copy the workspace before trying edits if you want to keep the tracked examples pristine.

## Validation versus reporting

- **Validation today:** required metadata, conditional front-matter JSON Schema,
  section/content rules, unique IDs, typed outgoing references and cardinality.
  `check` reads source without an index; errors exit 1.
- **Reporting today:** SQL over the last explicit indexed snapshot. Business-policy
  violations are rows, not `check` diagnostics. A successful SQL report exits 0 even
  when it finds violations. Re-run `index` after edits.
- **Deferred:** lifecycle-dependent body/relation rules, transition validation,
  configurable cross-document policy severities, acyclic-relation validation,
  named-query execution, live Jira/Confluence/CI import and external availability
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
`.mdx` indexes are excluded. Tests cover all SQL files, exact baseline rows,
positive violation cases, schema errors, edits/deletions, indexed-snapshot semantics,
search/extraction, graph traversal and CLI/native parity.

```sh
cargo test --locked --workspace
cargo build --locked -p md-cli
npm --prefix npm/mdx run build
npm --prefix npm/mdx test
```
