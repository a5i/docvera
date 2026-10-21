# SHACL workspace validation

Cross-document policies are standard SHACL Turtle, not a custom YAML rule language. Markdown remains authoritative; RDF is an ephemeral, derived graph. `check` and the native Node API evaluate the same current source without SQLite or a network connection.

## Configuration

`docvera.toml` is the single workspace config:

```toml
version = 1
include = ["docs/**/*.md"]

[schemas]
"docs/features/*.md" = "schemas/feature.yaml"
"docs/epics/*.md" = "schemas/epic.yaml"

[validation.shacl]
shapes = ["policies/relations.ttl", "policies/lifecycle.ttl"]
```

- Omit `validation.shacl` to disable this layer. If present, `shapes` must be non-empty.
- Paths are explicit, workspace-relative `.ttl` files, confined to the canonical workspace root, including symlinks. Globs, remote loading and `owl:imports` are unsupported. Duplicate canonical file paths are errors.
- Encoding: files are UTF-8. A single leading BOM is tolerated (Windows editors emit it); any other BOM position is a load error — Oxigraph's lexer can silently absorb a BOM into the following token and corrupt IRIs. CRLF line endings are legal Turtle whitespace and behave identically to LF.
- Files are parsed separately and merged into one shapes graph in sorted path order. Prefix declarations and blank-node labels are file-local. There is **no override/precedence semantics**: named shapes combine their RDF statements.
- Shapes need absolute IRIs or explicit Turtle `@base`; the filesystem location is not an implicit RDF base.
- Invalid Turtle, shape compilation errors, an entirely targetless shapes graph, unsupported SHACL predicates, or validator failures are operation errors. There is no silent fallback to SQL or another evaluator.
- Legacy `docvera.yaml` supports the equivalent nested configuration during migration. Having both configs is an error. Document schemas remain YAML/JSON; front matter supports YAML/TOML.

## Example Core policy

Declare `frontMatter.epic` as a relation in the Feature's YAML schema so the existing resolver supplies document edges:

```yaml
relations:
  - from: frontMatter.epic
    mustExist: true
```

`targetType` (one type) or `targetTypes` (a non-empty list of allowed alternatives) is optional; the two fields are mutually exclusive. If specified, the resolver diagnoses wrong target types before SHACL. If both are omitted, SHACL can diagnose `sh:class` or a union via `sh:or` itself.

`policies/relations.ttl`:

```turtle
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix type: <urn:docvera:v1:type:> .
@prefix f: <urn:docvera:v1:field:> .
@prefix policy: <urn:docvera:policy:> .

policy:Feature a sh:NodeShape ;
    sh:targetClass type:feature ;
    sh:property policy:FeatureEpic .

policy:FeatureEpic a sh:PropertyShape ;
    sh:path f:epic ;
    sh:minCount 1 ;
    sh:maxCount 1 ;
    sh:class type:epic ;
    sh:message "Feature must link to exactly one Epic" .
```

Use the schema's actual `documentType`, exactly and case-sensitively. `type:epic` is not `type:jira-epic` or `type:Epic`.

```sh
docvera --root . check
docvera --root . --format json check
```

## RDF projection v1

Namespaces use URNs so the vocabulary does not imply ownership of a public HTTP domain:

| Concept | IRI / representation |
|---|---|
| Document with ID `FEAT-1` | `urn:docvera:document:id:FEAT-1` |
| Document without ID | `urn:docvera:document:path:` + encoded relative path |
| `documentType: feature` in schema | `rdf:type <urn:docvera:v1:type:feature>` |
| `status: planned` in front matter | `<urn:docvera:v1:field:status> "planned"` |
| Resolved `frontMatter.epic` relation | `<urn:docvera:v1:field:epic> <target-document-IRI>` |
| Nested metadata `review.owner` | `<urn:docvera:v1:field:review.owner>` |
| Relative Markdown path | `<urn:docvera:v1:path>` string |
| Raw Markdown body | `<urn:docvera:v1:body>` string |
| Resolved body link to a document | `<urn:docvera:v1:link> <target-document-IRI>` |

ID/type/property segments use UTF-8 percent encoding, preserving ASCII letters, digits, `-` and `_`. Nested object segments are joined with `.`; a literal dot within a key is encoded as `%2E`, so it cannot collide with nesting. IDs remain stable across file moves; path-based identities do not. Cross-workspace references are unsupported.

- SHACL requires **globally unique declared IDs**, even if local identity rules use a narrower scope. Duplicate IDs produce `shacl.incomplete` errors; SHACL is not evaluated on an ambiguous projection.
- Strings map to `xsd:string`, booleans to `xsd:boolean`, integral JSON numbers to `xsd:integer`, and other numbers to `xsd:double`. No automatic date, enum rank or IRI inference.
- Null and absent values emit no triples. Objects flatten into dotted properties; arrays emit repeated property values, not RDF lists. RDF has set semantics: duplicate values/edges collapse, order and empty arrays/objects are not represented. Use JSON Schema for array length/order requirements.
- `$schema` is excluded. Relation fields are object properties, not their raw ID/path strings. All other scalar metadata, including `id`, is projected.
- Only **successfully resolved document targets** become edges. Unresolved/ambiguous/wrong-type references do not manufacture typed nodes. Existing resolver diagnostics remain authoritative; use `mustExist: true` when absence must fail. Resolved non-document assets are not RDF document nodes. `#fragment` is validated by the resolver; the RDF edge points to the document.
- Declare metadata relations in YAML schemas; SHACL does not guess that arbitrary string fields contain references. Required relations can be expressed in SHACL while the YAML relation declaration handles resolution.

## SHACL-SPARQL capability

Pinned `shacl = 0.3.29`, Native mode, with its `sparql` feature. The dependency gate in `tests/shacl_spike.rs` verifies a conforming graph, a Core violation, a cross-document SPARQL violation, a fixed graph and extraction of report entries. Integration tests additionally exercise Markdown projection and CLI/native parity.

For a draft Epic whose Features must all be draft:

```turtle
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix type: <urn:docvera:v1:type:> .

<urn:docvera:policy:DraftEpic> a sh:NodeShape ;
    sh:targetClass type:epic ;
    sh:sparql [
        sh:message "Draft Epic has a non-draft Feature" ;
        sh:select """
            PREFIX f: <urn:docvera:v1:field:>
            PREFIX type: <urn:docvera:v1:type:>
            SELECT $this WHERE {
                $this f:status "draft" .
                ?feature a type:feature ; f:epic $this .
                FILTER NOT EXISTS { ?feature f:status "draft" }
            }
        """
    ] .
```

Queries use explicit inline `PREFIX` declarations. SHACL-SPARQL prefix declarations/custom components/targets, newer SHACL drafts, rules and SHACL-AF are not advertised as supported. Unknown SHACL predicates are rejected. This is a tested integration subset, **not a complete W3C conformance certification**.

See the executable [design-invariants workspace](../examples/use-cases/design-invariants) for multiple conditional policies. These are snapshot invariants: no status cascading, file rewriting or transition/model checking occurs.

## Diagnostics and CI

- Core/SPARQL results map to `shacl.<ConstraintComponent>`, e.g. `shacl.MinCountConstraintComponent`. These component codes do not depend on generated blank-node labels.
- Every evaluated violation additionally carries a structured `shacl` object on the diagnostic: `focusNode`, `constraintComponent` (full IRI), `resultSeverity`, optional `sourceShape`/`resultPath`/`value`, `messages` (all languages), and `sourceField` — an RFC 6901 JSON pointer into that document's front matter when an exact location is known. RDF terms are adapter-owned `{kind: iri|blankNode|literal, …}` shapes; blank-node labels are never exposed. This is additive: `shacl` is absent on local/operational diagnostics, and older JSON consumers can ignore it.
- Source mapping uses YAML/TOML parser locations, not string search. Direct `urn:docvera:v1:field:…` predicate paths map to the exact UTF-8 byte span of the value (array items map to the specific offending item). Collections highlight the key token rather than claiming a fabricated range; YAML aliases keep their use-site token and never expose child locations of a remote definition. Body paths map to the body range; `urn:docvera:v1:link` maps to the matching Markdown link span when unambiguous. Complex property paths, SPARQL results, missing fields and anything ambiguous fall back to the whole front matter with `sourceField` unset — no invented precision.
- Messages include policy messages, named source shapes, result paths and optional values. Name shapes when stable policy identification matters. Fields are currently carried in the existing diagnostic message, not a separate structured SHACL report contract.
- The focus document maps back to its Markdown path. A focus node that cannot be mapped to a document is an operation error, never a discarded violation.
- `sh:Violation` → `error`, `sh:Warning` → `warning`, `sh:Info` → `info`. Other severities conservatively map to `error`.
- Exit 1: error diagnostics, including incomplete duplicate-ID projections. Warnings fail only with `--warnings-as-errors`; info does not. Policy load/compile/execution failures exit 2 and reject Node operations.
- Diagnostics are deterministically sorted. Structured `shacl` payloads persist to the SQL `diagnostics.shacl` column (JSON, nullable; additive column migration for existing indexes), so reports can be queried with `json_extract`. Shape sources participate in the index fingerprint, so changing a policy invalidates indexed document validation even if Markdown is unchanged. SQL persists derived diagnostics; it does not define normative validation.

## Limits and trust boundary

Shapes are **workspace-trusted code**, not adversarial inputs. The loader rejects `owl:imports`; queries containing `SERVICE` or `FROM` words are conservatively rejected (including inside query comments/strings). This guard is not a full SPARQL sandbox. No SPARQL endpoint is configured. The adapter does not add a persistent RDF database; Rudof uses its compatible in-memory Oxigraph stack internally.

No query deadline, memory quota, process isolation, full conformance corpus, incremental SHACL evaluation or large-workspace performance baseline is provided yet. Projection currently scans relation metadata and rebuilds the whole graph. Stale external snapshots, general completeness/unknown states, versioned policy bundles, suppressions and field-level provenance remain future RFC work. Local schema/reference diagnostics are retained; SHACL does not make an incomplete Jira snapshot authoritative.

Rudof crates are pinned to `0.3.29` (MIT/Apache-2.0); OxRDF terms are pinned to compatible `0.3.5`. Use the repository's current stable Rust toolchain and lockfile; no older MSRV guarantee is established. Cross-platform targets remain subject to the repository CI matrix, not inferred from a Linux build.
