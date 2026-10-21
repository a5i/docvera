use crate::{model::*, workspace::Workspace};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params, types::ValueRef};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::Path,
    time::{Duration, Instant},
};

const DDL: &str = "
CREATE TABLE IF NOT EXISTS engine_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS documents(internal_id TEXT PRIMARY KEY, declared_id TEXT, type TEXT, path TEXT UNIQUE NOT NULL, metadata TEXT NOT NULL, hash TEXT NOT NULL, source TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sections(section_id TEXT PRIMARY KEY, document_id TEXT NOT NULL, heading TEXT, level INTEGER, parent_id TEXT, ordinal INTEGER, text TEXT, span TEXT);
CREATE TABLE IF NOT EXISTS nodes(node_id TEXT PRIMARY KEY, document_id TEXT, section_id TEXT, kind TEXT, language TEXT, span TEXT);
CREATE TABLE IF NOT EXISTS relations(source_document_id TEXT, target_document_id TEXT, kind TEXT, reference TEXT, resolution_status TEXT, source_span TEXT);
CREATE TABLE IF NOT EXISTS diagnostics(document_id TEXT, rule TEXT, severity TEXT, span TEXT, message TEXT, shacl TEXT);
CREATE VIRTUAL TABLE IF NOT EXISTS section_fts USING fts5(section_id UNINDEXED, document_id UNINDEXED, title, heading, body, tokenize='unicode61');
CREATE INDEX IF NOT EXISTS relation_source ON relations(source_document_id);
CREATE INDEX IF NOT EXISTS relation_target ON relations(target_document_id);
";

fn db_path(root: &Path) -> std::path::PathBuf {
    root.join(".mdx/index.sqlite3")
}
fn reader(root: &Path) -> Result<Connection> {
    let root = root.canonicalize()?;
    crate::workspace::within(&root, &db_path(&root))
        .context("Index unavailable or outside workspace; run mdx index")?;
    let conn = Connection::open_with_flags(db_path(&root), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .context("Index unavailable; run mdx index")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    let version: String = conn.query_row(
        "SELECT value FROM engine_meta WHERE key='format'",
        [],
        |r| r.get(0),
    )?;
    if version != "1" {
        bail!("Index format changed; remove .mdx and rebuild");
    }
    Ok(conn)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexResult {
    pub contract_version: u32,
    pub documents: usize,
    pub updated: usize,
    pub deleted: usize,
    pub diagnostics: usize,
}

pub fn index(workspace: &Workspace, rebuild: bool) -> Result<IndexResult> {
    let dir = workspace.root.join(".mdx");
    fs::create_dir_all(&dir)?;
    crate::workspace::within(&workspace.root, &dir)?;
    let database = db_path(&workspace.root);
    if database.symlink_metadata().is_ok() {
        crate::workspace::within(&workspace.root, &database)?;
    }
    let mut conn = Connection::open(db_path(&workspace.root))?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch(DDL)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    // Additive format-v1 migration: old snapshots retain their content/FTS rows.
    let has_shacl: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('diagnostics') WHERE name='shacl')",
        [],
        |r| r.get(0),
    )?;
    if !has_shacl {
        tx.execute("ALTER TABLE diagnostics ADD COLUMN shacl TEXT", [])?;
    }
    let old_fingerprint: Option<String> = tx
        .query_row(
            "SELECT value FROM engine_meta WHERE key='fingerprint'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let old: BTreeMap<String, String> = tx
        .prepare("SELECT path, hash FROM documents")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let current: BTreeSet<_> = workspace.documents.iter().map(|d| d.path.clone()).collect();
    let deleted: Vec<_> = old
        .keys()
        .filter(|p| !current.contains(*p))
        .cloned()
        .collect();
    let force = rebuild || old_fingerprint.as_deref() != Some(&workspace.fingerprint);
    let changed: Vec<_> = workspace
        .documents
        .iter()
        .filter(|d| force || old.get(&d.path) != Some(&d.hash))
        .collect();
    for path in deleted
        .iter()
        .map(String::as_str)
        .chain(changed.iter().map(|d| d.path.as_str()))
    {
        tx.execute("DELETE FROM section_fts WHERE document_id=?1", [path])?;
        tx.execute("DELETE FROM sections WHERE document_id=?1", [path])?;
        tx.execute("DELETE FROM nodes WHERE document_id=?1", [path])?;
        tx.execute("DELETE FROM documents WHERE path=?1", [path])?;
    }
    for doc in &changed {
        tx.execute(
            "INSERT INTO documents VALUES (?1,?2,?3,?1,?4,?5,?6)",
            params![
                doc.path,
                doc.declared_id,
                doc.document_type,
                doc.metadata.to_string(),
                doc.hash,
                doc.source
            ],
        )?;
        let title = doc
            .metadata
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("");
        for (ordinal, section) in doc.sections.iter().enumerate() {
            let parent = section.parent.map(|i| &doc.sections[i].id);
            let text = &doc.source[section.content_start..section.direct_end];
            tx.execute(
                "INSERT INTO sections VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    section.id,
                    doc.path,
                    section.heading,
                    section.level,
                    parent,
                    ordinal as i64,
                    text,
                    serde_json::to_string(&section.span)?
                ],
            )?;
            tx.execute(
                "INSERT INTO section_fts VALUES (?1,?2,?3,?4,?5)",
                params![section.id, doc.path, title, section.heading, text],
            )?;
        }
        for (i, node) in doc.nodes.iter().enumerate() {
            let section = doc
                .sections
                .iter()
                .rev()
                .find(|s| node.span.start >= s.content_start && node.span.start < s.direct_end)
                .map(|s| &s.id);
            tx.execute(
                "INSERT INTO nodes VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    format!("{}:{i}", doc.path),
                    doc.path,
                    section,
                    node.kind,
                    node.language,
                    serde_json::to_string(&node.span)?
                ],
            )?;
        }
    }
    // Re-resolve the complete small PoC graph on each batch, including inbound dependencies.
    tx.execute("DELETE FROM relations", [])?;
    tx.execute("DELETE FROM diagnostics", [])?;
    for relation in &workspace.relations {
        tx.execute(
            "INSERT INTO relations VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                relation.source,
                relation.target,
                relation.kind,
                relation.reference,
                relation.status,
                serde_json::to_string(&relation.span)?
            ],
        )?;
    }
    let check = workspace.check();
    for d in &check.diagnostics {
        tx.execute(
            "INSERT INTO diagnostics(document_id,rule,severity,span,message,shacl) VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                d.path,
                d.rule,
                d.severity,
                serde_json::to_string(&d.span)?,
                d.message,
                d.shacl.as_ref().map(serde_json::to_string).transpose()?
            ],
        )?;
    }
    tx.execute(
        "INSERT OR REPLACE INTO engine_meta VALUES ('fingerprint',?1)",
        [&workspace.fingerprint],
    )?;
    tx.execute(
        "INSERT OR REPLACE INTO engine_meta VALUES ('format','1')",
        [],
    )?;
    tx.commit()?;
    Ok(IndexResult {
        contract_version: CONTRACT_VERSION,
        documents: workspace.documents.len(),
        updated: changed.len(),
        deleted: deleted.len(),
        diagnostics: check.diagnostics.len(),
    })
}

pub fn query(root: &Path, sql: &str) -> Result<Value> {
    use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
    let conn = reader(root)?;
    // Read-only connection AND deny-by-default authorizer; no attach, pragma, DDL, or extension loading.
    conn.authorizer(Some(|context: AuthContext<'_>| match context.action {
        AuthAction::Select | AuthAction::Recursive => Authorization::Allow,
        AuthAction::Read { table_name, .. }
            if ["documents", "sections", "nodes", "relations", "diagnostics"]
                .contains(&table_name) =>
        {
            Authorization::Allow
        }
        AuthAction::Function { function_name }
            if [
                "count",
                "sum",
                "avg",
                "min",
                "max",
                "total",
                "coalesce",
                "ifnull",
                "nullif",
                "lower",
                "upper",
                "length",
                "substr",
                "substring",
                "trim",
                "replace",
                "abs",
                "round",
                "json_extract",
                "json_array_length",
                "json_type",
                "->",
                "->>",
                "group_concat",
                "like",
                "glob",
            ]
            .contains(&function_name.to_lowercase().as_str()) =>
        {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }))?;
    let deadline = Instant::now() + Duration::from_secs(2);
    conn.progress_handler(1000, Some(move || Instant::now() > deadline))?;
    let mut statement = conn.prepare(sql)?;
    if !statement.readonly() {
        bail!("Only read-only SQL is allowed");
    }
    let columns: Vec<String> = statement
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    if columns.iter().collect::<BTreeSet<_>>().len() != columns.len() {
        bail!("Use unique column aliases");
    }
    let mut cursor = statement.query([])?;
    let mut rows = vec![];
    while let Some(row) = cursor.next()? {
        if rows.len() >= 1000 {
            bail!("Query exceeds 1000 rows; use LIMIT");
        }
        let mut object = serde_json::Map::new();
        for (i, name) in columns.iter().enumerate() {
            let value = match row.get_ref(i)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(v) => json!(v),
                ValueRef::Real(v) => json!(v),
                ValueRef::Text(v) => Value::String(std::str::from_utf8(v)?.into()),
                ValueRef::Blob(_) => bail!("BLOB output is unsupported"),
            };
            object.insert(name.clone(), value);
        }
        rows.push(Value::Object(object));
    }
    Ok(json!({ "contractVersion": CONTRACT_VERSION, "rows": rows }))
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchOptions {
    pub text: String,
    pub document_type: Option<String>,
    pub section: Option<String>,
    pub limit: Option<usize>,
}

pub fn search(root: &Path, options: &SearchOptions) -> Result<Value> {
    let conn = reader(root)?;
    // User text is a literal phrase, never SQL or FTS query syntax.
    if options.text.trim().is_empty() {
        bail!("Search text must not be empty");
    }
    let text = format!("\"{}\"", options.text.replace('"', "\"\""));
    let mut statement = conn.prepare("SELECT d.declared_id, d.path, s.section_id, s.heading, s.span, bm25(section_fts,0,0,3,2,1), snippet(section_fts,4,'[',']',' … ',24)
        FROM section_fts JOIN documents d ON d.path=section_fts.document_id JOIN sections s ON s.section_id=section_fts.section_id
        WHERE section_fts MATCH ?1 AND (?2 IS NULL OR d.type=?2) AND (?3 IS NULL OR s.heading=?3)
        ORDER BY bm25(section_fts,0,0,3,2,1), d.path, s.ordinal LIMIT ?4")?;
    let rows = statement.query_map(
        params![
            text,
            options.document_type,
            options.section,
            options.limit.unwrap_or(20).min(100) as i64
        ],
        |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, String>(6)?,
            ))
        },
    )?;
    let mut results = vec![];
    for row in rows {
        let (id, path, section_id, heading, span, score, snippet) = row?;
        results.push(json!({"documentId": id, "path": path, "sectionId": section_id, "section": heading, "location": serde_json::from_str::<Value>(&span)?, "score": -score, "snippet": snippet}));
    }
    Ok(json!({"contractVersion": CONTRACT_VERSION, "results": results}))
}

pub fn extract(root: &Path, section_id: &str) -> Result<Value> {
    let conn = reader(root)?;
    let (source, span, path): (String, String, String) = conn.query_row("SELECT d.source,s.span,d.path FROM sections s JOIN documents d ON d.path=s.document_id WHERE s.section_id=?1", [section_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let span: Span = serde_json::from_str(&span)?;
    Ok(
        json!({"contractVersion": CONTRACT_VERSION, "sectionId": section_id, "path": path, "location": span, "markdown": source[span.start..span.end]}),
    )
}

pub fn graph(root: &Path, id: &str, direction: &str, depth: usize) -> Result<Value> {
    if !["incoming", "outgoing"].contains(&direction) {
        bail!("direction must be incoming or outgoing");
    }
    if depth > 32 {
        bail!("Graph depth must be <= 32");
    }
    let conn = reader(root)?;
    let paths: Vec<String> = conn
        .prepare("SELECT path FROM documents WHERE declared_id=?1 OR path=?1 ORDER BY path")?
        .query_map([id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let [start] = paths.as_slice() else {
        bail!("Graph root missing or ambiguous: {id}");
    };
    let edges: Vec<(String, String, String)> = conn.prepare("SELECT DISTINCT source_document_id,target_document_id,kind FROM relations WHERE resolution_status='resolved' AND target_document_id IS NOT NULL ORDER BY 1,2,3")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut visited = BTreeSet::from([start.clone()]);
    let mut queue = VecDeque::from([(start.clone(), 0)]);
    let mut result_edges = BTreeSet::new();
    while let Some((node, distance)) = queue.pop_front() {
        if distance >= depth {
            continue;
        }
        for (source, target, kind) in &edges {
            let (from, to) = if direction == "outgoing" {
                (source, target)
            } else {
                (target, source)
            };
            if *from == node {
                result_edges.insert((source.clone(), target.clone(), kind.clone()));
                if visited.insert(to.clone()) {
                    queue.push_back((to.clone(), distance + 1));
                }
            }
        }
    }
    Ok(
        json!({"contractVersion": CONTRACT_VERSION, "root": start, "nodes": visited, "edges": result_edges.into_iter().map(|(s,t,k)| json!({"source":s,"target":t,"kind":k})).collect::<Vec<_>>()}),
    )
}
