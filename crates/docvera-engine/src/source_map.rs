//! YAML parser locations, never substring/indentation heuristics.
use crate::model::Span;
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, SeqAccess, Visitor},
};
use serde_saphyr::Spanned;
use std::{collections::BTreeMap, fmt};

enum Node {
    Scalar,
    Sequence(Vec<Spanned<Node>>),
    Mapping(Vec<(Spanned<String>, Spanned<Node>)>),
}
impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Nodes;
        impl<'de> Visitor<'de> for Nodes {
            type Value = Node;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a YAML value")
            }
            fn visit_bool<E>(self, _: bool) -> Result<Node, E> {
                Ok(Node::Scalar)
            }
            fn visit_i64<E>(self, _: i64) -> Result<Node, E> {
                Ok(Node::Scalar)
            }
            fn visit_u64<E>(self, _: u64) -> Result<Node, E> {
                Ok(Node::Scalar)
            }
            fn visit_f64<E>(self, _: f64) -> Result<Node, E> {
                Ok(Node::Scalar)
            }
            fn visit_str<E>(self, _: &str) -> Result<Node, E> {
                Ok(Node::Scalar)
            }
            fn visit_string<E>(self, _: String) -> Result<Node, E> {
                Ok(Node::Scalar)
            }
            fn visit_unit<E>(self) -> Result<Node, E> {
                Ok(Node::Scalar)
            }
            fn visit_none<E>(self) -> Result<Node, E> {
                Ok(Node::Scalar)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
                let mut items = vec![];
                while let Some(value) = seq.next_element()? {
                    items.push(value);
                }
                Ok(Node::Sequence(items))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
                let mut fields = vec![];
                while let Some(pair) = map.next_entry()? {
                    fields.push(pair);
                }
                Ok(Node::Mapping(fields))
            }
        }
        deserializer.deserialize_any(Nodes)
    }
}

pub fn pointer_segment(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

pub fn front_matter(source: &str, start: usize, end: usize) -> BTreeMap<String, Span> {
    let mut result = BTreeMap::new();
    // Failure to retain locations must not change YAML semantics. Existing schema
    // validation remains authoritative; an unavailable location uses document fallback.
    if let Ok(node) = serde_saphyr::from_str::<Spanned<Node>>(&source[start..end]) {
        collect(&node, "", source, start, end, &mut result);
    }
    result
}
fn location(
    node: &serde_saphyr::Location,
    source: &str,
    offset: usize,
    end: usize,
) -> Option<Span> {
    let span = node.span();
    let start = offset.checked_add(usize::try_from(span.byte_offset()?).ok()?)?;
    let stop = start.checked_add(usize::try_from(span.byte_len()?).ok()?)?;
    (start < stop && stop <= end && source.is_char_boundary(start) && source.is_char_boundary(stop))
        .then(|| Span::new(source, start, stop))
}
fn collect(
    node: &Spanned<Node>,
    pointer: &str,
    source: &str,
    start: usize,
    end: usize,
    out: &mut BTreeMap<String, Span>,
) {
    if let Some(span) = location(&node.referenced, source, start, end) {
        out.insert(pointer.into(), span);
    }
    // Alias/merge replay can point children at definitions rather than the use
    // site. Retain the reference token but do not invent child locations.
    if node.referenced != node.defined {
        return;
    }
    match &node.value {
        Node::Scalar => {}
        Node::Sequence(items) => {
            for (i, item) in items.iter().enumerate() {
                collect(item, &format!("{pointer}/{i}"), source, start, end, out);
            }
        }
        Node::Mapping(fields) => {
            for (key, value) in fields {
                let pointer = format!("{pointer}/{}", pointer_segment(&key.value));
                collect(value, &pointer, source, start, end, out);
                // Collection locations from Spanned are opening tokens, not full
                // ranges. Highlight the exact key rather than claiming '[' is the
                // whole value. Aliases retain their own use-site token.
                let collection = matches!(value.value, Node::Mapping(_) | Node::Sequence(_))
                    && value.referenced == value.defined;
                if (collection || !out.contains_key(&pointer))
                    && let Some(span) = location(&key.referenced, source, start, end)
                {
                    out.insert(pointer, span);
                }
            }
        }
    }
}
