import type { Metadata } from "next";
import { FlaskConical, ArrowUpRight } from "lucide-react";
import Link from "next/link";
import { Playground } from "@/components/playground";

export const metadata: Metadata = { title: "Playground" };
export default function PlaygroundPage() {
  return <main id="main" className="playground-page page-container"><div className="eyebrow"><FlaskConical size={14} /> YOUR BROWSER IS THE LAB</div><div className="playground-page-heading"><div><h1>Less guessing. More checking.</h1><p>Six hands-on examples. Real documents, editable schemas, and the docvera engine.</p></div><Link className="text-link" href="/docs/schemas/">Schema guide <ArrowUpRight size={15} /></Link></div><Playground /><div className="playground-help"><div><span>01</span><strong>Change the document</strong><p>Try a different status, or remove a required section.</p></div><div><span>02</span><strong>Adjust the contract</strong><p>Open note.yaml to edit metadata and section rules.</p></div><div><span>03</span><strong>Ask a question</strong><p>Open query.sql to explore the workspace with SQL.</p></div></div><p className="playground-limits">Examples use small in-memory workspaces of one to three documents. Switch documents with the file selector; Reset restores the current example. CLI and native Node.js support additional capabilities, including SHACL policies. Edits are temporary and reset when you leave this page.</p></main>;
}
