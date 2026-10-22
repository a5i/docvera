export const docs = [
  { slug: "", title: "Introduction", group: "Get started", description: "What docvera validates, and how the pieces fit together." },
  { slug: "quickstart", title: "Quickstart", group: "Get started", description: "Install the CLI and validate your first workspace." },
  { slug: "schemas", title: "Document schemas", group: "Core concepts", description: "Type front matter and give Markdown a predictable structure." },
  { slug: "sql", title: "Query with SQL", group: "Core concepts", description: "Explore documents, sections and diagnostics with read-only SQL." },
  { slug: "wasm", title: "Browser & WASM", group: "Integrations", description: "Use the portable engine without a server or native addon." },
] as const;
export function docHref(slug: string) { return `/docs/${slug ? `${slug}/` : ""}`; }
