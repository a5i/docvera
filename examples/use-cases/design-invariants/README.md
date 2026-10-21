# DS Lead: SHACL design-system invariants

Governance kernel for a small design system: stable principles must govern
rules, stable rules must be governed by active principles, failure-capable
patterns must have failure scenarios, async patterns must declare progress and
success states, deprecated entities must be unreferenced, and stable patterns
must be implemented or tracked by an open finding.

These snapshot invariants are **standard SHACL-SPARQL policies**, not a custom
schema assertion language or hand-written Rust validator. `docvera.toml` selects
`policies/design-invariants.ttl`; local YAML schemas validate document structure
and declare relations for resolution. RDF is derived in memory without an index.
`PTN-1` satisfies "implemented or tracked" through open finding `FIND-1`;
`PTN-2` is draft and not checked; `PRIN-3` is deprecated and unreferenced.

From the repository root after building `docvera`:

```sh
root=examples/use-cases/design-invariants
target/debug/docvera --root "$root" check
target/debug/docvera --root "$root" index
target/debug/docvera --root "$root" query sql "$(cat "$root/queries/untracked-stable-patterns.sql")" --format json
```

Expected: 9 documents, no diagnostics; the SQL report returns `rows: []`.

## Try changes in a copy

Each change produces a SHACL diagnostic (`check` exits 1). The message identifies
the named source shape under `urn:docvera:policy:design:`:

- Remove `loading` from `PTN-1` states:
  `async-pattern-declares-progress-state`.
- Delete the `failureScenarios` line from `PTN-1`:
  `failure-capable-pattern-has-failure-scenario`.
- Set `FIND-1` status to `resolved`:
  `stable-pattern-implemented-or-tracked` on `PTN-1`
  (the SQL report returns `[{"pattern":"PTN-1"}]` after indexing).
- Empty `governs` on `PRIN-2`: `stable-principle-governs-a-rule`
  and `stable-rule-governed-by-active-principle` on `RULE-2`.
- Point `RULE-2` at deprecated `PRIN-3`:
  `deprecated-principle-unreferenced` on `PRIN-3`.
- Delete the failure scenario file: `relation.target` on `PTN-1` plus the
  failure-scenario policy violation. Unresolved targets are not RDF nodes.

SHACL evaluates after reference resolution and uses the actual resolved edges,
including path-based references. Warning-severity results would not fail `check`
unless `--warnings-as-errors` is selected. Malformed Turtle/queries are operation
errors (exit 2), never a passing check.

See [SHACL vocabulary, configuration and limits](../../../crates/docvera-cli/assets/shacl.md) and
[schema DSL](../../../crates/docvera-cli/assets/schema.md).
