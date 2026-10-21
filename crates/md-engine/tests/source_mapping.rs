use md_engine::{
    Workspace, index,
    model::{RdfPath, RdfTerm},
    parser,
};
use std::{fs, path::Path};

fn put(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn fixture(source: &str, policy: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    put(
        dir.path(),
        "mdx.toml",
        "version = 1\ninclude = ['docs/*.md']\n[schemas]\n'docs/*.md' = 'schema.yaml'\n[validation.shacl]\nshapes = ['policy.ttl']\n",
    );
    put(
        dir.path(),
        "schema.yaml",
        "schemaVersion: 1\ndocumentType: note\nbody: {allowPreamble: true, additionalSections: true}\nrelations:\n  - {from: frontMatter.related}\n",
    );
    put(dir.path(), "docs/note.md", source);
    put(
        dir.path(),
        "policy.ttl",
        &format!(
            "@prefix sh: <http://www.w3.org/ns/shacl#> . @prefix f: <urn:mdx:v1:field:> . <urn:policy:Note> a sh:NodeShape ; sh:targetClass <urn:mdx:v1:type:note> ; {policy}"
        ),
    );
    dir
}

#[test]
fn yaml_locations_are_utf8_byte_offsets_with_crlf_and_nested_flow_values() {
    let source = "---\r\nid: N-1\r\ntitle: Привет 🦀\r\nstatus: 'planned'\r\nreview: {owner: \"Лёша\", states: [draft, planned]}\r\n'a.b': separate\r\n'a/b~c': escaped\r\n---\r\n# Note\r\n";
    let doc = parser::parse("note.md".into(), source.into());
    for (pointer, expected, line) in [
        ("/status", "'planned'", 4),
        ("/review/owner", "\"Лёша\"", 5),
        ("/review/states/1", "planned", 5),
        ("/a.b", "separate", 6),
        ("/a~1b~0c", "escaped", 7),
    ] {
        let span = doc
            .metadata_spans
            .get(pointer)
            .unwrap_or_else(|| panic!("{pointer}: {:?}", doc.metadata_spans));
        assert_eq!(&source[span.start..span.end], expected, "{pointer}");
        assert_eq!(span.line, line);
    }
}

#[test]
fn direct_path_diagnostics_include_structured_report_and_exact_value_span() {
    let source = "---\nid: N-1\ntitle: Привет\nstatus: planned\n---\n# Note\n";
    let dir = fixture(
        source,
        "sh:property <urn:policy:Status> . <urn:policy:Status> a sh:PropertyShape ; sh:path f:status ; sh:in (\"draft\") ; sh:message \"Неверный статус\"@ru, \"Wrong status\"@en .",
    );
    let result = Workspace::load(dir.path()).unwrap().check();
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let d = &result.diagnostics[0];
    assert_eq!(&source[d.span.start..d.span.end], "planned");
    assert_eq!(d.span.line, 4);
    let shacl = d.shacl.as_ref().unwrap();
    assert_eq!(shacl.source_field.as_deref(), Some("/status"));
    assert_eq!(
        shacl.result_severity,
        "http://www.w3.org/ns/shacl#Violation"
    );
    assert!(
        matches!(&shacl.focus_node, RdfTerm::Iri {value} if value == "urn:mdx:document:id:N-1")
    );
    assert!(
        matches!(&shacl.source_shape, Some(RdfTerm::Iri {value}) if value == "urn:policy:Status")
    );
    assert!(
        matches!(&shacl.result_path, Some(RdfPath::Predicate {iri}) if iri == "urn:mdx:v1:field:status")
    );
    assert!(
        matches!(&shacl.value, Some(RdfTerm::Literal {value, datatype, ..}) if value == "planned" && datatype.ends_with("#string"))
    );
    assert!(
        shacl
            .messages
            .iter()
            .any(|m| m.value == "Неверный статус" && m.language.as_deref() == Some("ru"))
    );
    assert!(
        shacl
            .messages
            .iter()
            .any(|m| m.value == "Wrong status" && m.language.as_deref() == Some("en"))
    );
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["diagnostics"][0]["shacl"]["sourceField"], "/status");
    assert_eq!(
        serde_json::to_value(
            serde_json::from_value::<md_engine::model::CheckResult>(json.clone()).unwrap()
        )
        .unwrap(),
        json
    );
}

#[test]
fn nested_and_array_values_have_unambiguous_locations() {
    let source = "---\nid: N-1\nreview:\n  owner: nobody\nstates: [draft, planned]\n---\n# Note\n";
    let dir = fixture(
        source,
        "sh:property [ sh:path f:review.owner ; sh:in (\"team\") ] ; sh:property [ sh:path f:states ; sh:in (\"draft\") ] .",
    );
    let result = Workspace::load(dir.path()).unwrap().check();
    assert_eq!(result.diagnostics.len(), 2, "{result:?}");
    for d in &result.diagnostics {
        let field = d.shacl.as_ref().unwrap().source_field.as_deref().unwrap();
        match field {
            "/review/owner" => assert_eq!(&source[d.span.start..d.span.end], "nobody"),
            "/states/1" => assert_eq!(&source[d.span.start..d.span.end], "planned"),
            _ => panic!("{field}: {d:?}"),
        }
    }
}

#[test]
fn missing_fields_complex_paths_and_sparql_use_document_fallback() {
    for policy in [
        "sh:property [ sh:path f:missing ; sh:minCount 1 ] .",
        "sh:property [ sh:path (f:related f:status) ; sh:minCount 1 ] .",
        "sh:sparql [ sh:select \"SELECT $this WHERE {}\" ] .",
    ] {
        let source = "---\nid: N-1\n---\n# Note\n";
        let dir = fixture(source, policy);
        let ws = Workspace::load(dir.path()).unwrap();
        let result = ws.check();
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        let d = &result.diagnostics[0];
        assert!(d.shacl.as_ref().unwrap().source_field.is_none());
        assert_eq!(d.span.start, 0);
        assert_eq!(d.span.end, ws.documents[0].body_start);
    }
}

#[test]
fn relation_items_map_back_to_reference_tokens_not_target_documents() {
    let source = "---\nid: N-1\nrelated: [N-2, N-3]\n---\n# Note\n";
    let dir = fixture(
        source,
        "sh:property [ sh:path f:related ; sh:class <urn:other-type> ] .",
    );
    put(
        dir.path(),
        "docs/target-2.md",
        "---\nid: N-2\n---\n# Second\n",
    );
    put(
        dir.path(),
        "docs/target-3.md",
        "---\nid: N-3\n---\n# Third\n",
    );
    let ws = Workspace::load(dir.path()).unwrap();
    assert_eq!(ws.relations.len(), 2);
    for relation in &ws.relations {
        assert_eq!(
            &source[relation.span.start..relation.span.end],
            relation.reference
        );
    }
    let result = ws.check();
    assert_eq!(result.diagnostics.len(), 2, "{result:?}");
    for d in &result.diagnostics {
        let value = &source[d.span.start..d.span.end];
        assert!(value == "N-2" || value == "N-3", "{d:?}");
        assert!(
            d.shacl
                .as_ref()
                .unwrap()
                .source_field
                .as_deref()
                .unwrap()
                .starts_with("/related/")
        );
    }
}

#[test]
fn structured_reports_survive_sql_index_and_legacy_index_migration() {
    let dir = fixture(
        "---\nid: N-1\nstatus: planned\n---\n# Note\n",
        "sh:property [ sh:path f:status ; sh:in (\"draft\") ] .",
    );
    let ws = Workspace::load(dir.path()).unwrap();
    index::index(&ws, false).unwrap();
    let query = "SELECT json_extract(shacl,'$.focusNode.value') AS focus, json_extract(shacl,'$.sourceField') AS field FROM diagnostics";
    let expected = serde_json::json!([{"focus":"urn:mdx:document:id:N-1","field":"/status"}]);
    assert_eq!(index::query(dir.path(), query).unwrap()["rows"], expected);
    let conn = rusqlite::Connection::open(dir.path().join(".mdx/index.sqlite3")).unwrap();
    conn.execute("ALTER TABLE diagnostics DROP COLUMN shacl", [])
        .unwrap();
    drop(conn);
    assert_eq!(index::index(&ws, false).unwrap().updated, 0);
    assert_eq!(index::query(dir.path(), query).unwrap()["rows"], expected);
}

#[test]
fn aliases_multiline_values_and_duplicate_array_values_never_fake_locations() {
    let source = "---\nid: N-1\ndefault: &state planned\nstatus: *state\nsettings: &settings {owner: nobody}\nreview: *settings\ntext: |-\n  first line\n  second line\nstates: [planned, planned]\n---\n# Note\n";
    let doc = parser::parse("note.md".into(), source.into());
    let alias = doc.metadata_spans.get("/status").unwrap();
    assert_eq!(&source[alias.start..alias.end], "*state");
    assert!(
        !doc.metadata_spans.contains_key("/review/owner"),
        "alias child locations must not point at a remote definition"
    );
    let text = doc.metadata_spans.get("/text").unwrap();
    assert!(source[text.start..text.end].contains("second line"));
    let dir = fixture(
        source,
        "sh:property [ sh:path f:states ; sh:in (\"draft\") ] .",
    );
    let result = Workspace::load(dir.path()).unwrap().check();
    assert_eq!(
        result.diagnostics.len(),
        1,
        "RDF collapses duplicate values: {result:?}"
    );
    let d = &result.diagnostics[0];
    assert_eq!(
        d.shacl.as_ref().unwrap().source_field.as_deref(),
        Some("/states")
    );
    assert_eq!(&source[d.span.start..d.span.end], "states");
}

#[test]
fn older_diagnostics_deserialize_without_shacl_and_local_errors_omit_it() {
    let old = serde_json::json!({"path":"note.md", "span":{"start":0,"end":0,"line":1,"column":1,"endLine":1,"endColumn":1},"rule":"schema.test","severity":"error","message":"old"});
    let diagnostic: md_engine::model::Diagnostic = serde_json::from_value(old.clone()).unwrap();
    assert!(diagnostic.shacl.is_none());
    assert_eq!(serde_json::to_value(diagnostic).unwrap(), old);
    let doc = parser::parse("note.md".into(), "---\nnot a mapping\n---\n".into());
    assert!(
        serde_json::to_value(&doc.diagnostics)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d.get("shacl").is_none())
    );
}

#[test]
fn body_paths_use_body_ranges_and_complex_paths_are_structured() {
    let source = "---\nid: N-1\n---\n# Note\nTODO\n";
    let dir = fixture(
        source,
        "sh:property [ sh:path <urn:mdx:v1:body> ; sh:maxLength 1 ] ; sh:property [ sh:path [ sh:inversePath f:related ] ; sh:minCount 1 ] .",
    );
    let ws = Workspace::load(dir.path()).unwrap();
    let result = ws.check();
    assert_eq!(result.diagnostics.len(), 2, "{result:?}");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.span.start == ws.documents[0].body_start && d.span.end == source.len())
    );
    assert!(result.diagnostics.iter().any(|d| matches!(
        &d.shacl.as_ref().unwrap().result_path,
        Some(RdfPath::Inverse { .. })
    )));
    let first = serde_json::to_value(result).unwrap();
    assert_eq!(
        first,
        serde_json::to_value(Workspace::load(dir.path()).unwrap().check()).unwrap()
    );
}
