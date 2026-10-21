use docvera_engine::{Workspace, schema::Schema};
use std::{fs, path::Path};
use tempfile::TempDir;

fn fixture(type_fields: &str, references: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join("docvera.toml"),
        "version = 1\ninclude = [\"*.md\"]\n[schemas]\n\"source.md\" = \"source.yaml\"\n\"req*.md\" = \"requirement.yaml\"\n\"rule*.md\" = \"rule.yaml\"\n\"other.md\" = \"other.yaml\"\n",
    ).unwrap();
    fs::write(
        root.join("source.yaml"),
        format!("schemaVersion: 1\ndocumentType: scenario\nbody: {{ additionalSections: true }}\nrelations:\n  - from: frontMatter.verifies\n    {type_fields}\n"),
    ).unwrap();
    for kind in ["requirement", "rule", "other"] {
        fs::write(
            root.join(format!("{kind}.yaml")),
            format!("schemaVersion: 1\ndocumentType: {kind}\nbody: {{ additionalSections: true }}\nidentity: {{ field: frontMatter.id, uniqueWithin: documentType }}\n"),
        ).unwrap();
    }
    document(root, "source.md", &format!("verifies: {references}"));
    document(root, "req.md", "id: REQ-1");
    document(root, "rule.md", "id: RULE-1");
    document(root, "other.md", "id: OTHER-1");
    fs::write(root.join("asset.txt"), "asset").unwrap();
    dir
}

fn document(root: &Path, path: &str, metadata: &str) {
    fs::write(root.join(path), format!("---\n{metadata}\n---\n# Target\n")).unwrap();
}

#[test]
fn union_accepts_each_allowed_type_by_id_and_path() {
    let dir = fixture(
        "targetTypes: [requirement, rule]",
        "[REQ-1, 'id:RULE-1', 'req.md#target', 'rule.md#target']",
    );
    let ws = Workspace::load(dir.path()).unwrap();
    assert!(ws.check().valid(), "{:?}", ws.check().diagnostics);
    assert_eq!(ws.relations.len(), 4);
    assert!(ws.relations.iter().all(|r| r.status == "resolved"));

    // Alternatives do not require coverage of every listed type.
    document(dir.path(), "source.md", "verifies: REQ-1");
    assert!(Workspace::load(dir.path()).unwrap().check().valid());
}

#[test]
fn union_rejects_wrong_types_missing_targets_assets_and_bad_anchors() {
    let dir = fixture(
        "targetTypes: [requirement, rule]\n    mustExist: false",
        "[OTHER-1, other.md, MISSING, asset.txt, 'req.md#absent']",
    );
    let ws = Workspace::load(dir.path()).unwrap();
    let statuses: Vec<_> = ws.relations.iter().map(|r| r.status.as_str()).collect();
    assert_eq!(
        statuses,
        [
            "wrong-type",
            "wrong-type",
            "missing",
            "missing",
            "missing-anchor"
        ]
    );
    // mustExist:false suppresses missing targets only.
    assert_eq!(ws.check().diagnostics.len(), 3);
    assert!(
        ws.check()
            .diagnostics
            .iter()
            .all(|d| d.rule == "relation.target")
    );

    fs::write(dir.path().join("source.yaml"), "schemaVersion: 1\ndocumentType: scenario\nbody: { additionalSections: true }\nrelations:\n  - from: frontMatter.verifies\n    targetTypes: [requirement, rule]\n").unwrap();
    assert_eq!(
        Workspace::load(dir.path())
            .unwrap()
            .check()
            .diagnostics
            .len(),
        5
    );
}

#[test]
fn union_filters_id_candidates_but_never_picks_between_allowed_types() {
    let dir = fixture("targetTypes: [requirement, rule]", "SHARED");
    document(dir.path(), "req.md", "id: SHARED");
    document(dir.path(), "other.md", "id: SHARED");
    let ws = Workspace::load(dir.path()).unwrap();
    assert!(ws.check().valid());
    assert_eq!(ws.relations[0].target.as_deref(), Some("req.md"));

    document(dir.path(), "rule.md", "id: SHARED");
    let ws = Workspace::load(dir.path()).unwrap();
    assert_eq!(ws.relations[0].status, "ambiguous");
}

#[test]
fn singleton_list_and_legacy_single_type_have_identical_resolution() {
    let references = "[REQ-1, RULE-1, req.md, rule.md, MISSING, asset.txt]";
    let legacy = fixture("targetType: requirement", references);
    let plural = fixture("targetTypes: [requirement]", references);
    let a = Workspace::load(legacy.path()).unwrap();
    let b = Workspace::load(plural.path()).unwrap();
    let a = serde_json::to_value(a.check()).unwrap();
    let b = serde_json::to_value(b.check()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn invalid_target_type_lists_are_schema_errors() {
    for fields in [
        "targetTypes: []",
        "targetTypes: ['']",
        "targetTypes: ['   ']",
        "targetTypes: [rule, rule]",
        "targetType: rule\n    targetTypes: [requirement]",
        "targetTypes: rule",
    ] {
        let text = format!(
            "schemaVersion: 1\ndocumentType: scenario\nrelations:\n  - from: frontMatter.verifies\n    {fields}\n"
        );
        let result = serde_saphyr::from_str::<Schema>(&text)
            .map_err(anyhow::Error::from)
            .and_then(|schema| schema.check_definition());
        assert!(result.is_err(), "Accepted invalid schema: {fields}");
    }
}
