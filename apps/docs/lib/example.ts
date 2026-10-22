import type { WorkspaceConfig } from "@a5i/docvera/wasm";

export const exampleConfig: WorkspaceConfig = {
  version: 1,
  include: ["docs/**/*.md"],
  schemas: { "docs/**/*.md": "schemas/note.yaml" },
};

export const exampleDocument = `---
id: NOTE-001
title: A better knowledge base
status: draft
---
# A better knowledge base

## Context
Our decisions live in Markdown, next to the code.

## Decision
Give documents a schema. Keep the plain text.
`;

export const exampleSchema = `schemaVersion: 1
documentType: note
frontMatter:
  type: object
  required: [id, title, status]
  properties:
    id: {type: string}
    title: {type: string}
    status: {enum: [draft, accepted]}
  additionalProperties: false
identity:
  field: frontMatter.id
  uniqueWithin: workspace
body:
  sections:
    - titleEquals: frontMatter.title
      children:
        sections:
          - title: Context
            nonEmpty: true
          - title: Decision
            nonEmpty: true
`;

export const exampleSql = "SELECT declared_id, type, path\nFROM documents\nORDER BY path";


export interface PlaygroundExample {
  id: string;
  title: string;
  description: string;
  challenge: string;
  primaryPath: string;
  documents: Record<string, string>;
  schema: string;
  sql: string;
  breakDocument: (text: string) => string;
}

function note(id: string, title: string, status = "draft") {
  return exampleDocument.replaceAll("NOTE-001", id)
    .replaceAll("A better knowledge base", title)
    .replace("status: draft", `status: ${status}`);
}

export const playgroundExamples: PlaygroundExample[] = [
  {
    id: "metadata",
    title: "01 · Typed metadata",
    description: "Require fields and restrict status to draft or accepted with JSON Schema.",
    challenge: "Introduce an error to set status to unknown. Run a check, then restore an allowed value.",
    primaryPath: "docs/note.md",
    documents: { "docs/note.md": exampleDocument },
    schema: exampleSchema,
    sql: exampleSql,
    breakDocument: text => text.replace(/status: .*/, "status: unknown"),
  },
  {
    id: "structure",
    title: "02 · Required sections",
    description: "An architecture decision needs Context, Decision and Consequences, under a matching H1.",
    challenge: "Introduce an error to rename Consequences. The schema reports the missing or unexpected section.",
    primaryPath: "docs/decision.md",
    documents: {
      "docs/decision.md": note("ADR-001", "Keep decisions in Git", "accepted") + "\n## Consequences\nEvery decision can be reviewed alongside the implementation.\n",
    },
    schema: exampleSchema.replace("documentType: note", "documentType: adr") + "          - title: Consequences\n            nonEmpty: true\n",
    sql: "SELECT heading, level\nFROM sections\nORDER BY heading",
    breakDocument: text => text.replace("## Consequences", "## Trade-offs"),
  },
  {
    id: "links",
    title: "03 · Links & anchors",
    description: "Two notes connected with a relative Markdown link to a real heading in the other file.",
    challenge: "Introduce an error to point at a missing file. Or edit the related note's Decision heading to break its anchor.",
    primaryPath: "docs/overview.md",
    documents: {
      "docs/overview.md": note("NOTE-001", "Connected knowledge") + "\nRead the [related decision](related.md#decision).\n",
      "docs/related.md": note("NOTE-002", "A shared decision", "accepted"),
    },
    schema: exampleSchema,
    sql: exampleSql,
    breakDocument: text => text.replace("related.md#decision", "missing.md#decision"),
  },
  {
    id: "identity",
    title: "04 · Unique identities",
    description: "Document identities are checked across the whole workspace, not just one file.",
    challenge: "Introduce an error to give both notes the same ID. Open either document to inspect the collision.",
    primaryPath: "docs/first.md",
    documents: {
      "docs/first.md": note("NOTE-001", "First decision"),
      "docs/second.md": note("NOTE-002", "Second decision", "accepted"),
    },
    schema: exampleSchema,
    sql: exampleSql,
    breakDocument: text => text.replace("id: NOTE-001", "id: NOTE-002"),
  },
  {
    id: "sections-sql",
    title: "05 · Query the section tree",
    description: "Join documents and sections to list the H2 headings of every note in the workspace.",
    challenge: "Open query.sql and run the join. Introduce an error to remove a required Decision section, then inspect Diagnostics.",
    primaryPath: "docs/architecture.md",
    documents: {
      "docs/architecture.md": note("ARCH-001", "Application architecture", "accepted"),
      "docs/delivery.md": note("PLAN-001", "Delivery workflow"),
    },
    schema: exampleSchema,
    sql: "SELECT d.declared_id, d.path, s.heading\nFROM documents d\nJOIN sections s ON d.path = s.document_id\nWHERE s.level = 2\nORDER BY d.path, s.heading",
    breakDocument: text => text.replace("## Decision", "## Implementation"),
  },
  {
    id: "workspace-report",
    title: "06 · Workspace reports",
    description: "Aggregate three documents with read-only SQL. Validation diagnostics are queryable too.",
    challenge: "Run the prepared count query. Introduce an invalid status, then try SELECT * FROM diagnostics in query.sql.",
    primaryPath: "docs/product.md",
    documents: {
      "docs/product.md": note("NOTE-001", "Product scope", "accepted"),
      "docs/architecture.md": note("NOTE-002", "System boundaries", "accepted"),
      "docs/testing.md": note("NOTE-003", "Testing strategy"),
    },
    schema: exampleSchema,
    sql: "SELECT type, COUNT(*) AS documents\nFROM documents\nGROUP BY type\nORDER BY type",
    breakDocument: text => text.replace(/status: .*/, "status: unknown"),
  },
];
