---
name: docvera-policies
description: Author and test docvera document schemas and SHACL cross-document policies. Use for front-matter JSON Schema, section-tree rules, typed relations, lifecycle invariants, generated JSON Schemas, or policy diagnostics; not automatic workflow transitions.
---

# Encode and test validation rules

## Establish the contract and intent

1. Read `docvera guide --format json`, including its embedded schema and SHACL
   references. Inspect existing config, schemas and policies at the agreed root.
2. State the business rule and at least one passing and failing case before
   editing. Obtain approval before weakening a requirement or changing which
   documents are subject to validation.
3. Choose the right layer: front-matter JSON Schema for metadata, the section DSL
   for body structure, declared relations for reference resolution, and SHACL
   for cross-document invariants. SQL reports do not enforce validation policy.

```sh
docvera schema config --format json
docvera schema document --format json
docvera schema check-result --format json
docvera schema diagnostic --format json
```

These commands emit bare generated JSON Schemas. `document` describes the
schema envelope, not a Markdown document's metadata. Structural schema checking
does not replace `check`, which also validates semantic and filesystem rules.

## Write document schemas deliberately

Schemas are local YAML/JSON envelopes: `schemaVersion: 1`, `documentType`, and
optional `frontMatter`, `identity`, `body`, `relations`, `lint`. Unknown DSL fields
are rejected. Use the guide rather than inventing a `constraints` DSL.

For a new `note` type, this minimal `schemas/note.yaml` requires an identity and
non-empty owner while explicitly permitting arbitrary Markdown structure:

```yaml
schemaVersion: 1
documentType: note
frontMatter:
  type: object
  required: [id, owner]
  properties:
    id: { type: string, minLength: 1 }
    owner: { type: string, minLength: 1 }
identity: { field: frontMatter.id, uniqueWithin: workspace }
body: { allowPreamble: true, additionalSections: true }
```

Map document globs in `[schemas]` or use a document-relative front-matter
`$schema` selector. Do not add `$schema` to the envelope to select an editor
meta-schema. External JSON Schema `$ref` retrieval is disabled.

For structured bodies, defaults are strict: omitted body/children containers are
closed, not permissive. Use exactly one heading matcher per section; test order,
optional/repeated sections and non-empty content. Relations must be declared
from `frontMatter.<field>`; ordinary metadata strings do not become graph edges.

## Add cross-document SHACL only when needed

Add explicit workspace-relative Turtle paths under `[validation.shacl]` in the
existing config. Do not replace its schema mappings. Paths must be local and
confined to the workspace; no remote loading, globs or `owl:imports`.

For the example note type, `policies/note.ttl` adds a simple named Core policy:

```turtle
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix type: <urn:docvera:v1:type:> .
@prefix f: <urn:docvera:v1:field:> .

<urn:docvera:policy:NoteOwner> a sh:NodeShape ;
    sh:targetClass type:note ;
    sh:property [
        sh:path f:owner ;
        sh:minCount 1 ;
        sh:message "Note must have an owner"
    ] .
```

The metadata schema intentionally also checks owner; SHACL is useful when the
rule grows into a cross-document condition. Use the actual case-sensitive
`documentType`. Resolved declared relations become RDF object edges; missing,
ambiguous and wrong-type references do not. Use resolver diagnostics alongside
SHACL. Globally duplicate IDs make the SHACL projection incomplete.

Arrays project as repeated RDF values, not ordered lists; duplicate values
collapse. Keep array order/length constraints in JSON Schema. For conditional
rules, use only the inline SPARQL subset documented in the installed guide with
explicit prefixes. This is not full W3C SHACL/SHACL-AF conformance.

## Prove the rule, including failures

In disposable fixtures, test: a conforming case, a targeted violation, a repaired
case, and a malformed/unreadable policy. Check diagnostics and exit codes, not
just that a report exists. Ensure each intended shape actually targets a fixture
node: a conforming empty target set does not prove the rule.

- Exit 1: validation findings; evaluated SHACL findings include structured
  `shacl` data, sometimes an exact `sourceField` JSON pointer.
- Exit 2: policy loading/compilation/execution failed, not conformance.
- Warnings fail only with `--warnings-as-errors`; info does not fail.

Finish with `check --format json` at the agreed root, optionally with the strict
warning gate. Report the rule change and evidence separately from approval.
SHACL is trusted executable input without time/memory isolation. Validation
checks a current snapshot; it does not enforce transitions, cascade statuses,
rewrite files, or establish the completeness of external snapshots.
