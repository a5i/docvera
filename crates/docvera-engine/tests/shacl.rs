#![cfg(all(feature = "sql", feature = "shacl"))]
use docvera_engine::{Workspace, model::RdfTerm};
use std::{fs, path::Path};
use tempfile::TempDir;

const CONFIG: &str = r#"version = 1
include = ["docs/*.md"]
[schemas]
"docs/feature*.md" = "schemas/feature.yaml"
"docs/epic*.md" = "schemas/epic.yaml"
[validation.shacl]
shapes = ["policies/core.ttl"]
"#;
const PREFIX: &str = "@prefix sh: <http://www.w3.org/ns/shacl#> . @prefix f: <urn:docvera:v1:field:> . @prefix type: <urn:docvera:v1:type:> .\n";
const CORE: &str = r#"<urn:policy:Feature> a sh:NodeShape ; sh:targetClass type:feature ; sh:property <urn:policy:EpicRelation> .
<urn:policy:EpicRelation> a sh:PropertyShape ; sh:path f:epic ; sh:minCount 1 ; sh:maxCount 1 ; sh:class type:epic ; sh:message "Feature must have exactly one Epic" ."#;
fn put(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn fixture() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "docvera.toml", CONFIG);
    for kind in ["feature", "epic"] {
        put(
            dir.path(),
            &format!("schemas/{kind}.yaml"),
            &format!(
                "schemaVersion: 1\ndocumentType: {kind}\nidentity: {{field: frontMatter.id, uniqueWithin: workspace}}\nbody: {{additionalSections: true, allowPreamble: true}}\nrelations:\n  - {{from: frontMatter.epic, min: 0}}\n"
            ),
        );
    }
    put(
        dir.path(),
        "docs/feature.md",
        "---\nid: FEAT-1\nepic: EPIC-1\nstatus: planned\n---\n# Feature\n",
    );
    put(
        dir.path(),
        "docs/epic.md",
        "---\nid: EPIC-1\nstatus: draft\n---\n# Epic\n",
    );
    put(dir.path(), "policies/core.ttl", &format!("{PREFIX}{CORE}"));
    dir
}
fn check(root: &Path) -> docvera_engine::model::CheckResult {
    Workspace::load(root).unwrap().check()
}

#[test]
fn multiple_violations_on_ties_are_deterministically_sorted() {
    let dir = fixture();
    put(
        dir.path(),
        "policies/core.ttl",
        &format!(
            "{PREFIX}<urn:a> a sh:NodeShape ; sh:targetClass type:feature ;
                sh:property [ sh:path f:epic ; sh:minCount 1 ] ;
                sh:property [ sh:path f:owner ; sh:minCount 1 ] ;
                sh:property [ sh:path f:title ; sh:minCount 1 ] ."
        ),
    );
    put(
        dir.path(),
        "docs/feature.md",
        "---\nid: FEAT-1\nstatus: planned\n---\n# Feature\n",
    );
    let first = check(dir.path());
    assert_eq!(first.diagnostics.len(), 3, "{first:?}");
    // Same fallback span on every finding: ordering must fall through to the
    // message, and blank-node property shapes stay out of the message.
    let messages: Vec<_> = first
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect();
    let mut sorted = messages.clone();
    sorted.sort();
    assert_eq!(messages, sorted);
    assert!(
        messages
            .iter()
            .all(|m| m.contains("[path: urn:docvera:v1:field:")
                && !m.contains("blank")
                && !m.contains("f0b"))
    );
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(check(dir.path())).unwrap()
    );
}

#[test]
fn document_without_front_matter_projects_path_identity_and_degenerate_span() {
    let dir = fixture();
    put(dir.path(), "docs/feature-plain.md", "# Just prose\n");
    let result = check(dir.path());
    assert_eq!(result.diagnostics.len(), 2, "{result:?}");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.rule == "identity.required")
    );
    let finding = result
        .diagnostics
        .iter()
        .find(|d| d.rule.starts_with("shacl."))
        .unwrap();
    assert_eq!(finding.path, "docs/feature-plain.md");
    assert_eq!((finding.span.start, finding.span.end), (0, 0));
    assert_eq!(finding.span.line, 1);
    let shacl = finding.shacl.as_ref().unwrap();
    assert!(shacl.source_field.is_none());
    assert!(
        matches!(&shacl.focus_node, RdfTerm::Iri { value } if value.starts_with("urn:docvera:document:path:docs%2Ffeature-plain")),
        "{shacl:?}"
    );
}

#[test]
fn policy_files_with_leading_bom_and_crlf_validate_identically() {
    // Turtle (RDF 1.1) permits a BOM as the first character, and Windows editors
    // emit it. CRLF is legal whitespace. Both must behave exactly like plain LF
    // files: the shape must actually take effect, not silently vanish.
    let prefix = "@prefix sh: <http://www.w3.org/ns/shacl#> . @prefix f: <urn:docvera:v1:field:> . @prefix type: <urn:docvera:v1:type:> .\n";
    let policy = format!(
        "{prefix}<urn:one> a sh:NodeShape ; sh:targetClass type:feature ; sh:property [ sh:path f:epic ; sh:minCount 1 ] ."
    );
    for (label, text) in [
        ("plain", policy.clone()),
        ("crlf", policy.replace("; ", ";\r\n")),
        ("bom", format!("\u{FEFF}{policy}")),
        (
            "bom-crlf",
            format!("\u{FEFF}{}", policy.replace("; ", ";\r\n")),
        ),
        (
            "bom-crlf",
            format!("\u{FEFF}{PREFIX}{}", policy.replace("; ", ";\r\n")),
        ),
    ] {
        let dir = fixture();
        put(dir.path(), "policies/core.ttl", &text);
        put(
            dir.path(),
            "docs/feature.md",
            "---\nid: FEAT-1\n---\n# Feature\n",
        );
        let result = check(dir.path());
        assert_eq!(result.diagnostics.len(), 1, "{label}: {result:?}");
        assert_eq!(
            result.diagnostics[0].rule, "shacl.MinCountConstraintComponent",
            "{label}"
        );
    }
}

#[test]
fn mid_file_bom_is_a_parse_error_not_a_shape() {
    // U+FEFF is only whitespace at offset 0 per the grammar; elsewhere it must
    // fail the load instead of being tolerated into validation.
    let dir = fixture();
    put(
        dir.path(),
        "policies/core.ttl",
        // A BOM in a token position would corrupt IRIs (Oxigraph swallows it
        // into the local name), so the loader must reject it explicitly.
        &format!("{PREFIX}<urn:one> a sh:NodeShape\u{FEFF} ; sh:targetClass type:feature ."),
    );
    assert!(error(dir.path()).contains("BOM is only allowed as the first character"));
}
fn error(root: &Path) -> String {
    match Workspace::load(root) {
        Ok(_) => panic!("expected load failure"),
        Err(e) => format!("{e:#}"),
    }
}

#[test]
fn core_cardinality_type_resolution_and_markdown_provenance() {
    let dir = fixture();
    assert!(check(dir.path()).diagnostics.is_empty());
    assert!(!dir.path().join(".docvera").exists());
    for (reference, rule) in [
        ("", "MinCountConstraintComponent"),
        ("epic: [EPIC-1, EPIC-2]\n", "MaxCountConstraintComponent"),
        ("epic: FEAT-1\n", "ClassConstraintComponent"),
        ("epic: MISSING\n", "MinCountConstraintComponent"),
        ("epic: ../outside.md\n", "MinCountConstraintComponent"),
    ] {
        put(
            dir.path(),
            "docs/feature.md",
            &format!("---\nid: FEAT-1\n{reference}---\n# Feature\n"),
        );
        put(
            dir.path(),
            "docs/epic-2.md",
            "---\nid: EPIC-2\n---\n# Epic\n",
        );
        let result = check(dir.path());
        assert!(!result.valid(), "{reference}");
        let finding = result
            .diagnostics
            .iter()
            .find(|d| d.rule == format!("shacl.{rule}"))
            .unwrap_or_else(|| panic!("{reference}: {result:?}"));
        assert_eq!(finding.path, "docs/feature.md");
        if reference.is_empty() {
            assert_eq!(finding.span.start, 0);
        } else {
            assert!(finding.span.start > 0);
        }
        assert!(finding.span.end > 3);
        assert!(finding.message.contains("urn:policy:EpicRelation"));
        assert!(finding.message.contains("urn:docvera:v1:field:epic"));
        if reference.contains("MISSING") || reference.contains("outside") {
            assert!(
                result
                    .diagnostics
                    .iter()
                    .any(|d| d.rule == "relation.target")
            );
        }
    }
    put(
        dir.path(),
        "docs/feature.md",
        "---\nid: FEAT-1\nepic: [EPIC-1, EPIC-1]\n---\n# Feature\n",
    );
    assert!(check(dir.path()).valid(), "RDF counts distinct targets");
    put(
        dir.path(),
        "docs/feature.md",
        "---\nid: FEAT-1\nepic: epic.md\n---\n# Feature\n",
    );
    assert!(
        check(dir.path()).valid(),
        "path references use the resolved graph"
    );
}

#[test]
fn cross_document_sparql_is_index_free_and_deterministic() {
    let dir = fixture();
    put(
        dir.path(),
        "policies/core.ttl",
        &format!(
            r#"{PREFIX}
        <urn:policy:DraftEpic> a sh:NodeShape ; sh:targetClass type:epic ; sh:sparql [
            sh:message "Draft epic has a non-draft child" ;
            sh:select """PREFIX f: <urn:docvera:v1:field:>
                SELECT $this WHERE {{ $this f:status "draft" . ?child f:epic $this .
                    FILTER NOT EXISTS {{ ?child f:status "draft" }} }}""" ] ."#
        ),
    );
    let first = check(dir.path());
    assert!(!first.valid());
    assert_eq!(first.diagnostics.len(), 1);
    assert_eq!(first.diagnostics[0].path, "docs/epic.md");
    assert!(first.diagnostics[0].message.contains("non-draft child"));
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(check(dir.path())).unwrap()
    );
    assert!(!dir.path().join(".docvera").exists());
    put(
        dir.path(),
        "docs/feature.md",
        "---\nid: FEAT-1\nepic: epic.md\nstatus: draft\n---\n# Feature\n",
    );
    assert!(check(dir.path()).valid());
}

#[test]
fn shape_loading_and_engine_failures_never_pass_silently() {
    for (policy, expected) in [
        ("not Turtle", "Invalid SHACL Turtle"),
        ("", "no shapes with targets"),
        (
            "<urn:s> a sh:NodeShape ; sh:targetClass type:feature ; sh:minCount \"bad\" .",
            "SHACL",
        ),
        (
            "<urn:s> a sh:NodeShape ; sh:targetClass type:feature ; sh:unknownRule true .",
            "Unsupported SHACL predicate",
        ),
        (
            "<urn:s> a sh:NodeShape ; sh:targetClass type:feature ; sh:sparql [ sh:select \"SELECT $this WHERE { SERVICE <https://example.invalid/> { ?s ?p ?o } }\" ] .",
            "SERVICE/FROM",
        ),
        (
            "<urn:s> a sh:NodeShape ; sh:targetClass type:feature ; sh:property <urn:p> . <urn:p> sh:path f:epic, f:owner .",
            "exactly one sh:path",
        ),
        (
            "<urn:s> a sh:NodeShape ; sh:targetClass type:feature ; sh:sparql [ sh:select \"not a query\" ] .",
            "SHACL",
        ),
        (
            "<urn:s> a sh:NodeShape ; sh:targetClass type:feature ; sh:sparql [ sh:message \"Missing query\" ] .",
            "SHACL",
        ),
        (
            "<urn:s> a sh:NodeShape ; sh:targetClass type:feature ; sh:property [ sh:minCount 1 ] .",
            "SHACL",
        ),
        (
            "<urn:s> <http://www.w3.org/2002/07/owl#imports> <https://example.invalid/> .",
            "imports",
        ),
    ] {
        let dir = fixture();
        put(
            dir.path(),
            "policies/core.ttl",
            &format!("{PREFIX}{policy}"),
        );
        assert!(
            error(dir.path()).contains(expected),
            "{policy}: {}",
            error(dir.path())
        );
    }
    let dir = fixture();
    fs::remove_file(dir.path().join("policies/core.ttl")).unwrap();
    assert!(error(dir.path()).contains("Cannot load SHACL shapes"));
}

#[test]
fn multiple_shape_files_keep_prefixes_and_blank_nodes_local() {
    let dir = fixture();
    put(
        dir.path(),
        "docvera.toml",
        &CONFIG.replace(
            "[\"policies/core.ttl\"]",
            "[\"policies/core.ttl\", \"policies/other.ttl\"]",
        ),
    );
    put(
        dir.path(),
        "policies/core.ttl",
        &format!(
            "{PREFIX}<urn:one> a sh:NodeShape ; sh:targetClass type:feature ; sh:property _:p . _:p sh:path f:epic ; sh:minCount 1 ."
        ),
    );
    put(
        dir.path(),
        "policies/other.ttl",
        &format!(
            "{PREFIX}@prefix f: <urn:other:> . <urn:two> a sh:NodeShape ; sh:targetClass type:epic ; sh:property _:p . _:p sh:path f:missing ; sh:minCount 1 ."
        ),
    );
    let result = check(dir.path());
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].path, "docs/epic.md");
    assert!(result.diagnostics[0].message.contains("urn:other:missing"));
}

#[test]
fn severity_mapping_and_shape_edits_invalidate_the_index_fingerprint() {
    let dir = fixture();
    let workspace = Workspace::load(dir.path()).unwrap();
    put(
        dir.path(),
        "policies/core.ttl",
        &format!(
            "{PREFIX}<urn:s> a sh:NodeShape ; sh:targetClass type:feature ; sh:property [ sh:path f:owner ; sh:minCount 1 ; sh:severity sh:Warning ] ."
        ),
    );
    let updated = Workspace::load(dir.path()).unwrap();
    assert_ne!(workspace.fingerprint, updated.fingerprint);
    let result = updated.check();
    assert!(result.valid());
    assert_eq!(result.diagnostics[0].severity, "warning");
    assert!(
        updated
            .query_sql("SELECT count(*) AS n FROM diagnostics")
            .unwrap()["rows"][0]["n"]
            .as_u64()
            .unwrap()
            >= 1
    );
    put(
        dir.path(),
        "policies/core.ttl",
        &fs::read_to_string(dir.path().join("policies/core.ttl"))
            .unwrap()
            .replace("sh:Warning", "sh:Info"),
    );
    let result = check(dir.path());
    assert!(result.valid());
    assert_eq!(result.diagnostics[0].severity, "info");
}

#[test]
fn duplicate_ids_are_explicitly_incomplete_not_passing() {
    let dir = fixture();
    put(
        dir.path(),
        "docs/epic-duplicate.md",
        "---\nid: EPIC-1\n---\n# Epic\n",
    );
    let result = check(dir.path());
    assert!(!result.valid());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.rule == "shacl.incomplete")
    );
}

#[test]
fn projection_preserves_datatypes_unicode_nested_fields_and_null_semantics() {
    let dir = fixture();
    put(
        dir.path(),
        "docs/feature.md",
        "---\nid: 'Юникод / %#'\nepic: EPIC-1\nactive: true\ncount: 3\nratio: 1.5\nlabels: [alpha, beta, alpha]\nreviewer: null\nreview:\n  owner: 'A \"quoted\" owner'\n---\n# Unicode body\n",
    );
    put(
        dir.path(),
        "policies/core.ttl",
        &format!(
            r#"{PREFIX}@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        <urn:s> a sh:NodeShape ; sh:targetClass type:feature ;
            sh:property [ sh:path f:active ; sh:datatype xsd:boolean ; sh:hasValue true ] ;
            sh:property [ sh:path f:count ; sh:datatype xsd:integer ; sh:hasValue 3 ] ;
            sh:property [ sh:path f:ratio ; sh:datatype xsd:double ] ;
            sh:property [ sh:path f:labels ; sh:minCount 2 ; sh:maxCount 2 ] ;
            sh:property [ sh:path f:reviewer ; sh:maxCount 0 ] ;
            sh:property [ sh:path f:review.owner ; sh:minCount 1 ; sh:datatype xsd:string ] ."#
        ),
    );
    assert!(
        check(dir.path()).diagnostics.is_empty(),
        "{:?}",
        check(dir.path())
    );
}

#[cfg(unix)]
#[test]
fn shape_symlinks_cannot_escape_the_workspace() {
    let dir = fixture();
    let external = tempfile::tempdir().unwrap();
    put(external.path(), "external.ttl", &format!("{PREFIX}{CORE}"));
    fs::remove_file(dir.path().join("policies/core.ttl")).unwrap();
    std::os::unix::fs::symlink(
        external.path().join("external.ttl"),
        dir.path().join("policies/core.ttl"),
    )
    .unwrap();
    assert!(error(dir.path()).contains("escapes workspace"));
}

#[test]
fn policy_paths_are_explicit_local_turtle_files() {
    let external = tempfile::tempdir().unwrap();
    // Use a native absolute path; `/tmp/...` is only drive-rooted on Windows.
    // JSON string escaping is also valid for these TOML basic strings.
    let absolute_paths = serde_json::to_string(&[external.path().join("policy.ttl")]).unwrap();
    for (paths, expected) in [
        ("[]", "must not be empty"),
        ("[\"policies/missing.ttl\"]", "Cannot load SHACL"),
        ("[\"policies/core.json\"]", "workspace-relative .ttl"),
        (absolute_paths.as_str(), "workspace-relative .ttl"),
        ("[\"policies/*.ttl\"]", "Cannot load SHACL"),
        (
            "[\"policies/core.ttl\", \"policies/../policies/core.ttl\"]",
            "Duplicate SHACL",
        ),
    ] {
        let dir = fixture();
        put(
            dir.path(),
            "docvera.toml",
            &CONFIG.replace("[\"policies/core.ttl\"]", paths),
        );
        assert!(
            error(dir.path()).contains(expected),
            "{paths}: {}",
            error(dir.path())
        );
    }
}

#[test]
fn legacy_yaml_configuration_runs_the_same_shacl_policies() {
    let dir = fixture();
    put(
        dir.path(),
        "docs/feature.md",
        "---\nid: FEAT-1\n---\n# Feature\n",
    );
    let before = serde_json::to_value(check(dir.path())).unwrap();
    fs::remove_file(dir.path().join("docvera.toml")).unwrap();
    put(
        dir.path(),
        "docvera.yaml",
        "version: 1\ninclude: ['docs/*.md']\nschemas:\n  'docs/feature*.md': schemas/feature.yaml\n  'docs/epic*.md': schemas/epic.yaml\nvalidation:\n  shacl:\n    shapes: [policies/core.ttl]\n",
    );
    assert_eq!(before, serde_json::to_value(check(dir.path())).unwrap());
}

#[test]
fn custom_constraints_dsl_is_rejected_in_document_schemas() {
    let dir = fixture();
    let schema = fs::read_to_string(dir.path().join("schemas/feature.yaml")).unwrap();
    put(
        dir.path(),
        "schemas/feature.yaml",
        &format!("{schema}constraints: []\n"),
    );
    let result = check(dir.path());
    assert!(!result.valid());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.rule == "schema.resolve" && d.message.contains("constraints"))
    );
}
