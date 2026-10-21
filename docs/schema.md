# Schema DSL v1

This is a document-schema envelope, **not** a JSON Schema dialect. `frontMatter` is a normal JSON Schema value; `body`, `identity`, `relations` and `lint` are engine extensions. Unknown envelope/DSL fields are rejected rather than silently ignored.

## Resolution and workspace

`mdx.toml` is the canonical workspace configuration (`mdx init` creates it):

```toml
version = 1
include = ["docs/**/*.md"]
exclude = ["docs/drafts/**"]

[schemas]
"docs/adr-*.md" = "schemas/adr.yaml"

# Optional: policies are standard SHACL in separate Turtle files.
[validation.shacl]
shapes = ["policies/relations.ttl", "policies/lifecycle.ttl"]
```

Omit `[validation.shacl]` to run local/schema/reference checks without SHACL. An enabled section requires a non-empty explicit list of local `.ttl` paths; no globs or remote downloads. See [SHACL policies and RDF vocabulary](shacl.md).

Legacy `mdx.yaml` remains supported with the equivalent `version`, `include`, `exclude`, `schemas`, and `validation.shacl.shapes` structure. Exactly one config may exist: having both `mdx.toml` and `mdx.yaml` is an operation error, with no merging or precedence. To migrate, translate the workspace config into TOML and remove `mdx.yaml`; document schemas and Markdown front matter remain YAML/JSON. Unknown configuration keys and versions are rejected.

Paths are relative to the workspace and normalized to `/` in results. Directory symlinks are not followed, symlink files are not discovered. `.git`, `.mdx`, `target` and `node_modules` are always skipped. Includes/excludes use globset semantics, not `.gitignore` semantics. All included files require a schema. Multiple matching mappings are an error unless a document provides an explicit selector.

A document's front-matter `$schema` takes precedence and resolves relative to that document. Schema files must be inside the canonical workspace root; remote selection is unsupported. The selector is a reserved engine field, removed only from the copy of metadata passed to JSON Schema validation. It remains available in the IR/index.

Front matter must start at the first line with `---` and close with `---`. LF/CRLF are supported; no BOM or alternative delimiter contract. Values must form a mapping; duplicate YAML keys are errors. Metadata diagnostics locate individual YAML tokens where the parser retains locations: front-matter JSON Schema errors and relation errors highlight the whole front matter, while `relation.cardinality` highlights the declared field and `relation.target` highlights the exact reference token (specific array item when unambiguous). YAML aliases keep their use-site token; collections highlight the key rather than a fabricated range; anything unlocatable falls back to the whole front matter. SHACL findings carry the same mapping via `shacl.sourceField`; see [`shacl.md`](shacl.md).

## Envelope

Required: `schemaVersion: 1`, non-empty `documentType`. Optional: `frontMatter`, `identity`, `body`, `relations`, `lint`.

Front-matter JSON Schema supports normal validation/composition, including internal `$ref`/`$defs`. External `$ref` retrieval is disabled (file **and** network). Do not assume support for `$ref` between separate document-schema envelopes or for body-schema composition.

`identity`:

```yaml
identity:
  field: frontMatter.id
  uniqueWithin: documentType  # workspace | documentType | directory
```

The field must resolve to a non-empty string. Collisions are reported on every affected document. Internal database keys are relative paths, never declared IDs, so invalid duplicate documents remain indexable. Without an identity rule, the conventional string `id` is still exposed for link resolution, but uniqueness is not enforced. Directory scope means the same immediate parent directory, not recursive descendants.

## Body and section trees

A section starts at its heading and ends before the next heading of the same or higher level. Parent is the nearest preceding lower-level heading. Code fences are parsed as code, never as headings. ATX and Setext headings are supported; heading matching uses parsed visible inline text, not raw Markdown delimiters.

Every `body`/`children` container has:

| Field | Default | Meaning |
|---|---|---|
| `sections` | `[]` | Rules for immediate child sections |
| `ordered` | `true` | Matched rule indexes must be nondecreasing |
| `additionalSections` | `false` | Allow unmatched sections **and their subtrees** |
| `allowPreamble` | `false` | At the document root only, allow non-whitespace before first heading |

Each section rule requires **exactly one** heading matcher:

- `title`: exact, case-sensitive text.
- `pattern`: Rust regex (use `^…$` for a full match).
- `titleEquals`: string metadata field, e.g. `frontMatter.title`.

Other fields:

| Field | Default | Meaning |
|---|---|---|
| `level` | parent level + 1; root 1 | Required absolute heading level, 1–6 |
| `min` | 1 | Minimum occurrences among immediate siblings |
| `max` | 1 | Maximum occurrences; use an explicit finite number for repetition |
| `nonEmpty` | false | Require non-whitespace direct content before the next heading |
| `children` | closed, empty container | Rules for nested sections |
| `content` | `[]` | Node-count constraints on direct section content |

Optional section: `min: 0`. Repeated section: `min: 0, max: 20`. If an optional section exists, its content/children still must validate. Missing ancestors do not trigger all missing-descendant diagnostics. Missing sections/cardinality failures point to the parent section, or document span at root.

With strict ordering, repetitions of a rule must remain grouped; `A, B, A` is invalid when the rule order is `A, B`. Unknown headings do not advance the rule cursor. If one heading matches multiple rules, the engine emits `body.ambiguous` and uses the first match for further diagnostics. Avoid overlapping regexes.

The default body is closed and empty: omitting it does not implicitly permit arbitrary headings. To allow any document structure, explicitly use `body: { allowPreamble: true, additionalSections: true }`.

### Content

```yaml
content:
  - kind: paragraph
    min: 1
  - kind: code
    language: bash
    min: 1
    max: 3
```

Kinds: `paragraph`, `code`, `list`, `table`, `blockquote`. `language` is only valid for code and matches the first fenced-info token exactly; indented code has no language. `min` defaults to 1, `max` is unbounded when omitted. Counts use node start positions in direct section content; nested AST blocks (e.g. paragraphs inside lists) also count. Child sections do not satisfy a parent's content constraints. Unmentioned node kinds are permitted. This prototype does **not** validate arbitrary block sequences, prohibit node kinds, or inspect table column schemas. `nonEmpty` checks raw non-whitespace content, not semantic prose.

## Relations and links

```yaml
relations:
  - from: frontMatter.related
    targetType: feature
    min: 1
    max: 5
    mustExist: true
```

A source field is a string or array of strings; missing field means zero entries. `targetType` is optional; without it any document type is accepted. `min` defaults to 0, `max` is unbounded, `mustExist` defaults to true. Targets may be IDs (`FEATURE-0042` or `id:FEATURE-0042`), paths relative to the source document, and optional `#anchor` fragments. Typed relations narrow ID candidates by `targetType`; multiple candidates remain ambiguous. `mustExist: false` suppresses only missing-target errors, not wrong types, ambiguous references or invalid anchors. Body Markdown links/images are always checked independently; their edge kind is `link`.

Reference statuses: `resolved`, `missing`, `ambiguous`, `wrong-type`, `missing-anchor`, `outside-workspace`, `invalid-encoding`. Files outside the workspace cannot be link targets. Existing non-Markdown assets without an anchor are accepted but not graph nodes. Markdown targets must be included documents. Links with URI schemes other than `id:` and protocol-relative links are skipped; external availability and URL safety are not validated. HTML `<a>`/custom anchors are not resolved. Logical ID paths are case-sensitive; filesystem lookup follows OS rules.

Anchor policy: lowercase Unicode letters/numbers, preserve `_` and `-`, replace each whitespace with `-`, discard other characters. Resolve collisions by appending `-1`, `-2`, etc. until unused (including collisions with explicitly suffixed headings). This is an explicit PoC policy, not a guarantee of GitHub/GitLab slug parity. URL percent escapes are decoded before lookup.

## Cross-document policies

Cross-document policies use **SHACL**, not a custom YAML assertion DSL. Configure local Turtle shape files in `mdx.toml` under `[validation.shacl]`. The engine projects current documents and resolved references into an in-memory RDF graph, runs pinned Rudof SHACL Native validation, and translates results into Markdown diagnostics. See [SHACL policies](shacl.md) and the executable [design-invariants](../examples/use-cases/design-invariants) example. A `constraints` key in a document schema is not supported.

## Lint and diagnostics

```yaml
lint:
  noSkippedHeadings: true
  noTrailingSpaces: true
```

Both rules default to false; both emit **warnings**, separate from schema errors. Trailing-space lint is literal and can flag Markdown hard breaks and code lines; leave it off when those are intentional. No automatic fixes.

Diagnostic fields: `path`, `span`, `rule`, `severity`, `message`. Rules are namespaced (`yaml.*`, `schema.*`, `body.*`, `identity.*`, `relation.*`, `link.*`, `shacl.*`, `lint.*`). Diagnostics are sorted deterministically. See [`contracts/v1.json`](contracts/v1.json) for offset and exit-code conventions.
