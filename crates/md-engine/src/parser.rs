use crate::model::*;
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
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

/// TOML dates become ISO strings, not serde's private datetime marker objects.
/// Non-finite floats have no JSON equivalent and must not silently become null.
fn toml_json(value: toml::Value) -> Result<Value, String> {
    Ok(match value {
        toml::Value::String(v) => Value::String(v),
        toml::Value::Integer(v) => Value::Number(v.into()),
        toml::Value::Float(v) => Value::Number(
            serde_json::Number::from_f64(v)
                .ok_or("Non-finite TOML floats cannot be represented as JSON metadata")?,
        ),
        toml::Value::Boolean(v) => Value::Bool(v),
        toml::Value::Datetime(v) => Value::String(v.to_string()),
        toml::Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(toml_json)
                .collect::<Result<_, _>>()?,
        ),
        toml::Value::Table(values) => Value::Object(
            values
                .into_iter()
                .map(|(k, v)| Ok((k, toml_json(v)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}

pub fn parse(path: String, source: String) -> Document {
    parse_with_options(path, source, &Default::default(), &Default::default())
}

pub fn parse_with_options(
    path: String,
    source: String,
    options: &crate::config::ParserOptions,
    front_matter: &crate::config::FrontMatterOptions,
) -> Document {
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
        metadata_spans: Default::default(),
        diagnostics: vec![],
    };
    // Keep the BOM in source (and the hash); only skip it for syntax recognition.
    let bom = if doc.source.starts_with('\u{feff}') {
        3
    } else {
        0
    };
    doc.body_start = bom;
    let mut lines = doc.source[bom..].split_inclusive('\n');
    let opening = lines.next().map(|l| l.trim_end_matches(['\r', '\n']));
    let format = match opening {
        Some("---") if front_matter.yaml => Some("yaml"),
        Some("+++") if front_matter.toml => Some("toml"),
        _ => None,
    };
    if let Some(format) = format {
        let start = doc.source[bom..]
            .find('\n')
            .map_or(doc.source.len(), |p| bom + p + 1);
        let mut offset = start;
        let mut close = None;
        for line in lines {
            let delimiter = line.trim_end_matches(['\r', '\n']);
            if (format == "yaml" && matches!(delimiter, "---" | "..."))
                || (format == "toml" && delimiter == "+++")
            {
                close = Some((offset, offset + line.len()));
                break;
            }
            offset += line.len();
        }
        if let Some((end, body)) = close {
            doc.body_start = body;
            let parsed: Result<Value, String> = if format == "yaml" {
                serde_saphyr::from_str(&doc.source[start..end]).map_err(|e| e.to_string())
            } else {
                toml::from_str::<toml::Value>(&doc.source[start..end])
                    .map_err(|e| e.to_string())
                    .and_then(toml_json)
            };
            match parsed {
                Ok(value) if value.is_object() => {
                    doc.metadata = value;
                    doc.metadata_spans = if format == "yaml" {
                        crate::source_map::front_matter(&doc.source, start, end)
                    } else {
                        crate::toml_source_map::front_matter(&doc.source, start, end)
                    };
                }
                Ok(_) => doc.diagnostic(
                    Span::new(&doc.source, start, end),
                    &format!("{format}.object"),
                    "Front matter must be a mapping",
                ),
                Err(error) => doc.diagnostic(
                    Span::new(&doc.source, start, end),
                    &format!("{format}.parse"),
                    error,
                ),
            }
        } else {
            // An enabled opening delimiter is an attempted metadata block, including
            // while typing: report it rather than reinterpret it as Markdown.
            doc.diagnostic(
                doc.whole_span(),
                &format!("{format}.unclosed"),
                format!("Unclosed {format} front matter"),
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
    for (event, range) in
        Parser::new_ext(&doc.source[body_start..], options.options()).into_offset_iter()
    {
        let start = range.start + body_start;
        let end = range.end + body_start;
        let span = Span::new(&doc.source, start, end);
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some((start, level as u8, String::new()))
            }
            Event::Text(text) | Event::Code(text) | Event::InlineMath(text) => {
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
