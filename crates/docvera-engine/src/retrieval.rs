//! Retrieval directly from the validated in-memory Workspace; no persistent index.
use crate::{Workspace, model::CONTRACT_VERSION};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeSet, VecDeque};

pub fn extract(workspace: &Workspace, section_id: &str) -> Result<Value> {
    let mut found = workspace.documents.iter().flat_map(|d| {
        d.sections
            .iter()
            .filter(move |s| s.id == section_id)
            .map(move |s| (d, s))
    });
    let Some((document, section)) = found.next() else {
        bail!("Section not found: {section_id}")
    };
    if found.next().is_some() {
        bail!("Ambiguous section: {section_id}");
    }
    let span = &section.span;
    let Some(markdown) = document.source.get(span.start..span.end) else {
        bail!("Invalid section span: {section_id}");
    };
    Ok(
        json!({"contractVersion": CONTRACT_VERSION, "sectionId": section_id,
        "path": document.path, "location": span, "markdown": markdown}),
    )
}

pub fn graph(workspace: &Workspace, id: &str, direction: &str, depth: usize) -> Result<Value> {
    if !["incoming", "outgoing"].contains(&direction) {
        bail!("direction must be incoming or outgoing");
    }
    if depth > 32 {
        bail!("Graph depth must be <= 32");
    }
    let paths: Vec<_> = workspace
        .documents
        .iter()
        .filter(|d| d.path == id || d.declared_id.as_deref() == Some(id))
        .map(|d| d.path.clone())
        .collect();
    let [start] = paths.as_slice() else {
        bail!("Graph root missing or ambiguous: {id}")
    };
    let edges: BTreeSet<_> = workspace
        .relations
        .iter()
        .filter(|r| r.status == "resolved")
        .filter_map(|r| Some((r.source.clone(), r.target.clone()?, r.kind.clone())))
        .collect();
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
        json!({"contractVersion": CONTRACT_VERSION, "root": start, "nodes": visited,
        "edges": result_edges.into_iter().map(|(s,t,k)| json!({"source":s,"target":t,"kind":k})).collect::<Vec<_>>()}),
    )
}

impl Workspace {
    pub fn extract(&self, section_id: &str) -> Result<Value> {
        extract(self, section_id)
    }
    pub fn graph(&self, id: &str, direction: &str, depth: usize) -> Result<Value> {
        graph(self, id, direction, depth)
    }
    /// Read-only SQL over a temporary projection of this snapshot. Requires the `sql` feature.
    #[cfg(feature = "sql")]
    pub fn query_sql(&self, sql: &str) -> Result<Value> {
        crate::sql::SqlSnapshot::from_workspace(self)?.query(sql)
    }
}
