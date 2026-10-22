# Product Owner: product traceability

`VISION-001 ← INIT-001 ← FEAT-001/FEAT-002 → REQ-001`; planned `FEAT-001`
also points to the local Jira snapshot `PROD-101`. Requirements are independently
identified documents rather than headings. The draft feature intentionally has no epic.

From the repository root after building `docvera`:

```sh
root=examples/use-cases/product-traceability
target/debug/docvera --root "$root" check
target/debug/docvera --root "$root" query sql "$(cat "$root/queries/missing-epics.sql")" --format json
target/debug/docvera --root "$root" query sql "$(cat "$root/queries/orphan-requirements.sql")" --format json
```

Expected: 6 documents, no diagnostics, both reports have `rows: []`.

## Try changes in a copy

- Change `docs/feature-draft.md` from `draft` to `planned` and re-run the query:
  `missing-epics` returns `[{"feature":"FEAT-002"}]`. `check` still passes.
- Delete both feature documents and re-run the query: `orphan-requirements` returns
  `[{"requirement":"REQ-001"}]`; the remaining documents still validate.
- Remove the feature owner, rename `Problem`, remove its requirement references,
  or point its initiative at `REQ-001`: `check` fails.
- Delete the requirement: inbound feature relations become missing-target diagnostics.

Required Problem/Value/Success Metrics sections apply at every status in this example.
Only the missing-epic report is status-dependent; lifecycle-dependent body validation
is not implemented. `PROD-101` is an offline local record, not a live Jira check.
See [shared capability boundaries](../README.md#validation-versus-reporting).
