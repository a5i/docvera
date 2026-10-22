import type { Metadata } from "next";
import { notFound } from "next/navigation";
import Link from "next/link";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { ArrowLeft, ArrowRight, ArrowUpRight, BookOpen } from "lucide-react";
import Introduction from "@/content/introduction.mdx";
import Quickstart from "@/content/quickstart.mdx";
import Schemas from "@/content/schemas.mdx";
import Sql from "@/content/sql.mdx";
import Wasm from "@/content/wasm.mdx";
import { DocsSidebar } from "@/components/docs-sidebar";
import { docs, docHref } from "@/lib/navigation";
import { headingId } from "@/lib/headings";

const pages = { "": Introduction, quickstart: Quickstart, schemas: Schemas, sql: Sql, wasm: Wasm };
export const dynamicParams = false;
export function generateStaticParams() { return docs.map(item => ({ slug: item.slug ? [item.slug] : [] })); }
type Props = { params: Promise<{ slug?: string[] }> };
export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const slug = (await params).slug?.join("/") ?? "";
  const item = docs.find(item => item.slug === slug);
  return { title: item?.title ?? "Not found", description: item?.description };
}

export default async function DocPage({ params }: Props) {
  const slug = (await params).slug?.join("/") ?? "";
  const index = docs.findIndex(item => item.slug === slug);
  if (index === -1) notFound();
  const item = docs[index];
  const Content = pages[item.slug];
  const source = await readFile(path.join(process.cwd(), "content", `${item.slug || "introduction"}.mdx`), "utf8");
  const headings = [...source.matchAll(/^## (.+)$/gm)].map(match => match[1]);
  const previous = docs[index - 1];
  const next = docs[index + 1];
  return <div className="docs-layout page-container"><DocsSidebar /><main id="main" className="docs-main"><div className="docs-breadcrumb"><BookOpen size={13} /> Docs <span>/</span> {item.group}</div><div className="docs-title"><div className="eyebrow">{item.group.toUpperCase()}</div><h1>{item.title}</h1><p>{item.description}</p></div><article className="docs-prose"><Content /></article><div className="docs-edit"><a href={`https://github.com/a5i/docvera/blob/main/apps/docs/content/${item.slug || "introduction"}.mdx`}>Edit this page on GitHub <ArrowUpRight size={13} /></a></div><nav className="docs-pagination" aria-label="Adjacent documentation pages">{previous ? <Link href={docHref(previous.slug)}><small><ArrowLeft size={12} /> Previous</small><strong>{previous.title}</strong></Link> : <div />}{next && <Link href={docHref(next.slug)}><small>Next <ArrowRight size={12} /></small><strong>{next.title}</strong></Link>}</nav></main><aside className="docs-toc"><span>ON THIS PAGE</span><nav aria-label="On this page">{headings.map(heading => <a key={heading} href={`#${headingId(heading)}`}>{heading}</a>)}</nav><div className="toc-note">The same engine.<br />CLI · Node.js · WASM</div></aside></div>;
}
