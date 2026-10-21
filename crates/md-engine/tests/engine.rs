use md_engine::{
    Workspace,
    index::{self, SearchOptions},
    parser,
    schema::Schema,
};
use serde_json::{Value, json};
use std::{fs, path::Path};
use tempfile::TempDir;

fn fixture() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    md_engine::init(dir.path()).unwrap();
    dir
}
fn edit(root: &Path, file: &str, from: &str, to: &str) {
    let path = root.join(file);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "Missing edit fragment {from}");
    fs::write(path, text.replace(from, to)).unwrap();
}
fn rules(root: &Path) -> Vec<String> {
    Workspace::load(root)
        .unwrap()
        .check()
        .diagnostics
        .into_iter()
        .map(|d| d.rule)
        .collect()
}
fn has(root: &Path, rule: &str) {
    assert!(rules(root).contains(&rule.to_owned()), "Expected {rule}");
}
fn search(root: &Path, text: &str) -> Value {
    index::search(
        root,
        &SearchOptions {
            text: text.into(),
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn full_lifecycle_query_graph_search_and_extraction() {
    let dir = fixture();
    let root = dir.path();
    let ws = Workspace::load(root).unwrap();
    assert!(ws.check().valid());
    assert_eq!(ws.documents.len(), 3);
    assert_eq!(index::index(&ws, false).unwrap().updated, 3);
    assert_eq!(index::index(&ws, false).unwrap().updated, 0);
    let rows = index::query(
        root,
        "SELECT declared_id,metadata->>'status' AS status FROM documents WHERE type='adr'",
    )
    .unwrap();
    assert_eq!(rows["rows"][0]["status"], "accepted");
    let graph = index::graph(root, "FEATURE-0042", "incoming", 2).unwrap();
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 3);
    let hits = index::search(
        root,
        &SearchOptions {
            text: "OAuth".into(),
            document_type: Some("feature".into()),
            section: Some("Security".into()),
            limit: None,
        },
    )
    .unwrap();
    assert_eq!(hits["results"].as_array().unwrap().len(), 1);
    let hit = &hits["results"][0];
    assert!(hit["score"].as_f64().unwrap() > 0.0);
    assert_eq!(hit["location"]["line"], 22);
    let extracted = index::extract(root, hit["sectionId"].as_str().unwrap()).unwrap();
    assert!(
        extracted["markdown"]
            .as_str()
            .unwrap()
            .starts_with("## Security")
    );
    assert!(extracted["markdown"].as_str().unwrap().contains("PKCE"));

    edit(root, "docs/feature-0042.md", "OAuth", "OpenID");
    let ws = Workspace::load(root).unwrap();
    assert_eq!(index::index(&ws, false).unwrap().updated, 1);
    assert_eq!(
        search(root, "OpenID")["results"].as_array().unwrap().len(),
        1
    );
    assert!(
        search(root, "OAuth")["results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h["documentId"] != "FEATURE-0042")
    );
    fs::remove_file(root.join("docs/feature-0042.md")).unwrap();
    let ws = Workspace::load(root).unwrap();
    assert!(!ws.check().valid());
    assert_eq!(index::index(&ws, false).unwrap().deleted, 1);
    assert!(
        search(root, "OpenID")["results"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        index::query(root, "SELECT count(*) AS n FROM documents").unwrap()["rows"][0]["n"],
        2
    );
    assert!(ws.relations.iter().any(|r| r.status == "missing"));
}

#[test]
fn optional_nested_section_can_be_removed_but_required_content_is_checked_when_present() {
    let dir = fixture();
    edit(dir.path(), "docs/adr-0001.md", "```bash", "```rust");
    has(dir.path(), "body.content");
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "### Implementation\n\n```rust\nmdx index --rebuild\n```\n\n",
        "",
    );
    assert!(Workspace::load(dir.path()).unwrap().check().valid());
}

#[test]
fn order_cardinality_empty_and_additional_sections() {
    for (from, to, rule) in [
        ("## Context", "## Surprise", "body.additional"),
        (
            "Documentation must remain available offline.",
            "",
            "body.empty",
        ),
        ("## Context", "### Context", "body.level"),
        ("## Decision", "## Context", "body.cardinality"),
    ] {
        let dir = fixture();
        edit(dir.path(), "docs/adr-0001.md", from, to);
        has(dir.path(), rule);
    }
    let dir = fixture();
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "## Context",
        "## Consequences",
    );
    edit(dir.path(), "docs/adr-0001.md", "## Decision", "## Context");
    has(dir.path(), "body.order");
}

#[test]
fn repeated_pattern_sections_and_unordered_children() {
    let dir = fixture();
    assert!(Workspace::load(dir.path()).unwrap().check().valid());
    edit(dir.path(), "docs/test-0001.md", "## Scope", "## TEMP");
    edit(dir.path(), "docs/test-0001.md", "## Cases", "## Scope");
    edit(dir.path(), "docs/test-0001.md", "## TEMP", "## Cases");
    // Cases now needs a list; supply it without changing order.
    edit(
        dir.path(),
        "docs/test-0001.md",
        "Verify [the feature](id:FEATURE-0042) offline.",
        "- Verify [the feature](id:FEATURE-0042) offline.",
    );
    assert!(Workspace::load(dir.path()).unwrap().check().valid());
}

#[test]
fn metadata_schema_selector_reserved_and_internal_refs_work() {
    let dir = fixture();
    edit(
        dir.path(),
        "schemas/adr.yaml",
        "title: { type: string, minLength: 5 }",
        "title: { $ref: '#/$defs/title' }",
    );
    edit(
        dir.path(),
        "schemas/adr.yaml",
        "  type: object",
        "  $defs:\n    title: { type: string, minLength: 5 }\n  type: object",
    );
    assert!(Workspace::load(dir.path()).unwrap().check().valid());
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "status: accepted",
        "status: impossible",
    );
    has(dir.path(), "schema.frontMatter");
}

#[test]
fn malformed_yaml_and_duplicate_keys_are_diagnostics() {
    let dir = fixture();
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "status: accepted",
        "status: [",
    );
    has(dir.path(), "yaml.parse");
    let dir = fixture();
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "status: accepted",
        "status: accepted\nstatus: proposed",
    );
    has(dir.path(), "yaml.parse");
    let doc = parser::parse("bad.md".into(), "---\nid: x".into());
    assert_eq!(doc.diagnostics[0].rule, "yaml.unclosed");
}

#[test]
fn duplicates_remain_indexable_and_references_become_ambiguous() {
    let dir = fixture();
    let root = dir.path();
    fs::copy(
        root.join("docs/feature-0042.md"),
        root.join("docs/feature-copy.md"),
    )
    .unwrap();
    let ws = Workspace::load(root).unwrap();
    assert_eq!(
        ws.check()
            .diagnostics
            .iter()
            .filter(|d| d.rule == "identity.duplicate")
            .count(),
        2
    );
    assert!(ws.relations.iter().any(|r| r.status == "ambiguous"));
    index::index(&ws, false).unwrap();
    assert_eq!(
        index::query(root, "SELECT count(*) AS n FROM documents").unwrap()["rows"][0]["n"],
        4
    );
    assert!(index::graph(root, "FEATURE-0042", "incoming", 1).is_err());
}

#[test]
fn broken_anchor_wrong_type_missing_path_and_relation_cardinality() {
    let dir = fixture();
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "#requirements",
        "#nonexistent",
    );
    let ws = Workspace::load(dir.path()).unwrap();
    assert!(ws.relations.iter().any(|r| r.status == "missing-anchor"));
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "related: [FEATURE-0042]",
        "related: [TEST-0001]",
    );
    let ws = Workspace::load(dir.path()).unwrap();
    assert!(ws.relations.iter().any(|r| r.status == "wrong-type"));
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "related: [TEST-0001]",
        "related: []",
    );
    has(dir.path(), "relation.cardinality");
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "feature-0042.md#nonexistent",
        "missing.md",
    );
    has(dir.path(), "link.target");
}

#[test]
fn schema_changes_invalidate_unchanged_documents() {
    let dir = fixture();
    let root = dir.path();
    index::index(&Workspace::load(root).unwrap(), false).unwrap();
    edit(
        root,
        "schemas/adr.yaml",
        "documentType: adr",
        "documentType: decision",
    );
    let ws = Workspace::load(root).unwrap();
    assert!(index::index(&ws, false).unwrap().updated > 0);
    assert_eq!(
        index::query(
            root,
            "SELECT type FROM documents WHERE declared_id='ADR-0001'"
        )
        .unwrap()["rows"][0]["type"],
        "decision"
    );
}

#[test]
fn lint_warnings_are_separate_from_schema_errors() {
    let dir = fixture();
    edit(
        dir.path(),
        "schemas/adr.yaml",
        "  noSkippedHeadings: true",
        "  noSkippedHeadings: true\n  noTrailingSpaces: true",
    );
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "Documentation must remain available offline.",
        "Documentation must remain available offline.  ",
    );
    let result = Workspace::load(dir.path()).unwrap().check();
    assert!(result.valid());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.rule == "lint.trailing-spaces" && d.severity == "warning")
    );
}

#[test]
fn sql_rejects_mutation_file_access_extensions_and_unbounded_results() {
    let dir = fixture();
    let root = dir.path();
    index::index(&Workspace::load(root).unwrap(), false).unwrap();
    for sql in [
        "DELETE FROM documents",
        "DROP TABLE documents",
        "PRAGMA user_version=99",
        "ATTACH DATABASE ':memory:' AS other",
        "SELECT load_extension('evil')",
        "SELECT readfile('/etc/passwd')",
        "SELECT * FROM sqlite_master",
        "SELECT randomblob(1000000000)",
        "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1002) SELECT x FROM n",
    ] {
        assert!(
            index::query(root, sql).is_err(),
            "Unsafe query accepted: {sql}"
        );
    }
    assert_eq!(
        index::query(root, "SELECT count(*) AS n FROM documents").unwrap()["rows"][0]["n"],
        3
    );
}

#[test]
fn cycles_terminate_and_depth_is_bounded() {
    let dir = fixture();
    edit(
        dir.path(),
        "docs/feature-0042.md",
        "Read, validate and search Markdown without a server.",
        "Read, validate and search Markdown without a server. [ADR](id:ADR-0001)",
    );
    index::index(&Workspace::load(dir.path()).unwrap(), false).unwrap();
    let graph = index::graph(dir.path(), "ADR-0001", "outgoing", 32).unwrap();
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 2);
    assert!(index::graph(dir.path(), "ADR-0001", "outgoing", 33).is_err());
    assert_eq!(
        index::graph(dir.path(), "ADR-0001", "incoming", 0).unwrap()["edges"],
        json!([])
    );
}

#[test]
fn parsing_tracks_unicode_crlf_setext_code_and_duplicate_anchors() {
    let text = "---\r\nid: X\r\n---\r\n\r\nПривет\r\n======\r\n\r\n## Same\r\n## Same\r\n## Same-1\r\n## Same\r\n```md\r\n# Not a heading\r\n```\r\n";
    let doc = parser::parse("unicode.md".into(), text.into());
    assert_eq!(doc.sections.len(), 5);
    assert_eq!(doc.sections[0].anchor, "привет");
    assert_eq!(doc.sections[0].span.line, 5);
    let anchors: Vec<_> = doc.sections.iter().map(|s| &s.anchor).collect();
    assert_eq!(
        anchors
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        5
    );
    assert_eq!(doc.sections[1].parent, Some(0));
    assert_eq!(doc.sections[1].span.end, doc.sections[2].span.start);
}

#[test]
fn unknown_schema_fields_bad_regex_and_remote_refs_fail_closed() {
    let dir = fixture();
    edit(dir.path(), "schemas/adr.yaml", "body:", "bodi:");
    has(dir.path(), "schema.resolve");
    let dir = fixture();
    edit(
        dir.path(),
        "schemas/adr.yaml",
        "title: Context",
        "pattern: '['",
    );
    has(dir.path(), "schema.resolve");
    let dir = fixture();
    edit(
        dir.path(),
        "schemas/adr.yaml",
        "title: { type: string, minLength: 5 }",
        "title: { $ref: 'https://example.invalid/schema.json' }",
    );
    has(dir.path(), "schema.resolve");
    assert!(
        serde_saphyr::from_str::<Schema>(
            "schemaVersion: 1\ndocumentType: x\nbody: { sections: [{title: x, max: 0}] }"
        )
        .unwrap()
        .check_definition()
        .is_err()
    );
}

#[test]
fn init_is_non_destructive_and_preamble_is_explicit() {
    let dir = fixture();
    let before = fs::read_to_string(dir.path().join("mdx.toml")).unwrap();
    assert!(md_engine::init(dir.path()).is_err());
    assert_eq!(
        fs::read_to_string(dir.path().join("mdx.toml")).unwrap(),
        before
    );
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "# Local-first storage",
        "Preamble\n\n# Local-first storage",
    );
    has(dir.path(), "body.preamble");
}

#[test]
fn missing_schema_and_outside_workspace_are_diagnostics() {
    let dir = fixture();
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "../schemas/adr.yaml",
        "../../../outside.yaml",
    );
    has(dir.path(), "schema.resolve");
    edit(
        dir.path(),
        "docs/feature-0042.md",
        "Read, validate and search Markdown without a server.",
        "[escape](../../outside.md)",
    );
    let ws = Workspace::load(dir.path()).unwrap();
    assert!(ws.relations.iter().any(|r| r.status == "outside-workspace"));
}

#[test]
fn explicit_id_never_falls_back_to_a_file_path() {
    let dir = fixture();
    fs::write(dir.path().join("docs/asset.txt"), "asset").unwrap();
    edit(
        dir.path(),
        "docs/adr-0001.md",
        "feature-0042.md#requirements",
        "id:asset.txt",
    );
    let ws = Workspace::load(dir.path()).unwrap();
    assert!(
        ws.relations
            .iter()
            .any(|r| r.reference == "id:asset.txt" && r.status == "missing")
    );
}

#[cfg(unix)]
#[test]
fn index_file_symlink_cannot_escape_workspace() {
    use std::os::unix::fs::symlink;
    let dir = fixture();
    let outside = fixture();
    index::index(&Workspace::load(outside.path()).unwrap(), false).unwrap();
    fs::create_dir(dir.path().join(".mdx")).unwrap();
    symlink(
        outside.path().join(".mdx/index.sqlite3"),
        dir.path().join(".mdx/index.sqlite3"),
    )
    .unwrap();
    assert!(index::index(&Workspace::load(dir.path()).unwrap(), false).is_err());
    assert!(index::query(dir.path(), "SELECT count(*) AS n FROM documents").is_err());
}

#[cfg(unix)]
#[test]
fn symlinks_cannot_escape_workspace() {
    use std::os::unix::fs::symlink;
    let dir = fixture();
    let outside = fixture();
    fs::remove_file(dir.path().join("schemas/adr.yaml")).unwrap();
    symlink(
        outside.path().join("schemas/adr.yaml"),
        dir.path().join("schemas/adr.yaml"),
    )
    .unwrap();
    has(dir.path(), "schema.resolve");
    symlink(outside.path(), dir.path().join(".mdx")).unwrap();
    assert!(index::index(&Workspace::load(dir.path()).unwrap(), false).is_err());
}

#[test]
fn non_utf8_document_error_names_the_file() {
    let dir = fixture();
    fs::write(dir.path().join("docs/binary.md"), b"\xff\xfe\x00bad").unwrap();
    let error = match Workspace::load(dir.path()) {
        Err(error) => error,
        Ok(_) => panic!("expected non-UTF-8 document to fail the load"),
    };
    assert!(
        format!("{error:#}").contains("binary.md"),
        "expected document path in error, got: {error:#}"
    );
}

#[test]
fn non_utf8_schema_error_names_the_schema_file() {
    let dir = fixture();
    fs::write(dir.path().join("schemas/adr.yaml"), b"\xff\xfe\x00bad").unwrap();
    let result = Workspace::load(dir.path()).unwrap().check();
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|d| d.rule == "schema.resolve")
        .expect("schema.resolve diagnostic");
    assert!(
        diagnostic.message.contains("adr.yaml"),
        "expected schema path in message, got: {}",
        diagnostic.message
    );
}
