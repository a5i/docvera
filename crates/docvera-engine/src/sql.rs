//! Experimental read-only SQL over an owned, in-memory projection of a Workspace.
//! No filesystem, SQLite, persistent index or full-text search is involved.
use crate::{Workspace, model::CONTRACT_VERSION};
use anyhow::{Result, bail};
use gluesql_core::{
    ast::{ColumnDef, DataType},
    data::{Key, Schema, Value as SqlValue},
    plan::StatementPlan,
    prelude::{Glue, Payload},
    store::{self, RowIter, Store},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

type SqlResult<T> = gluesql_core::error::Result<T>;

struct Table {
    schema: Schema,
    rows: Vec<Vec<SqlValue>>,
}

#[derive(Default)]
struct SnapshotStorage {
    tables: BTreeMap<String, Table>,
}

impl SnapshotStorage {
    fn table(&mut self, name: &str, columns: &[(&str, DataType)]) -> &mut Vec<Vec<SqlValue>> {
        &mut self
            .tables
            .entry(name.into())
            .or_insert_with(|| Table {
                schema: Schema {
                    table_name: name.into(),
                    column_defs: Some(
                        columns
                            .iter()
                            .map(|(name, ty)| ColumnDef {
                                name: (*name).into(),
                                data_type: ty.clone(),
                                nullable: true,
                                default: None,
                                unique: None,
                                comment: None,
                            })
                            .collect(),
                    ),
                    indexes: vec![],
                    engine: None,
                    foreign_keys: vec![],
                    comment: None,
                },
                rows: vec![],
            })
            .rows
    }
}

impl Store for SnapshotStorage {
    fn fetch_schema(&self, name: &str) -> SqlResult<Option<Schema>> {
        Ok(self.tables.get(name).map(|t| t.schema.clone()))
    }
    fn fetch_all_schemas(&self) -> SqlResult<Vec<Schema>> {
        Ok(self.tables.values().map(|t| t.schema.clone()).collect())
    }
    fn fetch_data(&self, name: &str, key: &Key) -> SqlResult<Option<Vec<SqlValue>>> {
        let Key::I64(index) = key else {
            return Ok(None);
        };
        Ok(usize::try_from(*index)
            .ok()
            .and_then(|i| self.tables.get(name)?.rows.get(i))
            .cloned())
    }
    fn scan_data<'a>(&'a self, name: &str) -> SqlResult<RowIter<'a>> {
        let rows = self
            .tables
            .get(name)
            .into_iter()
            .flat_map(|t| t.rows.iter());
        Ok(Box::new(
            rows.enumerate()
                .map(|(i, row)| Ok((Key::I64(i as i64), row.clone()))),
        ))
    }
}

// Default mutation methods all reject writes. No custom functions or external storage.
impl store::StoreMut for SnapshotStorage {}
impl store::Index for SnapshotStorage {}
impl store::IndexMut for SnapshotStorage {}
impl store::AlterTable for SnapshotStorage {}
impl store::Transaction for SnapshotStorage {}
impl store::Metadata for SnapshotStorage {}
impl store::CustomFunction for SnapshotStorage {}
impl store::CustomFunctionMut for SnapshotStorage {}
impl store::Planner for SnapshotStorage {}

fn text(value: &str) -> SqlValue {
    SqlValue::Str(value.into())
}
fn optional(value: Option<&str>) -> SqlValue {
    value.map(text).unwrap_or(SqlValue::Null)
}
fn structured(value: Value) -> Result<SqlValue> {
    Ok(SqlValue::try_from(value)?)
}

/// Reusable SQL projection. Independent of subsequent Workspace/checker updates.
/// Construction copies data; scans are not indexed. This is not an untrusted-query sandbox.
pub struct SqlSnapshot {
    glue: Glue<SnapshotStorage>,
}

impl SqlSnapshot {
    pub fn from_workspace(workspace: &Workspace) -> Result<Self> {
        use DataType::{Int, Map, Text};
        let mut storage = SnapshotStorage::default();
        let documents = storage.table(
            "documents",
            &[
                ("internal_id", Text),
                ("declared_id", Text),
                ("type", Text),
                ("path", Text),
                ("metadata", Map),
                ("hash", Text),
                ("source", Text),
            ],
        );
        for d in &workspace.documents {
            documents.push(vec![
                text(&d.path),
                optional(d.declared_id.as_deref()),
                optional(d.document_type.as_deref()),
                text(&d.path),
                structured(d.metadata.clone())?,
                text(&d.hash),
                text(&d.source),
            ]);
        }
        let sections = storage.table(
            "sections",
            &[
                ("section_id", Text),
                ("document_id", Text),
                ("heading", Text),
                ("level", Int),
                ("parent_id", Text),
                ("ordinal", Int),
                ("text", Text),
                ("span", Map),
            ],
        );
        for d in &workspace.documents {
            for (ordinal, s) in d.sections.iter().enumerate() {
                let content = d
                    .source
                    .get(s.content_start..s.direct_end)
                    .ok_or_else(|| anyhow::anyhow!("Invalid section span: {}", s.id))?;
                sections.push(vec![
                    text(&s.id),
                    text(&d.path),
                    text(&s.heading),
                    SqlValue::I64(s.level.into()),
                    optional(
                        s.parent
                            .and_then(|i| d.sections.get(i))
                            .map(|s| s.id.as_str()),
                    ),
                    SqlValue::I64(ordinal as i64),
                    text(content),
                    structured(serde_json::to_value(&s.span)?)?,
                ]);
            }
        }
        let nodes = storage.table(
            "nodes",
            &[
                ("node_id", Text),
                ("document_id", Text),
                ("section_id", Text),
                ("kind", Text),
                ("language", Text),
                ("span", Map),
            ],
        );
        for d in &workspace.documents {
            for (i, n) in d.nodes.iter().enumerate() {
                let section = d
                    .sections
                    .iter()
                    .rev()
                    .find(|s| n.span.start >= s.content_start && n.span.start < s.direct_end);
                nodes.push(vec![
                    text(&format!("{}:{i}", d.path)),
                    text(&d.path),
                    optional(section.map(|s| s.id.as_str())),
                    text(&n.kind),
                    optional(n.language.as_deref()),
                    structured(serde_json::to_value(&n.span)?)?,
                ]);
            }
        }
        let relations = storage.table(
            "relations",
            &[
                ("source_document_id", Text),
                ("target_document_id", Text),
                ("kind", Text),
                ("reference", Text),
                ("resolution_status", Text),
                ("source_span", Map),
            ],
        );
        for r in &workspace.relations {
            relations.push(vec![
                text(&r.source),
                optional(r.target.as_deref()),
                text(&r.kind),
                text(&r.reference),
                text(&r.status),
                structured(serde_json::to_value(&r.span)?)?,
            ]);
        }
        let diagnostics = storage.table(
            "diagnostics",
            &[
                ("document_id", Text),
                ("rule", Text),
                ("severity", Text),
                ("span", Map),
                ("message", Text),
                ("shacl", Map),
            ],
        );
        for d in workspace.check().diagnostics {
            diagnostics.push(vec![
                text(&d.path),
                text(&d.rule),
                text(&d.severity),
                structured(serde_json::to_value(&d.span)?)?,
                text(&d.message),
                structured(serde_json::to_value(&d.shacl)?)?,
            ]);
        }
        Ok(Self {
            glue: Glue::new(storage),
        })
    }

    /// Execute exactly one query. Duplicate labels and results exceeding 1000 rows are errors.
    /// The row limit is checked AFTER evaluation; no execution timeout is provided.
    pub fn query(&mut self, sql: &str) -> Result<Value> {
        let plans = self.glue.plan(sql)?;
        let [plan] = plans.as_slice() else {
            bail!("Exactly one read-only SQL query is required")
        };
        if !matches!(plan, StatementPlan::Query(_)) {
            bail!("Only read-only SQL queries are allowed");
        }
        let Payload::Select { labels, rows } = self.glue.execute_stmt(plan)? else {
            bail!("Expected a SELECT result");
        };
        if labels.iter().collect::<BTreeSet<_>>().len() != labels.len() {
            bail!("Use unique column aliases");
        }
        if rows.len() > 1000 {
            bail!("Query exceeds 1000 rows; use LIMIT");
        }
        let rows = rows
            .into_iter()
            .map(|row| {
                let object = labels
                    .iter()
                    .cloned()
                    .zip(row.into_iter().map(Value::try_from))
                    .map(|(label, value)| Ok((label, value?)))
                    .collect::<SqlResult<serde_json::Map<_, _>>>()?;
                Ok(Value::Object(object))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({ "contractVersion": CONTRACT_VERSION, "rows": rows }))
    }
}
