# Project Manager: offline Jira/Confluence governance

`PROJECT-001 ← MS-001 → PROJ-101 → SPEC-001` links a charter, milestone,
a completed Jira epic snapshot and an approved Confluence specification snapshot.
All files are local fixtures. No credentials, network, adapters or synchronization are used.

From the repository root after building `mdx`:

```sh
root=examples/use-cases/project-governance
target/debug/mdx --root "$root" check
target/debug/mdx --root "$root" index
target/debug/mdx --root "$root" query sql "$(cat "$root/queries/documentation-drift.sql")" --format json
target/debug/mdx --root "$root" query sql "$(cat "$root/queries/incomplete-milestones.sql")" --format json
```

Expected: 4 documents, no diagnostics; both reports have `rows: []`.

## Try changes in a copy

- Change specification status to draft: documentation-drift returns
  `[{"epic":"PROJ-101","specification":"SPEC-001","specification_status":"draft"}]`
  after indexing; `check` passes.
- Change epic status to in-progress: incomplete-milestones returns
  `[{"milestone":"MS-001","deliverable":"PROJ-101"}]`.
- Remove an epic owner, rename charter Scope or point the epic specification at
  MS-001: `check` fails.
- Delete the charter/specification: required typed references become missing-target errors.

The milestone report checks deliverable completion only; approved documentation is
reported separately. Every deliverable has a schema-required owner. Snapshot source
and capturedAt fields describe the fixture, not verified external freshness or existence.
External availability cannot be inferred from local validation: a future adapter needs
`pass`/`fail`/`unknown` semantics, especially when the upstream API is unavailable.
Decisions, snapshot TTL, synchronization conflicts and access checks are follow-ups.
See [shared capability boundaries](../README.md#validation-versus-reporting).
