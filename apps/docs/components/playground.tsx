"use client";

import dynamic from "next/dynamic";
import { useEffect, useRef, useState } from "react";
import { useTheme } from "next-themes";
import { markdown } from "@codemirror/lang-markdown";
import { yaml } from "@codemirror/lang-yaml";
import { sql as sqlLanguage } from "@codemirror/lang-sql";
import type { CheckResult, QueryResult } from "@a5i/docvera/wasm";
import { AlertCircle, ArrowUpRight, CheckCircle2, FileCode2, FileText, LoaderCircle, Play, RotateCcw, ShieldCheck, Terminal } from "lucide-react";
import Link from "next/link";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { exampleConfig, playgroundExamples, type PlaygroundExample } from "@/lib/example";

const CodeMirror = dynamic(() => import("@uiw/react-codemirror"), { ssr: false, loading: () => <div className="editor-loading">Loading editor…</div> });
type File = "document" | "schema" | "sql";
type Result = { check: CheckResult; query: QueryResult | null; duration: number };

export function Playground({ compact = false }: { compact?: boolean }) {
  const { resolvedTheme } = useTheme();
  const [file, setFile] = useState<File>("document");
  const [exampleId, setExampleId] = useState(playgroundExamples[0].id);
  const example = playgroundExamples.find(item => item.id === exampleId)!;
  const [documents, setDocuments] = useState<Record<string, string>>({ ...playgroundExamples[0].documents });
  const [documentPath, setDocumentPath] = useState(playgroundExamples[0].primaryPath);
  const document = documents[documentPath] ?? "";
  const [schema, setSchema] = useState(playgroundExamples[0].schema);
  const [sql, setSql] = useState(playgroundExamples[0].sql);
  const [busy, setBusy] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [outputTab, setOutputTab] = useState("diagnostics");
  const workerRef = useRef<Worker | null>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => { workerRef.current?.terminate(); if (timeoutRef.current) clearTimeout(timeoutRef.current); }, []);
  const errors = result?.check.diagnostics.filter(item => item.severity === "error").length ?? 0;
  const warnings = result?.check.diagnostics.filter(item => item.severity === "warning").length ?? 0;

  function edit(value: string) {
    setDirty(true);
    if (file === "document") setDocuments(current => ({ ...current, [documentPath]: value }));
    else if (file === "schema") setSchema(value);
    else setSql(value);
  }
  function loadExample(next: PlaygroundExample) {
    setExampleId(next.id);
    setDocuments({ ...next.documents });
    setDocumentPath(next.primaryPath);
    setSchema(next.schema);
    setSql(next.sql);
    setResult(null); setError(null); setDirty(false);
    setOutputTab("diagnostics"); setFile("document");
  }
  function reset() { loadExample(example); }
  function chooseExample(id: string) {
    const next = playgroundExamples.find(item => item.id === id);
    if (!next || next.id === exampleId) return;
    if ((schema !== example.schema || sql !== example.sql || Object.entries(example.documents).some(([path, text]) => documents[path] !== text)) && !window.confirm("Switch examples and discard your edits?")) return;
    loadExample(next);
  }
  function breakExample() {
    setDocuments(current => ({ ...current, [example.primaryPath]: example.breakDocument(current[example.primaryPath]) }));
    setDocumentPath(example.primaryPath);
    setFile("document"); setDirty(true);
  }
  function run() {
    if (busy) return;
    setBusy(true); setError(null);
    const started = performance.now();
    const finish = () => { workerRef.current?.terminate(); workerRef.current = null; if (timeoutRef.current) clearTimeout(timeoutRef.current); setBusy(false); };
    try {
      const worker = new Worker("/checker.worker.js", { type: "module" });
      workerRef.current = worker;
      timeoutRef.current = setTimeout(() => { setError("Execution stopped after 20 seconds. Simplify the input and try again."); finish(); }, 20_000);
      worker.onmessage = ({ data }) => {
        if (data.ok) { setResult({ check: data.check, query: data.query, duration: performance.now() - started }); setDirty(false); setOutputTab(file === "sql" ? "results" : "diagnostics"); }
        else { setError(data.error); setResult(null); }
        finish();
      };
      worker.onerror = event => { setError(event.message || "Could not start the WASM engine. Check your connection and retry."); setResult(null); finish(); };
      worker.postMessage({ config: exampleConfig, sources: { documents, schemas: { "schemas/note.yaml": schema } }, sql: file === "sql" ? sql : null });
    } catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); finish(); }
  }

  return <div className={`playground ${compact ? "playground-compact" : ""}`}>
    <div className="workspace-bar"><span><span className="status-dot" /> {compact ? "LIVE EXAMPLE" : "LOCAL WORKSPACE"}</span><span className="workspace-runtime">Rust → WASM <span>·</span> In your browser</span>{compact && <Link href="/playground/">Open playground <ArrowUpRight size={13} /></Link>}</div>
    {!compact && <div className="example-picker">
      <div className="example-picker-heading">
        <label>EXAMPLE<select aria-label="Choose example" value={exampleId} disabled={busy} onChange={event => chooseExample(event.target.value)}>{playgroundExamples.map(item => <option key={item.id} value={item.id}>{item.title}</option>)}</select></label>
        <span className="example-file-count">{Object.keys(documents).length} {Object.keys(documents).length === 1 ? "document" : "documents"} · 1 schema</span>
      </div>
      <p>{example.description}</p>
      <p className="example-challenge"><strong>Try this:</strong> {example.challenge}</p>
      {Object.keys(documents).length > 1 && <label className="document-picker">EDIT DOCUMENT<select aria-label="Workspace document" value={documentPath} disabled={busy} onChange={event => { setDocumentPath(event.target.value); setFile("document"); }}>{Object.keys(documents).map(path => <option key={path} value={path}>{path}</option>)}</select></label>}
    </div>}
    <Tabs value={file} onValueChange={value => setFile(value as File)} className="flex-col gap-0">
      <div className="editor-toolbar">
        <TabsList variant="line" aria-label="Workspace files">
          <TabsTrigger value="document"><FileText />{documentPath.split("/").pop()}</TabsTrigger>
          <TabsTrigger value="schema"><FileCode2 />note.yaml</TabsTrigger>
          <TabsTrigger value="sql"><Terminal />query.sql</TabsTrigger>
        </TabsList>
        <div className="editor-actions">
          <Button variant="ghost" size="icon-sm" aria-label="Reset example" disabled={busy} onClick={reset}><RotateCcw /></Button>
          <Button onClick={run} disabled={busy} size="sm">{busy ? <LoaderCircle className="animate-spin" /> : <Play size={12} />} {busy ? "Running…" : file === "sql" ? "Run query" : "Run check"}</Button>
        </div>
      </div>
      {(["document", "schema", "sql"] as const).map(name => <TabsContent key={name} value={name} className="editor-area">
        {file === name && <CodeMirror value={name === "document" ? document : name === "schema" ? schema : sql} height={compact ? "300px" : "390px"} theme={resolvedTheme === "dark" ? "dark" : "light"} extensions={[name === "document" ? markdown() : name === "schema" ? yaml() : sqlLanguage()]} onChange={edit} editable={!busy} basicSetup={{ foldGutter: false, highlightActiveLine: true, autocompletion: false }} aria-label={`Edit ${name}`} />}
      </TabsContent>)}
    </Tabs>
    <Tabs value={outputTab} onValueChange={value => setOutputTab(String(value))} className="flex-col gap-0">
      <div className="output-bar">
        <TabsList variant="line" aria-label="Output panels">
          <TabsTrigger value="diagnostics"><ShieldCheck />Diagnostics {result && <span className="count">{result.check.diagnostics.length}</span>}</TabsTrigger>
          <TabsTrigger value="results"><Terminal />SQL results</TabsTrigger>
        </TabsList>
        <span className="output-meta" data-dirty={dirty && !!result}>{dirty && result ? "Changes not checked" : result ? `${result.check.documents} ${result.check.documents === 1 ? "document" : "documents"} · ${Math.round(result.duration)} ms incl. startup` : "Ready when you are"}</span>
      </div>
      {(["diagnostics", "results"] as const).map(panel => <TabsContent value={panel} key={panel}>
      {outputTab === panel && (
    <div className="output-body" role="status" aria-live="polite" aria-busy={busy}>
      {error ? <div className="execution-error"><AlertCircle size={18} /><div><strong>Could not run this workspace</strong><pre>{error}</pre></div></div> : !result ? <div className="output-empty"><Terminal size={20} /><div><strong>Your Markdown. Real guarantees.</strong><p>Run a check, then change a field or a heading and try again.</p></div></div> : outputTab === "results" ? result.query ? <div className="query-table-wrap"><table className="query-table"><thead><tr>{Object.keys(result.query.rows[0] ?? {}).map(key => <th key={key}>{key}</th>)}</tr></thead><tbody>{result.query.rows.map((row, index) => <tr key={index}>{Object.values(row).map((value, column) => <td key={column}>{typeof value === "object" ? JSON.stringify(value) : String(value)}</td>)}</tr>)}</tbody></table>{!result.query.rows.length && <p>Query returned 0 rows.</p>}</div> : <p className="muted">Open query.sql and run a query to explore this workspace.</p> : <>
        {!result.check.diagnostics.length ? <div className="success-result"><CheckCircle2 size={21} /><div><strong>All checks passed</strong><p>{result.check.documents} {result.check.documents === 1 ? "document" : "documents"} validated. Metadata and structure match the schema.</p></div><span className="result-badge">VALID</span></div> : <><div className="diagnostic-summary">{errors} errors · {warnings} warnings</div>{result.check.diagnostics.map((diagnostic, index) => <div className="diagnostic" key={index}><AlertCircle size={16} /><div><code>{diagnostic.path}:{diagnostic.span.line}:{diagnostic.span.column}</code><p>{diagnostic.message}</p><small>{diagnostic.rule} · {diagnostic.severity}</small></div></div>)}</>}
      </>}
    </div>
      )}
      </TabsContent>)}
    </Tabs>
    <div className="workspace-bottom"><span><ShieldCheck size={13} /> Files stay in this browser. No server execution.</span><Button variant="ghost" size="xs" onClick={breakExample} disabled={busy}>Introduce an error ↗</Button></div>
  </div>;
}
