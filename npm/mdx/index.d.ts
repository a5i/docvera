export interface Span {
  /** Zero-based UTF-8 byte offsets, end exclusive. */
  start: number;
  end: number;
  /** One-based lines and Unicode scalar columns. */
  line: number;
  column: number;
  endLine: number;
  endColumn: number;
}
export interface Diagnostic {
  path: string;
  span: Span;
  rule: string;
  severity: 'error' | 'warning' | 'info';
  message: string;
  /** Present on evaluated SHACL violations, absent on local/operational diagnostics. */
  shacl?: ShaclResult;
}
export type RdfTerm =
  | { kind: 'iri'; value: string }
  | { kind: 'blankNode' }
  | { kind: 'literal'; value: string; datatype: string; language?: string };
export type RdfPath =
  | { kind: 'predicate'; iri: string }
  | { kind: 'sequence' | 'alternative'; paths: RdfPath[] }
  | { kind: 'inverse' | 'zeroOrMore' | 'oneOrMore' | 'zeroOrOne'; path: RdfPath };
export interface ShaclResult {
  focusNode: RdfTerm;
  constraintComponent: string;
  resultSeverity: string;
  sourceShape?: RdfTerm;
  resultPath?: RdfPath;
  value?: RdfTerm;
  messages: { value: string; language?: string }[];
  /** RFC 6901 JSON pointer into front matter, when an exact location is available. */
  sourceField?: string;
}
export interface CheckResult {
  contractVersion: 1;
  documents: number;
  diagnostics: Diagnostic[];
}
export interface IndexResult {
  contractVersion: 1;
  documents: number;
  updated: number;
  deleted: number;
  diagnostics: number;
}
export interface SearchOptions {
  text: string;
  documentType?: string;
  section?: string;
  limit?: number;
}
export interface SearchHit {
  documentId: string | null;
  path: string;
  sectionId: string;
  section: string;
  location: Span;
  score: number;
  snippet: string;
}
export interface GraphResult {
  contractVersion: 1;
  root: string;
  nodes: string[];
  edges: { source: string; target: string; kind: string }[];
}
export interface ExtractResult {
  contractVersion: 1;
  sectionId: string;
  path: string;
  location: Span;
  markdown: string;
}
export interface Workspace {
  check(): Promise<CheckResult>;
  index(options?: { rebuild?: boolean }): Promise<IndexResult>;
  querySql(sql: string): Promise<{ contractVersion: 1; rows: Record<string, string | number | null>[] }>;
  references(options: { id: string; direction?: 'incoming' | 'outgoing'; depth?: number }): Promise<GraphResult>;
  search(options: SearchOptions): Promise<{ contractVersion: 1; results: SearchHit[] }>;
  extract(sectionId: string): Promise<ExtractResult>;
  close(): Promise<void>;
}
/** Construction is lazy; I/O and validation occur in the individual operations. */
export function openWorkspace(options?: { root?: string }): Promise<Workspace>;

/** A snapshot, not a watcher. Portable workspace-relative paths; UTF-8 source text.
 * Reopen after config/shapes/asset changes; schemas can be updated in memory. Operations are serialized by the native checker. */
export interface Checker {
  check(): Promise<CheckResult>;
  /** Does not change the baseline. Excluded paths return documents=0. */
  checkDocument(path: string, text: string): Promise<CheckResult>;
  /** Overlay check including diagnostics affected in other documents. */
  checkWithText(path: string, text: string): Promise<CheckResult>;
  /** Register/replace schema text without disk I/O; invalid schemas produce document diagnostics. */
  updateSchema(path: string, text: string): Promise<void>;
  updateDocument(path: string, text: string): Promise<void>;
  removeDocument(path: string): Promise<void>;
  close(): Promise<void>;
}
export function openChecker(options?: { root?: string }): Promise<Checker>;
