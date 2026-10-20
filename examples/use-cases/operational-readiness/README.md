# Platform Engineer: operational readiness

Critical production service `SVC-001` references `RUN-001`, `DEP-001` and `SLO-001`.
Its production status conditionally requires an `oncall` metadata field through
standard JSON Schema. Required operational documents are checked by SQL reports,
not conditional relation validation.

From the repository root after building `mdx`:

```sh
root=examples/use-cases/operational-readiness
target/debug/mdx --root "$root" check
target/debug/mdx --root "$root" index
for report in missing-operational-docs missing-slos overdue-reviews; do
  target/debug/mdx --root "$root" query sql "$(cat "$root/queries/$report.sql")" --format json
done
target/debug/mdx --root "$root" search "database connection timeout" --type runbook --section Diagnosis --format json
target/debug/mdx --root "$root" extract 'docs/runbook.md#diagnosis' --format json
```

Expected: 4 documents, no diagnostics; all reports have `rows: []`.
Search finds one Diagnosis section belonging to RUN-001; extract returns its original Markdown.

## Try changes in a copy

- Remove `runbook: RUN-001`: `check` passes but missing-operational-docs returns
  `[{"service":"SVC-001"}]` after indexing. Removing deployment has the same effect.
- Remove `slo: SLO-001`: missing-slos returns `[{"service":"SVC-001"}]`.
- Set reviewedAt to `'2026-09-19'`: overdue-reviews returns
  `[{"runbook":"RUN-001","reviewed_at":"2026-09-19"}]`.
- Remove oncall: production service validation fails; changing lifecycle to
  development makes oncall optional.
- Rename Diagnosis, point runbook at SLO-001 or delete the runbook: `check` fails.
- Change the timeout phrase and reindex: the original phrase no longer matches.

Review evaluation is fixed at 2026-10-20, with a 30-day window and strict cutoff
`reviewedAt < 2026-09-20`. Dates use quoted ISO strings; the schema checks their
shape, not full calendar validity. The review report covers all runbooks.
API contracts, incidents and combined search/graph filtering are follow-ups.
See [shared capability boundaries](../README.md#validation-versus-reporting).
