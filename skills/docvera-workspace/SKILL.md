---
name: docvera-workspace
description: Initialize or maintain a docvera Markdown workspace, author schema-conforming documents, diagnose check failures, and add a CI validation gate. Use for docvera.toml configuration, missing sections, metadata, identities, relations, and source diagnostics.
---

# Author and validate a workspace

## Inspect before changing

1. Read `docvera guide --format json` from the installed CLI. If unavailable,
   report the capability mismatch and ask before installing/upgrading.
2. Confirm the intended workspace root. Inspect its configuration, schemas,
   included Markdown and configured policies before running them. Policies are
   trusted code; do not evaluate untrusted SHACL as if it were sandboxed.
3. Exactly one of canonical `docvera.toml` and legacy `docvera.yaml` may exist.
   Do not create the second or silently migrate an existing configuration.
4. On a genuinely new workspace, obtain approval to create files, then run
   `init` in a new directory. It creates a runnable ADR/Feature/Test Plan example,
   not an import or migration. Never use it to repair an existing workspace.

The examples use `demo` as a new workspace path; substitute the agreed root.

```sh
docvera --root demo init --format json
docvera --root demo check --format json
```

## Iterate on authoritative files

1. Inspect diagnostics, not just the exit status. Each has a `path`, `rule`,
   `severity`, `span`, and `message`; SHACL findings may carry structured `shacl`.
2. Fix the relevant source or an intentionally incorrect rule. Do not remove a
   required section, disable validation, change `untyped` to `skip`, or loosen
   policy just to silence findings without approval.
3. Keep schema mappings workspace-relative. A document's front-matter `$schema`
   selector instead resolves relative to that document and takes precedence.
   It is reserved and removed before front-matter JSON Schema validation.
4. Re-run check after each meaningful edit. There is no derived database, refresh
   step or automatic fix. CLI commands read current files every time.

Common diagnostic families:

- `yaml.*` / `toml.*`: malformed or unclosed front matter; duplicate keys.
- `schema.*`: schema selection/compilation or metadata validation.
- `body.*`: heading hierarchy, ordering, missing sections or content.
- `identity.*`: missing/duplicate identity fields.
- `relation.*` / `link.*`: missing, ambiguous, wrong-type or invalid-anchor targets.
- `lint.*`: warnings such as trailing spaces or skipped headings.
- `shacl.*`: cross-document policy findings, including incomplete projections.

Offsets are zero-based UTF-8 bytes with exclusive ends; line/column coordinates
are one-based Unicode scalar positions. Do not apply byte offsets directly as
JavaScript string indices or assume all diagnostics pinpoint one field.

## Finish and gate CI

```sh
docvera --root demo check --warnings-as-errors --format json
```

Exit 0 means this gate passed. Exit 1 means validation findings (warnings fail
only with `--warnings-as-errors`; info never fails). Exit 2 means an operation or
usage error, not a clean workspace. Operational JSON failures use
`error.code = "operation_failed"`; clap argument errors can still be plain text.
JSONL is one envelope per invocation, not one diagnostic per line.

For CI, install/pin the agreed CLI version, use an explicit root, preserve the
exit code, and retain diagnostics. Do not add `|| true` or interpret a failed
policy load as conformance. Report commands, remaining findings and any policy
changes separately. A successful check is not approval or external-system sync.
