use crate::model::*;
use anyhow::{Result, bail};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

fn yes() -> bool {
    true
}
fn one() -> usize {
    1
}
fn default_scope() -> String {
    "documentType".into()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Schema {
    pub schema_version: u32,
    pub document_type: String,
    #[serde(default)]
    pub front_matter: Value,
    #[serde(default)]
    pub identity: Option<Identity>,
    #[serde(default)]
    pub body: Body,
    #[serde(default)]
    pub relations: Vec<RelationRule>,
    #[serde(default)]
    pub lint: Lint,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Identity {
    pub field: String,
    #[serde(default = "default_scope")]
    pub unique_within: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationRule {
    pub from: String,
    pub target_type: Option<String>,
    #[serde(default = "yes")]
    pub must_exist: bool,
    #[serde(default)]
    pub min: usize,
    pub max: Option<usize>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Lint {
    #[serde(default)]
    pub no_skipped_headings: bool,
    #[serde(default)]
    pub no_trailing_spaces: bool,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Body {
    #[serde(default = "yes")]
    pub ordered: bool,
    #[serde(default)]
    pub additional_sections: bool,
    #[serde(default)]
    pub allow_preamble: bool,
    #[serde(default)]
    pub sections: Vec<SectionRule>,
}
impl Default for Body {
    fn default() -> Self {
        Self {
            ordered: true,
            additional_sections: false,
            allow_preamble: false,
            sections: vec![],
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SectionRule {
    pub title: Option<String>,
    pub pattern: Option<String>,
    pub title_equals: Option<String>,
    pub level: Option<u8>,
    #[serde(default = "one")]
    pub min: usize,
    #[serde(default = "one")]
    pub max: usize,
    #[serde(default)]
    pub non_empty: bool,
    #[serde(default)]
    pub content: Vec<ContentRule>,
    #[serde(default)]
    pub children: Body,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentRule {
    pub kind: String,
    pub language: Option<String>,
    #[serde(default = "one")]
    pub min: usize,
    pub max: Option<usize>,
}

pub fn field<'a>(metadata: &'a Value, path: &str) -> Option<&'a Value> {
    let mut value = metadata;
    for part in path.strip_prefix("frontMatter.")?.split('.') {
        value = value.get(part)?;
    }
    Some(value)
}
impl Schema {
    pub fn check_definition(&self) -> Result<()> {
        if self.schema_version != 1 {
            bail!("Unsupported schemaVersion: {}", self.schema_version);
        }
        if self.document_type.is_empty() {
            bail!("documentType must not be empty");
        }
        if !self.front_matter.is_null() {
            jsonschema::validator_for(&self.front_matter)?;
        }
        if let Some(id) = &self.identity {
            if !id.field.starts_with("frontMatter.") {
                bail!("identity.field must start with frontMatter.");
            }
            if !["workspace", "documentType", "directory"].contains(&id.unique_within.as_str()) {
                bail!("Invalid uniqueWithin");
            }
        }
        for relation in &self.relations {
            if !relation.from.starts_with("frontMatter.") {
                bail!("Only metadata relation sources are supported");
            }
            if relation.max.is_some_and(|max| max < relation.min) {
                bail!("Relation max < min");
            }
        }
        fn body(body: &Body) -> Result<()> {
            for rule in &body.sections {
                if [
                    rule.title.is_some(),
                    rule.pattern.is_some(),
                    rule.title_equals.is_some(),
                ]
                .into_iter()
                .filter(|b| *b)
                .count()
                    != 1
                {
                    bail!("Each section requires exactly one of title, pattern, titleEquals");
                }
                if let Some(p) = &rule.pattern {
                    Regex::new(p)?;
                }
                if rule.level.is_some_and(|l| !(1..=6).contains(&l)) || rule.max < rule.min {
                    bail!("Invalid section level or cardinality");
                }
                if rule
                    .title_equals
                    .as_ref()
                    .is_some_and(|p| !p.starts_with("frontMatter."))
                {
                    bail!("titleEquals must reference frontMatter");
                }
                for content in &rule.content {
                    if !["paragraph", "code", "list", "table", "blockquote"]
                        .contains(&content.kind.as_str())
                    {
                        bail!("Unsupported content kind: {}", content.kind);
                    }
                    if content.language.is_some() && content.kind != "code" {
                        bail!("language is only valid for code");
                    }
                    if content.max.is_some_and(|max| max < content.min) {
                        bail!("Content max < min");
                    }
                }
                body_check_children(&rule.children)?;
            }
            Ok(())
        }
        fn body_check_children(value: &Body) -> Result<()> {
            body(value)
        }
        body(&self.body)
    }
}

pub fn validate(doc: &mut Document, schema: &Schema) -> Result<()> {
    doc.document_type = Some(schema.document_type.clone());
    // The selector belongs to the engine, not to user metadata validation.
    let mut metadata = doc.metadata.clone();
    if let Some(object) = metadata.as_object_mut() {
        object.remove("$schema");
    }
    if !schema.front_matter.is_null() {
        let validator = jsonschema::validator_for(&schema.front_matter)?;
        for error in validator.iter_errors(&metadata) {
            doc.diagnostic(doc.metadata_span(), "schema.frontMatter", error.to_string());
        }
    }
    if let Some(identity) = &schema.identity {
        doc.declared_id = field(&doc.metadata, &identity.field)
            .and_then(Value::as_str)
            .map(str::to_owned);
        if doc.declared_id.as_ref().is_none_or(|id| id.is_empty()) {
            doc.diagnostic(
                doc.metadata_span(),
                "identity.required",
                "Identity must be a non-empty string",
            );
        }
    }
    validate_body(doc, &schema.body, None);
    if schema.lint.no_skipped_headings {
        let mut previous = 0;
        for section in doc.sections.clone() {
            if section.level > previous + 1 {
                doc.diagnostic(
                    section.span,
                    "lint.heading-level",
                    "Heading level is skipped",
                );
            }
            previous = section.level;
        }
    }
    if schema.lint.no_trailing_spaces {
        let mut offset = 0;
        for line in doc.source.clone().split_inclusive('\n') {
            let text = line.trim_end_matches(['\r', '\n']);
            if text.ends_with([' ', '\t']) {
                doc.diagnostic(
                    Span::new(&doc.source, offset, offset + text.len()),
                    "lint.trailing-spaces",
                    "Trailing whitespace",
                );
            }
            offset += line.len();
        }
    }
    // Lint findings are warnings and never schema failures.
    for d in &mut doc.diagnostics {
        if d.rule.starts_with("lint.") {
            d.severity = "warning".into();
        }
    }
    Ok(())
}

fn validate_body(doc: &mut Document, body: &Body, parent: Option<usize>) {
    let children: Vec<usize> = doc
        .sections
        .iter()
        .enumerate()
        .filter(|(_, s)| s.parent == parent)
        .map(|(i, _)| i)
        .collect();
    let (start, end) = parent.map_or(
        (
            doc.body_start,
            doc.sections
                .first()
                .map_or(doc.source.len(), |s| s.span.start),
        ),
        |p| (doc.sections[p].content_start, doc.sections[p].direct_end),
    );
    if parent.is_none() && !body.allow_preamble && !doc.source[start..end].trim().is_empty() {
        doc.diagnostic(
            Span::new(&doc.source, start, end),
            "body.preamble",
            "Text before the first heading is not allowed",
        );
    }
    let mut counts = vec![0; body.sections.len()];
    let mut previous = None;
    for index in children {
        let section = doc.sections[index].clone();
        let matches: Vec<usize> = body
            .sections
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.title.as_ref().is_some_and(|t| *t == section.heading)
                    || r.pattern
                        .as_ref()
                        .is_some_and(|p| Regex::new(p).is_ok_and(|r| r.is_match(&section.heading)))
                    || r.title_equals.as_ref().is_some_and(|p| {
                        field(&doc.metadata, p).and_then(Value::as_str)
                            == Some(section.heading.as_str())
                    })
            })
            .map(|(i, _)| i)
            .collect();
        if matches.len() > 1 {
            doc.diagnostic(
                section.span.clone(),
                "body.ambiguous",
                "Heading matches multiple section rules",
            );
        }
        let Some(&rule_index) = matches.first() else {
            if !body.additional_sections {
                doc.diagnostic(
                    section.span,
                    "body.additional",
                    format!("Unexpected section: {}", section.heading),
                );
            }
            continue;
        };
        let rule = &body.sections[rule_index];
        counts[rule_index] += 1;
        if body.ordered && previous.is_some_and(|p| rule_index < p) {
            doc.diagnostic(
                section.span.clone(),
                "body.order",
                format!("Section out of order: {}", section.heading),
            );
        }
        previous = Some(previous.map_or(rule_index, |p: usize| p.max(rule_index)));
        let expected_level = rule
            .level
            .unwrap_or_else(|| parent.map_or(1, |p| doc.sections[p].level + 1));
        if section.level != expected_level {
            doc.diagnostic(
                section.span.clone(),
                "body.level",
                format!("Expected H{expected_level}, found H{}", section.level),
            );
        }
        if rule.non_empty
            && doc.source[section.content_start..section.direct_end]
                .trim()
                .is_empty()
        {
            doc.diagnostic(
                section.span.clone(),
                "body.empty",
                "Section needs direct content, not only child headings",
            );
        }
        for content in &rule.content {
            let count = doc
                .nodes
                .iter()
                .filter(|n| {
                    n.span.start >= section.content_start
                        && n.span.start < section.direct_end
                        && n.kind == content.kind
                        && content
                            .language
                            .as_ref()
                            .is_none_or(|l| n.language.as_ref() == Some(l))
                })
                .count();
            if count < content.min || content.max.is_some_and(|max| count > max) {
                doc.diagnostic(
                    section.span.clone(),
                    "body.content",
                    format!(
                        "{}: expected {}..{:?} {}, found {count}",
                        section.heading, content.min, content.max, content.kind
                    ),
                );
            }
        }
        validate_body(doc, &rule.children, Some(index));
    }
    for (rule, count) in body.sections.iter().zip(counts) {
        if count < rule.min || count > rule.max {
            let span = parent.map_or_else(|| doc.whole_span(), |p| doc.sections[p].span.clone());
            doc.diagnostic(
                span,
                "body.cardinality",
                format!(
                    "Section {:?}: expected {}..{}, found {count}",
                    rule.title
                        .as_ref()
                        .or(rule.pattern.as_ref())
                        .or(rule.title_equals.as_ref()),
                    rule.min,
                    rule.max
                ),
            );
        }
    }
}
