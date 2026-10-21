//! TOML parser-owned ranges. Use the raw tree so datetime's private serde
//! representation cannot break provenance for otherwise valid metadata.
use crate::model::Span;
use std::collections::BTreeMap;
use toml::{
    Spanned,
    de::{DeTable, DeValue},
};

pub fn front_matter(source: &str, start: usize, end: usize) -> BTreeMap<String, Span> {
    let mut out = BTreeMap::new();
    if let Ok(table) = DeTable::parse(&source[start..end]) {
        collect_table(table.get_ref(), "", source, start, end, &mut out);
    }
    out
}

fn range<T>(node: &Spanned<T>, source: &str, offset: usize, end: usize) -> Option<Span> {
    let range = node.span();
    let start = offset.checked_add(range.start)?;
    let stop = offset.checked_add(range.end)?;
    (start < stop && stop <= end && source.is_char_boundary(start) && source.is_char_boundary(stop))
        .then(|| Span::new(source, start, stop))
}
fn collect_table(
    table: &DeTable<'_>,
    pointer: &str,
    source: &str,
    offset: usize,
    end: usize,
    out: &mut BTreeMap<String, Span>,
) {
    for (key, value) in table.iter() {
        let pointer = format!(
            "{pointer}/{}",
            crate::source_map::pointer_segment(key.get_ref())
        );
        collect(value, &pointer, source, offset, end, out);
        // Collection ranges can span disjoint table declarations; use the exact key.
        if (value.get_ref().is_table() || value.get_ref().is_array())
            && let Some(span) = range(key, source, offset, end)
        {
            out.insert(pointer, span);
        }
    }
}
fn collect(
    node: &Spanned<DeValue<'_>>,
    pointer: &str,
    source: &str,
    offset: usize,
    end: usize,
    out: &mut BTreeMap<String, Span>,
) {
    if let Some(span) = range(node, source, offset, end) {
        out.insert(pointer.into(), span);
    }
    if let Some(items) = node.get_ref().as_array() {
        for (i, item) in items.iter().enumerate() {
            collect(item, &format!("{pointer}/{i}"), source, offset, end, out);
        }
    } else if let Some(table) = node.get_ref().as_table() {
        collect_table(table, pointer, source, offset, end, out);
    }
}
