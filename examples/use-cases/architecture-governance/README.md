# Software Architect: architecture governance

`ARCH-001` defines boundaries. Infrastructure `COMP-002` depends on domain
`COMP-001`; both reference `API-001`. Accepted `ADR-001` references both components.

From the repository root after building `docvera`:

```sh
root=examples/use-cases/architecture-governance
target/debug/docvera --root "$root" check
target/debug/docvera --root "$root" query sql "$(cat "$root/queries/layer-violations.sql")" --format json
target/debug/docvera --root "$root" query sql "$(cat "$root/queries/unlinked-adrs.sql")" --format json
target/debug/docvera --root "$root" graph incoming COMP-001 --depth 2 --format json
target/debug/docvera --root "$root" graph outgoing COMP-002 --depth 2 --format json
```

Expected: 5 documents, no diagnostics; both reports have `rows: []`.
Both graph directions include the COMP-002 → COMP-001 dependency at depth 1.
Graph traversal includes all resolved relation kinds, not only dependencies.

## Try changes in a copy

- Add `dependsOn: [COMP-002]` to COMP-001: layer-violations returns
  `[{"source":"COMP-001","target":"COMP-002"}]` in the SQL report; `check` passes.
  This also creates a cycle: traversal terminates, but there is no acyclicity diagnostic.
- Remove ADR components: unlinked-adrs returns `[{"adr":"ADR-001"}]`;
  `check` still passes because only accepted ADRs need links under this report policy.
- Remove component owner/interfaces, rename ADR Context or point interfaces at
  ARCH-001: `check` fails.
- Delete API-001: both component interface references become missing-target errors.

The layer report demonstrates one explicit forbidden pair (domain → infrastructure),
not a complete architecture policy language. Deprecated-ADR replacement policies
and dedicated cycle/impact reports are follow-ups.
See [shared capability boundaries](../README.md#validation-versus-reporting).
