//! Public report normalization and conservative Markdown provenance.
use crate::{
    model::{Document, RdfPath, RdfTerm, Relation, ShaclMessage, ShaclResult, Span},
    source_map::pointer_segment,
};
use anyhow::{Result, bail};
use oxrdf::Term;
use rudof_rdf::rdf_core::{SHACLPath, term::Object};
use serde_json::Value;
use shacl::{types::Severity, validator::report::ValidationResult};
use std::collections::BTreeMap;

const SH: &str = "http://www.w3.org/ns/shacl#";
const FIELDS: &str = "urn:mdx:v1:field:";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

pub fn term(value: &Object) -> Result<RdfTerm> {
    match Term::from(value.clone()) {
        Term::NamedNode(iri) => Ok(RdfTerm::Iri {
            value: iri.as_str().into(),
        }),
        Term::BlankNode(_) => Ok(RdfTerm::BlankNode),
        Term::Literal(literal) => Ok(RdfTerm::Literal {
            value: literal.value().into(),
            datatype: literal.datatype().as_str().into(),
            language: literal.language().map(str::to_owned),
        }),
        _ => bail!("Unsupported RDF term in SHACL report: {value}"),
    }
}
fn path(value: &SHACLPath) -> RdfPath {
    match value {
        SHACLPath::Predicate { pred } => RdfPath::Predicate {
            iri: pred.as_str().into(),
        },
        SHACLPath::Sequence { paths } => RdfPath::Sequence {
            paths: paths.iter().map(path).collect(),
        },
        SHACLPath::Alternative { paths } => RdfPath::Alternative {
            paths: paths.iter().map(path).collect(),
        },
        SHACLPath::Inverse { path: inner } => RdfPath::Inverse {
            path: Box::new(path(inner)),
        },
        SHACLPath::ZeroOrMore { path: inner } => RdfPath::ZeroOrMore {
            path: Box::new(path(inner)),
        },
        SHACLPath::OneOrMore { path: inner } => RdfPath::OneOrMore {
            path: Box::new(path(inner)),
        },
        SHACLPath::ZeroOrOne { path: inner } => RdfPath::ZeroOrOne {
            path: Box::new(path(inner)),
        },
    }
}
pub fn normalize(result: &ValidationResult) -> Result<ShaclResult> {
    let mut messages: Vec<_> = result
        .message()
        .iter()
        .map(|(language, value)| ShaclMessage {
            value: value.clone(),
            language: language.as_ref().map(ToString::to_string),
        })
        .collect();
    messages.sort();
    messages.dedup();
    let result_severity = match result.severity() {
        Severity::Generic(iri) => iri.as_str().into(),
        other => format!("{SH}{other}"),
    };
    Ok(ShaclResult {
        focus_node: term(result.focus_node())?,
        constraint_component: match result.constraint_component() {
            Object::Iri(iri) => iri.as_str().into(),
            _ => bail!("SHACL constraint component must be an IRI"),
        },
        result_severity,
        source_shape: result.source().map(term).transpose()?,
        result_path: result.path().map(path),
        value: result.value().map(term).transpose()?,
        messages,
        source_field: None,
    })
}

pub fn locate(
    doc: &Document,
    result: &ShaclResult,
    relations: &[Relation],
    targets: &BTreeMap<String, String>,
) -> (Span, Option<String>) {
    let fallback = || (doc.metadata_span(), None);
    let Some(RdfPath::Predicate { iri }) = &result.result_path else {
        return fallback();
    };
    if iri == "urn:mdx:v1:body" {
        return (
            Span::new(&doc.source, doc.body_start, doc.source.len()),
            None,
        );
    }
    if iri == "urn:mdx:v1:link" {
        if let Some(RdfTerm::Iri { value }) = &result.value {
            let matches: Vec<_> = relations
                .iter()
                .filter(|r| {
                    r.source == doc.path
                        && r.kind == "link"
                        && r.target.as_ref() == targets.get(value)
                })
                .collect();
            if matches.len() == 1 {
                return (matches[0].span.clone(), None);
            }
        }
        return fallback();
    }
    let Some(encoded) = iri.strip_prefix(FIELDS) else {
        return fallback();
    };
    let Some(segments) = encoded
        .split('.')
        .map(|s| {
            percent_encoding::percent_decode_str(s)
                .decode_utf8()
                .ok()
                .map(|s| s.into_owned())
        })
        .collect::<Option<Vec<_>>>()
    else {
        return fallback();
    };
    let pointer = format!(
        "/{}",
        segments
            .iter()
            .map(|s| pointer_segment(s))
            .collect::<Vec<_>>()
            .join("/")
    );
    let Some(span) = doc.metadata_spans.get(&pointer) else {
        return fallback();
    };
    if let (Some(Value::Array(items)), Some(value)) =
        (doc.metadata.pointer(&pointer), &result.value)
    {
        let field = format!("frontMatter.{}", segments.join("."));
        let matches: Vec<_> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if let RdfTerm::Iri { value } = value {
                    relations.iter().any(|r| {
                        r.source == doc.path
                            && r.kind == field
                            && item.as_str() == Some(&r.reference)
                            && r.target.as_ref() == targets.get(value)
                    })
                } else {
                    literal(item).as_ref() == Some(value)
                }
            })
            .collect();
        if let [(index, _)] = matches.as_slice() {
            let item_pointer = format!("{pointer}/{index}");
            if let Some(span) = doc.metadata_spans.get(&item_pointer) {
                return (span.clone(), Some(item_pointer));
            }
        }
    }
    (span.clone(), Some(pointer))
}
fn literal(value: &Value) -> Option<RdfTerm> {
    let (value, datatype) = match value {
        Value::String(text) => (text.clone(), "string"),
        Value::Bool(value) => (value.to_string(), "boolean"),
        Value::Number(number) => (
            number.to_string(),
            if number.is_i64() || number.is_u64() {
                "integer"
            } else {
                "double"
            },
        ),
        _ => return None,
    };
    Some(RdfTerm::Literal {
        value,
        datatype: format!("{XSD}{datatype}"),
        language: None,
    })
}
