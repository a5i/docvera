#![cfg(feature = "sql")]
use docvera_engine::{CheckerContext, WorkspaceSources, config::Config, sql::SqlSnapshot};
use serde_json::json;

fn checker() -> CheckerContext {
    let config: Config = serde_json::from_value(json!({
        "version": 1, "include": ["**/*.md"], "schemas": {},
        "validation": {"untyped": "skip"}
    }))
    .unwrap();
    let sources = WorkspaceSources {
        documents: [
            ("a.md".into(), "---\ntitle: Привет ' SQL\nowner: team-a\ntags: [rust, wasm]\nsettings: {enabled: true}\n---\n# Alpha\nSee [Beta](b.md).\n".into()),
            ("b.md".into(), "# Beta\n```rust\nfn main() {}\n```\n".into()),
        ].into(),
        ..Default::default()
    };
    CheckerContext::from_sources(config, sources).unwrap()
}

#[test]
fn queries_structured_values_joins_and_empty_tables() {
    let workspace = checker().snapshot().unwrap();
    let mut sql = SqlSnapshot::from_workspace(&workspace).unwrap();
    assert_eq!(
        sql.query("SELECT path, metadata, declared_id FROM documents ORDER BY path")
            .unwrap(),
        json!({
            "contractVersion": 1,
            "rows": [
                {"path": "a.md", "metadata": {"title": "Привет ' SQL", "owner": "team-a", "tags": ["rust", "wasm"], "settings": {"enabled": true}}, "declared_id": null},
                {"path": "b.md", "metadata": {}, "declared_id": null}
            ]
        })
    );
    assert_eq!(
        sql.query("SELECT metadata['owner'] AS owner FROM documents WHERE path = 'a.md'")
            .unwrap()["rows"],
        json!([{"owner": "team-a"}])
    );
    let rows = sql.query("SELECT d.path, s.heading FROM documents d JOIN sections s ON s.document_id = d.path WHERE s.level = 1 ORDER BY d.path").unwrap();
    assert_eq!(
        rows["rows"],
        json!([{"path": "a.md", "heading": "Alpha"}, {"path": "b.md", "heading": "Beta"}])
    );
    assert_eq!(
        sql.query("SELECT COUNT(*) AS n FROM diagnostics").unwrap()["rows"],
        json!([{"n": 0}])
    );
    assert!(
        !sql.query("SELECT * FROM nodes").unwrap()["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !sql.query("SELECT * FROM relations").unwrap()["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let sections = sql.query("SELECT span FROM sections LIMIT 1").unwrap();
    assert!(sections["rows"][0]["span"]["start"].is_number());
}

#[test]
fn snapshots_are_independent_of_edits() {
    let mut checker = checker();
    let mut old = SqlSnapshot::from_workspace(&checker.snapshot().unwrap()).unwrap();
    checker.remove_document("b.md").unwrap();
    checker
        .update_document("a.md", "# Updated\n".into())
        .unwrap();
    let mut new = SqlSnapshot::from_workspace(&checker.snapshot().unwrap()).unwrap();
    assert_eq!(
        old.query("SELECT COUNT(*) AS n FROM documents").unwrap()["rows"],
        json!([{"n": 2}])
    );
    assert_eq!(
        new.query("SELECT COUNT(*) AS n FROM documents").unwrap()["rows"],
        json!([{"n": 1}])
    );
    assert_eq!(
        new.query("SELECT heading FROM sections WHERE level = 1")
            .unwrap()["rows"],
        json!([{"heading": "Updated"}])
    );
}

#[test]
fn rejects_mutation_multiple_statements_and_ambiguous_labels() {
    let mut sql = SqlSnapshot::from_workspace(&checker().snapshot().unwrap()).unwrap();
    for query in [
        "",
        "SELECT 1; SELECT 2",
        "DELETE FROM documents",
        "DROP TABLE documents",
        "UPDATE documents SET path = 'x'",
        "CREATE TABLE x (id INT)",
        "INSERT INTO documents (path) VALUES ('x')",
        "BEGIN",
        "COMMIT",
        "SHOW TABLES",
        "SELECT path AS x, source AS x FROM documents",
        "SELECT * FROM missing",
    ] {
        assert!(sql.query(query).is_err(), "Unexpectedly accepted: {query}");
    }
    assert_eq!(
        sql.query("SELECT COUNT(*) AS n FROM documents").unwrap()["rows"],
        json!([{"n": 2}])
    );
}

#[test]
fn row_cap_and_empty_workspace() {
    let mut checker = checker();
    for i in 0..1000 {
        checker
            .update_document(&format!("extra-{i}.md"), "# Extra\n".into())
            .unwrap();
    }
    let mut sql = SqlSnapshot::from_workspace(&checker.snapshot().unwrap()).unwrap();
    assert!(
        sql.query("SELECT path FROM documents")
            .unwrap_err()
            .to_string()
            .contains("1000")
    );
    assert_eq!(
        sql.query("SELECT path FROM documents LIMIT 3").unwrap()["rows"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let config =
        serde_json::from_value(json!({"version": 1, "include": ["**/*.md"], "schemas": {}}))
            .unwrap();
    let checker = CheckerContext::from_sources(config, WorkspaceSources::default()).unwrap();
    let mut sql = SqlSnapshot::from_workspace(&checker.snapshot().unwrap()).unwrap();
    assert_eq!(
        sql.query("SELECT * FROM documents").unwrap()["rows"],
        json!([])
    );
}
