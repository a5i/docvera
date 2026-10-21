use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}
impl Span {
    pub fn new(source: &str, start: usize, end: usize) -> Self {
        let position = |offset| {
            let before = &source[..offset];
            (
                before.bytes().filter(|b| *b == b'\n').count() + 1,
                before.rsplit('\n').next().unwrap_or("").chars().count() + 1,
            )
        };
        let (line, column) = position(start);
        let (end_line, end_column) = position(end);
        Self {
            start,
            end,
            line,
            column,
            end_line,
            end_column,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub path: String,
    pub span: Span,
    pub rule: String,
    pub severity: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shacl: Option<ShaclResult>,
}

/// Adapter-owned representation: no Rudof structs in the public contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RdfTerm {
    Iri {
        value: String,
    },
    /// Anonymous labels are deliberately omitted: engine IDs are not stable.
    BlankNode,
    Literal {
        value: String,
        datatype: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RdfPath {
    Predicate { iri: String },
    Sequence { paths: Vec<RdfPath> },
    Alternative { paths: Vec<RdfPath> },
    Inverse { path: Box<RdfPath> },
    ZeroOrMore { path: Box<RdfPath> },
    OneOrMore { path: Box<RdfPath> },
    ZeroOrOne { path: Box<RdfPath> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct ShaclMessage {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct ShaclResult {
    pub focus_node: RdfTerm,
    pub constraint_component: String,
    pub result_severity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_shape: Option<RdfTerm>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_path: Option<RdfPath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<RdfTerm>,
    pub messages: Vec<ShaclMessage>,
    /// RFC 6901 JSON pointer into front matter, only when precisely located.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub heading: String,
    pub level: u8,
    pub anchor: String,
    pub parent: Option<usize>,
    pub span: Span,
    pub content_start: usize,
    pub direct_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub kind: String,
    pub language: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub target: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub path: String,
    pub declared_id: Option<String>,
    pub document_type: Option<String>,
    pub metadata: Value,
    pub source: String,
    pub hash: String,
    pub body_start: usize,
    pub sections: Vec<Section>,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    /// RFC 6901 pointers into metadata; derived parser provenance, not public IR.
    #[serde(skip)]
    pub metadata_spans: std::collections::BTreeMap<String, Span>,
    pub diagnostics: Vec<Diagnostic>,
}
impl Document {
    pub fn diagnostic(&mut self, span: Span, rule: &str, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            path: self.path.clone(),
            span,
            rule: rule.into(),
            severity: "error".into(),
            message: message.into(),
            shacl: None,
        });
    }
    pub fn whole_span(&self) -> Span {
        Span::new(&self.source, 0, self.source.len())
    }
    pub fn field_item_span(&self, field: &str, index: usize) -> Span {
        let Some(name) = field.strip_prefix("frontMatter.") else {
            return self.metadata_span();
        };
        let mut pointer = format!(
            "/{}",
            name.split('.')
                .map(crate::source_map::pointer_segment)
                .collect::<Vec<_>>()
                .join("/")
        );
        if self
            .metadata
            .pointer(&pointer)
            .is_some_and(serde_json::Value::is_array)
        {
            pointer.push_str(&format!("/{index}"));
        }
        self.metadata_spans
            .get(&pointer)
            .cloned()
            .unwrap_or_else(|| self.field_span(field))
    }
    pub fn field_span(&self, field: &str) -> Span {
        let pointer = field.strip_prefix("frontMatter.").map(|name| {
            format!(
                "/{}",
                name.split('.')
                    .map(crate::source_map::pointer_segment)
                    .collect::<Vec<_>>()
                    .join("/")
            )
        });
        pointer
            .and_then(|p| self.metadata_spans.get(&p))
            .cloned()
            .unwrap_or_else(|| self.metadata_span())
    }
    pub fn metadata_span(&self) -> Span {
        Span::new(&self.source, 0, self.body_start)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub source: String,
    pub target: Option<String>,
    pub reference: String,
    pub kind: String,
    pub status: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub contract_version: u32,
    pub documents: usize,
    pub diagnostics: Vec<Diagnostic>,
}
impl CheckResult {
    pub fn valid(&self) -> bool {
        !self.diagnostics.iter().any(|d| d.severity == "error")
    }
}
