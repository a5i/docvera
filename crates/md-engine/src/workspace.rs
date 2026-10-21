use crate::{
    config::Config,
    model::*,
    parser, policy,
    schema::{self, Schema},
};
use anyhow::{Context, Result, bail};
use globset::Glob;
use percent_encoding::percent_decode_str;
use serde_json::Value;
use std::{
    collections::{HashMap, hash_map::Entry},
    fs,
    path::{Component, Path, PathBuf},
};
use walkdir::WalkDir;

pub struct Workspace {
    pub root: PathBuf,
    pub documents: Vec<Document>,
    pub relations: Vec<Relation>,
    pub fingerprint: String,
}

pub fn portable(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn within(root: &Path, path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("Cannot read {}", path.display()))?;
    if !canonical.starts_with(root) {
        bail!("Path escapes workspace: {}", path.display());
    }
    Ok(canonical)
}

impl Workspace {
    pub fn load(root: &Path) -> Result<Self> {
        let root = root.canonicalize()?;
        let (config, config_text) = Config::load(&root)?;
        let policies = policy::Policies::load(&root, config.validation.shacl.as_ref())?;
        let includes = config
            .include
            .iter()
            .map(|p| Ok(Glob::new(p)?.compile_matcher()))
            .collect::<Result<Vec<_>>>()?;
        let excludes = config
            .exclude
            .iter()
            .map(|p| Ok(Glob::new(p)?.compile_matcher()))
            .collect::<Result<Vec<_>>>()?;
        let mappings = config
            .schemas
            .iter()
            .map(|(p, s)| Ok((Glob::new(p)?.compile_matcher(), s)))
            .collect::<Result<Vec<_>>>()?;
        let mut paths = vec![];
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                !e.file_type().is_dir()
                    || ![".git", ".mdx", "target", "node_modules"]
                        .contains(&e.file_name().to_string_lossy().as_ref())
            })
        {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let relative = entry.path().strip_prefix(&root)?;
            let path = portable(relative);
            if includes.iter().any(|g| g.is_match(&path))
                && !excludes.iter().any(|g| g.is_match(&path))
            {
                paths.push((path, entry.into_path()));
            }
        }
        paths.sort();
        let mut documents = vec![];
        let mut schemas = vec![];
        let mut fingerprint = blake3::Hasher::new();
        fingerprint.update(env!("CARGO_PKG_VERSION").as_bytes());
        fingerprint.update(config_text.as_bytes());
        policies.fingerprint(&mut fingerprint);
        // One compiled artifact per canonical schema path, shared by every
        // document mapped to the same file: schema compilation cost is paid
        // once per file instead of once per document.
        let mut compiled_schemas: HashMap<PathBuf, schema::Compiled> = HashMap::new();
        for (path, absolute) in paths {
            let text = fs::read_to_string(&absolute)
                .with_context(|| format!("Cannot read {}", absolute.display()))?;
            let mut doc = parser::parse(path.clone(), text);
            let selected = if let Some(selector) = doc.metadata.get("$schema") {
                selector
                    .as_str()
                    .map(|s| absolute.parent().unwrap().join(s))
                    .ok_or_else(|| anyhow::anyhow!("$schema must be a string"))
            } else {
                let matches: Vec<_> = mappings.iter().filter(|(g, _)| g.is_match(&path)).collect();
                match matches.as_slice() {
                    [(_, schema)] => Ok(root.join(schema)),
                    [] => Err(anyhow::anyhow!("No schema mapping for {path}")),
                    _ => Err(anyhow::anyhow!(
                        "Multiple schema mappings for {path}; use explicit $schema"
                    )),
                }
            };
            let resolved: Result<()> = (|| {
                let schema_path = within(&root, &selected?)?;
                let compiled = match compiled_schemas.entry(schema_path) {
                    Entry::Occupied(entry) => {
                        fingerprint.update(entry.get().text().as_bytes());
                        entry.into_mut()
                    }
                    Entry::Vacant(vacant) => {
                        let path = vacant.key().clone();
                        let text = fs::read_to_string(&path)
                            .with_context(|| format!("Cannot read {}", path.display()))?;
                        fingerprint.update(text.as_bytes());
                        let schema: Schema = serde_saphyr::from_str(&text)?;
                        schema.check_definition()?;
                        vacant.insert(schema::Compiled::compile(text, &schema)?)
                    }
                };
                schema::validate(&mut doc, compiled.schema(), compiled)?;
                schemas.push(Some(compiled.schema().clone()));
                Ok(())
            })();
            match resolved {
                Ok(()) => {}
                Err(error) => {
                    doc.diagnostic(doc.metadata_span(), "schema.resolve", format!("{error:#}"));
                    schemas.push(None);
                }
            }
            documents.push(doc);
        }
        // Validate all members of each collision group, retaining path-based internal keys.
        for i in 0..documents.len() {
            let Some(identity) = schemas[i].as_ref().and_then(|s| s.identity.as_ref()) else {
                continue;
            };
            let Some(id) = documents[i].declared_id.clone() else {
                continue;
            };
            let collisions: Vec<String> = documents
                .iter()
                .enumerate()
                .filter(|(j, d)| {
                    *j != i
                        && d.declared_id.as_ref() == Some(&id)
                        && match identity.unique_within.as_str() {
                            "documentType" => d.document_type == documents[i].document_type,
                            "directory" => {
                                Path::new(&d.path).parent()
                                    == Path::new(&documents[i].path).parent()
                            }
                            _ => true,
                        }
                })
                .map(|(_, d)| d.path.clone())
                .collect();
            if !collisions.is_empty() {
                let span = documents[i].metadata_span();
                documents[i].diagnostic(
                    span,
                    "identity.duplicate",
                    format!("Duplicate ID {id}: {}", collisions.join(", ")),
                );
            }
        }
        let mut relations = vec![];
        for i in 0..documents.len() {
            if let Some(schema) = &schemas[i] {
                for rule in &schema.relations {
                    let value = schema::field(&documents[i].metadata, &rule.from);
                    let values: Vec<Value> = match value {
                        Some(Value::Array(v)) => v.clone(),
                        Some(v) => vec![v.clone()],
                        None => vec![],
                    };
                    if values.len() < rule.min || rule.max.is_some_and(|max| values.len() > max) {
                        let span = documents[i].field_span(&rule.from);
                        documents[i].diagnostic(
                            span,
                            "relation.cardinality",
                            format!("{} has {} targets", rule.from, values.len()),
                        );
                    }
                    for (item_index, value) in values.into_iter().enumerate() {
                        let span = documents[i].field_item_span(&rule.from, item_index);
                        if let Some(reference) = value.as_str() {
                            let relation = resolve(
                                &root,
                                &documents,
                                i,
                                reference,
                                rule.target_type.as_deref(),
                                &rule.from,
                                span.clone(),
                            );
                            if relation.status != "resolved"
                                && (rule.must_exist || relation.status != "missing")
                            {
                                documents[i].diagnostic(
                                    span,
                                    "relation.target",
                                    format!("{reference}: {}", relation.status),
                                );
                            }
                            relations.push(relation);
                        } else {
                            documents[i].diagnostic(
                                span,
                                "relation.value",
                                "Reference must be a string",
                            );
                        }
                    }
                }
            }
            for link in documents[i].links.clone() {
                if external(&link.target) {
                    continue;
                }
                let relation = resolve(
                    &root,
                    &documents,
                    i,
                    &link.target,
                    None,
                    "link",
                    link.span.clone(),
                );
                if relation.status != "resolved" {
                    documents[i].diagnostic(
                        link.span,
                        "link.target",
                        format!("{}: {}", link.target, relation.status),
                    );
                }
                relations.push(relation);
            }
        }
        policies.validate(&mut documents, &relations)?;
        Ok(Self {
            root,
            documents,
            relations,
            fingerprint: fingerprint.finalize().to_hex().to_string(),
        })
    }
    pub fn check(&self) -> CheckResult {
        let mut diagnostics: Vec<_> = self
            .documents
            .iter()
            .flat_map(|d| d.diagnostics.clone())
            .collect();
        diagnostics.sort_by(|a, b| {
            (
                &a.path,
                a.span.start,
                &a.rule,
                &a.message,
                &a.severity,
                &a.shacl,
                a.span.end,
            )
                .cmp(&(
                    &b.path,
                    b.span.start,
                    &b.rule,
                    &b.message,
                    &b.severity,
                    &b.shacl,
                    b.span.end,
                ))
        });
        CheckResult {
            contract_version: CONTRACT_VERSION,
            documents: self.documents.len(),
            diagnostics,
        }
    }
}

fn external(reference: &str) -> bool {
    if reference.starts_with("id:") {
        return false;
    }
    reference.starts_with("//")
        || reference.split_once(':').is_some_and(|(scheme, _)| {
            !scheme.is_empty()
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ['+', '-', '.'].contains(&c))
        })
}

fn resolve(
    root: &Path,
    docs: &[Document],
    source: usize,
    reference: &str,
    target_type: Option<&str>,
    kind: &str,
    span: Span,
) -> Relation {
    let mut relation = Relation {
        source: docs[source].path.clone(),
        target: None,
        reference: reference.into(),
        kind: kind.into(),
        status: "missing".into(),
        span,
    };
    let decoded = match percent_decode_str(reference).decode_utf8() {
        Ok(s) => s,
        Err(_) => {
            relation.status = "invalid-encoding".into();
            return relation;
        }
    };
    let (reference, anchor) = decoded
        .split_once('#')
        .map_or((decoded.as_ref(), None), |(p, a)| (p, Some(a)));
    let explicit_id = reference.starts_with("id:");
    let reference = reference.strip_prefix("id:").unwrap_or(reference);
    let candidates: Vec<_> = docs
        .iter()
        .filter(|d| d.declared_id.as_deref() == Some(reference))
        .collect();
    let typed: Vec<_> = candidates
        .iter()
        .copied()
        .filter(|d| target_type.is_none_or(|t| d.document_type.as_deref() == Some(t)))
        .collect();
    let target = if !candidates.is_empty() {
        if typed.len() != 1 {
            relation.status = if typed.is_empty() {
                "wrong-type"
            } else {
                "ambiguous"
            }
            .into();
            return relation;
        }
        Some(typed[0])
    } else {
        if explicit_id {
            return relation;
        }
        let base = Path::new(&docs[source].path)
            .parent()
            .unwrap_or(Path::new(""));
        let path = if reference.is_empty() {
            PathBuf::from(&docs[source].path)
        } else {
            base.join(reference.split('?').next().unwrap_or(reference))
        };
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::Normal(part) => normalized.push(part),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !normalized.pop() {
                        relation.status = "outside-workspace".into();
                        return relation;
                    }
                }
                _ => {
                    relation.status = "outside-workspace".into();
                    return relation;
                }
            }
        }
        let absolute = root.join(&normalized);
        if absolute.exists() && within(root, &absolute).is_err() {
            relation.status = "outside-workspace".into();
            return relation;
        }
        let found = docs.iter().find(|d| d.path == portable(&normalized));
        if found.is_none()
            && target_type.is_none()
            && anchor.is_none()
            && absolute.is_file()
            && normalized
                .extension()
                .is_none_or(|e| e != "md" && e != "markdown")
        {
            relation.status = "resolved".into();
            return relation;
        }
        found
    };
    if let Some(target) = target {
        relation.target = Some(target.path.clone());
        relation.status =
            if target_type.is_some_and(|t| target.document_type.as_deref() != Some(t)) {
                "wrong-type"
            } else if anchor
                .is_some_and(|a| !a.is_empty() && !target.sections.iter().any(|s| s.anchor == a))
            {
                "missing-anchor"
            } else {
                "resolved"
            }
            .into();
    }
    relation
}
