use crate::model::*;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde_json::{Value, json};
use std::collections::HashSet;

/// Deliberately documented slug policy, including Unicode and collision suffixes.
pub fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                Some(c)
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

pub fn parse(path: String, source: String) -> Document {
    let mut doc = Document {
        path,
        hash: blake3::hash(source.as_bytes()).to_hex().to_string(),
        source,
        declared_id: None,
        document_type: None,
        metadata: json!({}),
        body_start: 0,
        sections: vec![],
        nodes: vec![],
        links: vec![],
        diagnostics: vec![],
    };
    let mut lines = doc.source.split_inclusive('\n');
    if lines.next().is_some_and(|l| l.trim_end() == "---") {
        let start = doc.source.find('\n').map_or(doc.source.len(), |p| p + 1);
        let mut offset = start;
        let mut close = None;
        for line in lines {
            if line.trim_end() == "---" {
                close = Some((offset, offset + line.len()));
                break;
            }
            offset += line.len();
        }
        if let Some((end, body)) = close {
            doc.body_start = body;
            match serde_saphyr::from_str::<Value>(&doc.source[start..end]) {
                Ok(value) if value.is_object() => doc.metadata = value,
                Ok(_) => doc.diagnostic(
                    Span::new(&doc.source, start, end),
                    "yaml.object",
                    "Front matter must be a mapping",
                ),
                Err(e) => doc.diagnostic(
                    Span::new(&doc.source, start, end),
                    "yaml.parse",
                    e.to_string(),
                ),
            }
        } else {
            doc.diagnostic(
                doc.whole_span(),
                "yaml.unclosed",
                "Unclosed YAML front matter",
            );
            doc.body_start = doc.source.len();
        }
    }
    doc.declared_id = doc
        .metadata
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut heading: Option<(usize, u8, String)> = None;
    let mut used_anchors = HashSet::new();
    let body_start = doc.body_start;
    for (event, range) in Parser::new_ext(
        &doc.source[body_start..],
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    )
    .into_offset_iter()
    {
        let start = range.start + body_start;
        let end = range.end + body_start;
        let span = Span::new(&doc.source, start, end);
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some((start, level as u8, String::new()))
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, _, value)) = &mut heading {
                    value.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, _, value)) = &mut heading {
                    value.push(' ');
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((heading_start, level, text)) = heading.take() {
                    let base = slug(&text);
                    let mut anchor = base.clone();
                    let mut suffix = 0;
                    while !used_anchors.insert(anchor.clone()) {
                        suffix += 1;
                        anchor = format!("{base}-{suffix}");
                    }
                    let parent = doc.sections.iter().rposition(|s| s.level < level);
                    doc.sections.push(Section {
                        id: format!("{}#{}", doc.path, anchor),
                        heading: text,
                        level,
                        anchor,
                        parent,
                        span: Span::new(&doc.source, heading_start, doc.source.len()),
                        content_start: end,
                        direct_end: doc.source.len(),
                    });
                }
            }
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                doc.links.push(Link {
                    target: dest_url.to_string(),
                    span,
                })
            }
            Event::Start(tag) => {
                let (kind, language) = match tag {
                    Tag::Paragraph => ("paragraph", None),
                    Tag::CodeBlock(kind) => (
                        "code",
                        match kind {
                            CodeBlockKind::Fenced(info) => {
                                info.split_whitespace().next().map(str::to_owned)
                            }
                            _ => None,
                        },
                    ),
                    Tag::List(_) => ("list", None),
                    Tag::Table(_) => ("table", None),
                    Tag::BlockQuote(_) => ("blockquote", None),
                    _ => continue,
                };
                doc.nodes.push(Node {
                    kind: kind.into(),
                    language,
                    span,
                });
            }
            _ => {}
        }
    }
    for i in 0..doc.sections.len() {
        let next = doc
            .sections
            .get(i + 1)
            .map_or(doc.source.len(), |s| s.span.start);
        let end = doc.sections[i + 1..]
            .iter()
            .find(|s| s.level <= doc.sections[i].level)
            .map_or(doc.source.len(), |s| s.span.start);
        doc.sections[i].direct_end = next;
        doc.sections[i].span = Span::new(&doc.source, doc.sections[i].span.start, end);
    }
    doc
}
