# QA Lead: requirements and test traceability

`REL-001 → REQ-001/REQ-002`; active `TC-001 → REQ-001`; two execution snapshots
point to both `TC-001` and `REL-001`. The older execution failed, the latest passed.
Normal-priority `REQ-002` intentionally has no test.

From the repository root after building `docvera`:

```sh
root=examples/use-cases/qa-traceability
target/debug/docvera --root "$root" check
target/debug/docvera --root "$root" index
target/debug/docvera --root "$root" query sql "$(cat "$root/queries/coverage.sql")" --format json
target/debug/docvera --root "$root" query sql "$(cat "$root/queries/uncovered-critical.sql")" --format json
```

Expected: 6 documents, no diagnostics; uncovered-critical has `rows: []`.
Coverage rows:

```json
[
  {"requirement":"REQ-001","tests":1,"latest_result":"passed"},
  {"requirement":"REQ-002","tests":0,"latest_result":null}
]
```

## Try changes in a copy

- Promote `REQ-002` to `critical`: the critical-coverage report returns
  `[{"requirement":"REQ-002"}]` after indexing; `check` passes.
- Delete `docs/execution.md`: latest result becomes `failed`, not `passed`.
- Retire `TC-001`: active test count becomes zero and latest result becomes null;
  `REQ-001` appears in uncovered-critical.
- Remove Preconditions or point the test at `REL-001` as a requirement: `check` fails.
- Delete the test: execution references become missing-target diagnostics.

Coverage is scoped to REL-001, counts active tests once, and selects the latest
execution across those tests for that release by ISO date, breaking ties by execution
ID descending. This is a compact traceability report, **not** a claim that every test
passed, that evidence is fresh or that the release is ready. Execution files are
hand-authored CI snapshots, not an importer. The critical-coverage report is
workspace-wide. Defect linkage and release-readiness aggregation are follow-ups.
See [shared capability boundaries](../README.md#validation-versus-reporting).
