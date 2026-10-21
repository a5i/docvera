//! SHACL is the only cross-document policy language. RDF is derived, never persisted.
use crate::{
    config::Shacl,
    model::{Document, Relation},
    workspace::within,
};
use anyhow::{Context, Result, bail};
use oxrdf::{BlankNode, NamedOrBlankNode, Term, Triple};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use rudof_rdf::{
    rdf_core::{RDFFormat, term::Object},
    rdf_impl::{OxigraphInMemory, ReaderMode},
};
use serde_json::Value;
use shacl::{
    ir::IRSchema,
    rdf::ShaclParser,
    types::Severity,
    validator::{
        ShaclConfig, ShaclValidationMode,
        processor::{GraphValidation, ShaclProcessor},
        store::Graph,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Cursor,
    path::Path,
};

const SH: &str = "http://www.w3.org/ns/shacl#";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
const VOCAB: &str = "urn:mdx:v1:";

pub struct Policies {
    schema: Option<IRSchema>,
    sources: Vec<(String, String)>,
}

fn encode(value: &str) -> String {
    const ESCAPE: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'_');
    utf8_percent_encode(value, ESCAPE).to_string()
}
fn parse(text: &str, name: &str) -> Result<OxigraphInMemory> {
    Ok(OxigraphInMemory::from_reader(
        &mut Cursor::new(text),
        name,
        &RDFFormat::Turtle,
        None,
        &ReaderMode::Strict,
    )?)
}

impl Policies {
    pub fn load(root: &Path, config: Option<&Shacl>) -> Result<Self> {
        let mut sources = BTreeMap::new();
        if let Some(config) = config {
            let mut paths = BTreeSet::new();
            for path in &config.shapes {
                if Path::new(path).is_absolute()
                    || Path::new(path).extension().and_then(|s| s.to_str()) != Some("ttl")
                {
                    bail!("SHACL shapes must be workspace-relative .ttl paths: {path}");
                }
                let canonical = within(root, &root.join(path))
                    .with_context(|| format!("Cannot load SHACL shapes {path}"))?;
                if !paths.insert(canonical.clone()) {
                    bail!("Duplicate SHACL shapes file: {path}");
                }
                sources.insert(path.clone(), fs::read_to_string(canonical)?);
            }
        }
        Self::from_sources(config, &sources)
    }

    pub fn from_sources(config: Option<&Shacl>, input: &BTreeMap<String, String>) -> Result<Self> {
        let mut sources = vec![];
        let mut merged = String::new();
        if let Some(config) = config {
            if config.shapes.is_empty() {
                bail!("SHACL shapes must not be empty");
            }
            let mut paths = BTreeSet::new();
            for path in &config.shapes {
                crate::checker::normalize_path(path)?;
                if Path::new(path).extension().and_then(|s| s.to_str()) != Some("ttl")
                    || !paths.insert(path)
                {
                    bail!("SHACL shapes must be unique workspace-relative .ttl paths: {path}");
                }
                let mut text = input
                    .get(path)
                    .with_context(|| format!("Cannot load SHACL shapes {path}"))?
                    .clone();
                // Turtle permits a BOM only as the first character. Oxigraph's
                // lexer can swallow a BOM into the following token (observed:
                // `sh:NodeShape\ufeff` corrupts the rdf:type IRI so the shape
                // silently disappears), so normalize one leading BOM and
                // reject every other occurrence instead of trusting the lexer.
                if text.starts_with('\u{feff}') {
                    text.remove(0);
                }
                if text.contains('\u{feff}') {
                    bail!(
                        "SHACL shapes {path}: a BOM is only allowed as the first character of the file"
                    );
                }
                sources.push((path.clone(), text));
            }
            sources.sort_by(|a, b| a.0.cmp(&b.0));
            for (file_index, (name, text)) in sources.iter().enumerate() {
                let graph =
                    parse(text, name).with_context(|| format!("Invalid SHACL Turtle in {name}"))?;
                // Each file is parsed independently: prefixes/base declarations and blank
                // node labels must not leak across shape-file boundaries.
                let mut blanks = BTreeMap::new();
                for quad in graph.quads() {
                    preflight(quad.predicate.as_str(), &quad.object)
                        .with_context(|| format!("Unsupported SHACL policy in {name}"))?;
                    let mut rename = |node: BlankNode| {
                        let next = blanks.len();
                        blanks
                            .entry(node.as_str().to_owned())
                            .or_insert_with(|| {
                                BlankNode::new_unchecked(format!("f{file_index}b{next}"))
                            })
                            .clone()
                    };
                    let subject = match quad.subject {
                        NamedOrBlankNode::BlankNode(node) => rename(node).into(),
                        other => other,
                    };
                    let object = match quad.object {
                        Term::BlankNode(node) => rename(node).into(),
                        other => other,
                    };
                    merged.push_str(&format!(
                        "{} .\n",
                        Triple::new(subject, quad.predicate, object)
                    ));
                }
            }
        }
        let schema = if config.is_some() {
            let graph = parse(&merged, "merged SHACL shapes")?;
            check_shape_structure(&graph)?;
            let ast = ShaclParser::new(graph)
                .parse()
                .context("Cannot parse SHACL shapes")?;
            let schema = IRSchema::compile(&ast).context("Cannot compile SHACL shapes")?;
            if schema.iter_with_targets().next().is_none() {
                bail!("SHACL shapes graph has no shapes with targets");
            }
            Some(schema)
        } else {
            None
        };
        Ok(Self { schema, sources })
    }

    pub fn fingerprint(&self, hasher: &mut blake3::Hasher) {
        for (path, text) in &self.sources {
            hasher.update(&(path.len() as u64).to_le_bytes());
            hasher.update(path.as_bytes());
            hasher.update(&(text.len() as u64).to_le_bytes());
            hasher.update(text.as_bytes());
        }
    }

    pub fn validate(&self, docs: &mut [Document], relations: &[Relation]) -> Result<()> {
        let Some(schema) = &self.schema else {
            return Ok(());
        };
        let mut ids = BTreeSet::new();
        let duplicates: BTreeSet<_> = docs
            .iter()
            .filter_map(|d| d.declared_id.as_ref())
            .filter(|id| !ids.insert((*id).clone()))
            .cloned()
            .collect();
        if !duplicates.is_empty() {
            for doc in docs.iter_mut().filter(|d| {
                d.declared_id
                    .as_ref()
                    .is_some_and(|id| duplicates.contains(id))
            }) {
                doc.diagnostic(
                    doc.metadata_span(),
                    "shacl.incomplete",
                    "SHACL evaluation aborted: document IDs must be globally unique",
                );
            }
            return Ok(());
        }
        let projection = Projection::new(docs, relations)?;
        let targets = projection
            .documents
            .iter()
            .map(|(iri, &index)| (iri.clone(), docs[index].path.clone()))
            .collect();
        let graph = parse(&projection.turtle, "workspace RDF projection")?;
        let report = GraphValidation::new(Graph::try_from(graph)?)
            .validate(
                schema,
                &ShaclValidationMode::Native,
                &ShaclConfig::default(),
            )
            .context("SHACL engine failed; workspace was not validated")?;
        if !report.conforms() && report.results().is_empty() {
            bail!("SHACL engine returned non-conformance without diagnostics");
        }
        for result in report.results() {
            let Object::Iri(focus) = result.focus_node() else {
                bail!(
                    "SHACL focus node is not a projected document: {}",
                    result.focus_node()
                );
            };
            let Some(&index) = projection.documents.get(focus.as_str()) else {
                bail!("SHACL focus node cannot be mapped to Markdown: {focus}");
            };
            let component = match result.constraint_component() {
                Object::Iri(iri) => iri.as_str().strip_prefix(SH).unwrap_or("Constraint"),
                _ => "Constraint",
            };
            let rule = format!("shacl.{component}");
            let mut messages: Vec<_> = result.message().messages().values().cloned().collect();
            messages.sort();
            messages.dedup();
            let mut message = if messages.is_empty() {
                component.to_string()
            } else {
                messages.join("; ")
            };
            if let Some(Object::Iri(shape)) = result.source() {
                message.push_str(&format!(" [shape: {shape}]"));
            }
            if let Some(path) = result.path() {
                message.push_str(&format!(" [path: {path}]"));
            }
            if let Some(value) = result.value() {
                // Document IRIs are stable; do not expose unstable blank-node IDs.
                if !matches!(value, Object::BlankNode(_)) {
                    message.push_str(&format!(" [value: {value}]"));
                }
            }
            let mut structured = crate::shacl_report::normalize(result)?;
            let doc = &mut docs[index];
            let (span, source_field) =
                crate::shacl_report::locate(doc, &structured, relations, &targets);
            structured.source_field = source_field;
            doc.diagnostic(span, &rule, message);
            doc.diagnostics.last_mut().unwrap().shacl = Some(structured);
            doc.diagnostics.last_mut().unwrap().severity = match result.severity() {
                Severity::Warning => "warning",
                Severity::Info => "info",
                _ => "error",
            }
            .into();
        }
        Ok(())
    }
}

// Rudof can omit incomplete constraint nodes while parsing. Verify mandatory
// structural fields before handing over the merged graph, so malformed policies
// cannot accidentally become a conforming no-op.
fn check_shape_structure(graph: &OxigraphInMemory) -> Result<()> {
    let quads: Vec<_> = graph.quads().collect();
    for quad in &quads {
        let requirement = if quad.predicate.as_str() == format!("{SH}sparql") {
            Some((&quad.object, "select"))
        } else if quad.predicate.as_str() == format!("{SH}property") {
            Some((&quad.object, "path"))
        } else {
            None
        };
        if let Some((node, property)) = requirement {
            let values: Vec<_> = quads
                .iter()
                .filter(|q| {
                    Term::from(q.subject.clone()) == *node
                        && q.predicate.as_str() == format!("{SH}{property}")
                })
                .collect();
            if values.len() != 1 {
                bail!("Invalid SHACL shape: {node} must have exactly one sh:{property}");
            }
        }
        if quad.predicate.as_str() == RDF_TYPE
            && matches!(&quad.object, Term::NamedNode(iri) if iri.as_str() == format!("{SH}PropertyShape"))
        {
            let paths = quads
                .iter()
                .filter(|q| {
                    q.subject == quad.subject && q.predicate.as_str() == format!("{SH}path")
                })
                .count();
            if paths != 1 {
                bail!(
                    "Invalid SHACL PropertyShape: {} must have exactly one sh:path",
                    quad.subject
                );
            }
        }
    }
    Ok(())
}

// Fail closed on SHACL extensions/typos rather than silently ignoring policies.
// This is a bounded 2017 subset, not a claim of complete SHACL conformance.
fn preflight(predicate: &str, object: &Term) -> Result<()> {
    if let Some(local) = predicate.strip_prefix(SH) {
        const SUPPORTED: &[&str] = &[
            "targetClass",
            "targetNode",
            "targetSubjectsOf",
            "targetObjectsOf",
            "property",
            "path",
            "inversePath",
            "alternativePath",
            "zeroOrMorePath",
            "oneOrMorePath",
            "zeroOrOnePath",
            "minCount",
            "maxCount",
            "class",
            "datatype",
            "nodeKind",
            "minExclusive",
            "minInclusive",
            "maxExclusive",
            "maxInclusive",
            "minLength",
            "maxLength",
            "pattern",
            "flags",
            "languageIn",
            "uniqueLang",
            "equals",
            "disjoint",
            "lessThan",
            "lessThanOrEquals",
            "not",
            "and",
            "or",
            "xone",
            "node",
            "qualifiedValueShape",
            "qualifiedMinCount",
            "qualifiedMaxCount",
            "qualifiedValueShapesDisjoint",
            "closed",
            "ignoredProperties",
            "hasValue",
            "in",
            "severity",
            "message",
            "deactivated",
            "name",
            "description",
            "order",
            "group",
            "sparql",
            "select",
        ];
        if !SUPPORTED.contains(&local) {
            bail!("Unsupported SHACL predicate: {predicate}");
        }
        if local == "select" {
            let Term::Literal(query) = object else {
                bail!("sh:select must be a literal");
            };
            // Policies are workspace-trusted, not sandboxed. Conservatively reject
            // remote/dataset clauses, even when the words occur in query strings.
            if regex::Regex::new(r"(?i)\b(SERVICE|FROM)\b")
                .unwrap()
                .is_match(query.value())
            {
                bail!("SPARQL SERVICE/FROM is disabled; validation is local-only");
            }
        }
    }
    if predicate == "http://www.w3.org/2002/07/owl#imports" {
        bail!("owl:imports shapes are unsupported; list local .ttl files in mdx.toml");
    }
    Ok(())
}

struct Projection {
    turtle: String,
    documents: BTreeMap<String, usize>,
}
impl Projection {
    fn new(docs: &[Document], relations: &[Relation]) -> Result<Self> {
        let mut ids = BTreeSet::new();
        let mut paths = BTreeMap::new();
        let mut documents = BTreeMap::new();
        for (index, doc) in docs.iter().enumerate() {
            let iri = if let Some(id) = &doc.declared_id {
                if !ids.insert(id) {
                    bail!(
                        "SHACL projection requires globally unique document IDs; duplicate: {id}"
                    );
                }
                format!("urn:mdx:document:id:{}", encode(id))
            } else {
                format!("urn:mdx:document:path:{}", encode(&doc.path))
            };
            paths.insert(doc.path.as_str(), iri.clone());
            documents.insert(iri, index);
        }
        let mut turtle = String::new();
        for doc in docs {
            let iri = &paths[doc.path.as_str()];
            emit(
                &mut turtle,
                iri,
                &format!("{VOCAB}path"),
                &literal(&doc.path),
            );
            emit(
                &mut turtle,
                iri,
                &format!("{VOCAB}body"),
                &literal(&doc.source[doc.body_start..]),
            );
            if let Some(kind) = &doc.document_type {
                emit(
                    &mut turtle,
                    iri,
                    RDF_TYPE,
                    &format!("<{VOCAB}type:{}>", encode(kind)),
                );
            }
            if let Some(metadata) = doc.metadata.as_object() {
                for (key, value) in metadata {
                    if key == "$schema" {
                        continue;
                    }
                    project_value(&mut turtle, iri, &encode(key), value, &doc.path, relations)?;
                }
            }
        }
        for relation in relations.iter().filter(|r| r.status == "resolved") {
            let Some(target) = relation.target.as_deref().and_then(|p| paths.get(p)) else {
                continue;
            };
            let predicate = if relation.kind == "link" {
                format!("{VOCAB}link")
            } else {
                let Some(field) = relation.kind.strip_prefix("frontMatter.") else {
                    continue;
                };
                format!(
                    "{VOCAB}field:{}",
                    field.split('.').map(encode).collect::<Vec<_>>().join(".")
                )
            };
            emit(
                &mut turtle,
                &paths[relation.source.as_str()],
                &predicate,
                &format!("<{target}>"),
            );
        }
        Ok(Self { turtle, documents })
    }
}
fn literal(text: &str) -> String {
    serde_json::to_string(text).unwrap()
}
fn emit(out: &mut String, subject: &str, predicate: &str, object: &str) {
    out.push_str(&format!("<{subject}> <{predicate}> {object} .\n"));
}
fn project_value(
    out: &mut String,
    iri: &str,
    path: &str,
    value: &Value,
    doc: &str,
    relations: &[Relation],
) -> Result<()> {
    // Declared relation fields are object properties, never raw ID literals.
    if relations.iter().any(|r| {
        r.source == doc
            && r.kind.strip_prefix("frontMatter.").is_some_and(|field| {
                field.split('.').map(encode).collect::<Vec<_>>().join(".") == path
            })
    }) {
        return Ok(());
    }
    match value {
        Value::Null => {}
        Value::Array(items) => {
            for item in items {
                project_value(out, iri, path, item, doc, relations)?;
            }
        }
        Value::Object(fields) => {
            for (key, value) in fields {
                project_value(
                    out,
                    iri,
                    &format!("{path}.{}", encode(key)),
                    value,
                    doc,
                    relations,
                )?;
            }
        }
        Value::String(text) => emit(out, iri, &format!("{VOCAB}field:{path}"), &literal(text)),
        Value::Bool(value) => emit(
            out,
            iri,
            &format!("{VOCAB}field:{path}"),
            &format!("\"{value}\"^^<{XSD}boolean>"),
        ),
        Value::Number(number) => {
            let datatype = if number.is_i64() || number.is_u64() {
                "integer"
            } else {
                "double"
            };
            emit(
                out,
                iri,
                &format!("{VOCAB}field:{path}"),
                &format!("\"{number}\"^^<{XSD}{datatype}>"),
            );
        }
    }
    Ok(())
}
