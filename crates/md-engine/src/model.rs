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
        });
    }
    pub fn whole_span(&self) -> Span {
        Span::new(&self.source, 0, self.source.len())
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
