#![cfg(all(feature = "index", feature = "shacl"))]
#[path = "../../../tests/support/mod.rs"]
mod support;

use docvera_engine::{Workspace, index};
use serde_json::{Value, json};
use std::{fs, path::Path};
use support::{SCENARIOS, edit, fixture, reports};

fn check(root: &Path) -> docvera_engine::model::CheckResult {
    Workspace::load(root).unwrap().check()
}

fn refresh(root: &Path) {
    let ws = Workspace::load(root).unwrap();
    assert!(ws.check().valid(), "{:?}", ws.check().diagnostics);
    index::index(&ws, false).unwrap();
}

fn report(root: &Path, name: &str) -> Value {
    let sql = fs::read_to_string(root.join("queries").join(format!("{name}.sql"))).unwrap();
    index::query(root, &sql).unwrap()["rows"].clone()
}

#[test]
fn all_examples_validate_and_every_report_has_exact_baseline_rows() {
    for &(name, count) in SCENARIOS {
        let dir = fixture(name);
        let root = dir.path();
        assert!(!root.join(".docvera").exists());
        assert!(
            check(root).diagnostics.is_empty(),
            "{name}: {:?}",
            check(root)
        );
        assert!(!root.join(".docvera").exists(), "check must be index-free");
        let ws = Workspace::load(root).unwrap();
        assert_eq!(ws.documents.len(), count, "{name}");
        assert_eq!(index::index(&ws, false).unwrap().updated, count);
        assert_eq!(index::index(&ws, false).unwrap().updated, 0);
        for path in reports(root) {
            let rows = index::query(root, &fs::read_to_string(&path).unwrap()).unwrap();
            let expected = if path.file_stem().unwrap() == "coverage" {
                json!([
                    {"requirement":"REQ-001", "tests":1, "latest_result":"passed"},
                    {"requirement":"REQ-002", "tests":0, "latest_result":null}
                ])
            } else {
                json!([])
            };
            assert_eq!(rows["rows"], expected, "{name}: {}", path.display());
        }
    }
}

#[test]
fn every_scenario_rejects_missing_owner_missing_section_duplicate_id_and_wrong_type() {
    for (name, file, owner, heading, id, relation, wrong_target) in [
        (
            "product-traceability",
            "docs/feature.md",
            "identity-team",
            "Problem",
            "FEAT-001",
            "initiative: INIT-001",
            "initiative: REQ-001",
        ),
        (
            "qa-traceability",
            "docs/test.md",
            "qa",
            "Preconditions",
            "TC-001",
            "requirements: [REQ-001]",
            "requirements: [REL-001]",
        ),
        (
            "operational-readiness",
            "docs/service.md",
            "identity-team",
            "Overview",
            "SVC-001",
            "runbook: RUN-001",
            "runbook: SLO-001",
        ),
        (
            "architecture-governance",
            "docs/component.md",
            "identity-team",
            "Responsibilities",
            "COMP-001",
            "interfaces: [API-001]",
            "interfaces: [ARCH-001]",
        ),
        (
            "project-governance",
            "docs/epic.md",
            "identity-team",
            "Summary",
            "PROJ-101",
            "specification: SPEC-001",
            "specification: MS-001",
        ),
        (
            "design-invariants",
            "docs/pattern-import.md",
            "identity-team",
            "Behavior",
            "PTN-1",
            "failureScenarios: [FAIL-1]",
            "failureScenarios: [RULE-1]",
        ),
    ] {
        for (from, to, rule) in [
            (
                format!("owner: {owner}\n"),
                String::new(),
                "schema.frontMatter",
            ),
            (
                format!("# {heading}"),
                "# Unexpected".into(),
                "body.cardinality",
            ),
            (relation.into(), wrong_target.into(), "relation.target"),
        ] {
            let dir = fixture(name);
            edit(dir.path(), file, &from, &to);
            let result = check(dir.path());
            assert!(!result.valid(), "{name}: {from}");
            assert!(
                result.diagnostics.iter().any(|d| d.rule == rule),
                "{name}: {:?}",
                result.diagnostics
            );
        }
        let dir = fixture(name);
        // Duplicate a mapped document using an explicit selector so every workspace can use the same path.
        let original = fs::read_to_string(dir.path().join(file)).unwrap();
        let schema = match name {
            "product-traceability" => "feature",
            "qa-traceability" => "test",
            "operational-readiness" => "service",
            "architecture-governance" => "component",
            "design-invariants" => "pattern",
            _ => "epic",
        };
        let duplicate = original.replacen(
            "---\n",
            &format!("---\n$schema: ../schemas/{schema}.yaml\n"),
            1,
        );
        fs::write(dir.path().join("docs/duplicate.md"), duplicate).unwrap();
        assert!(
            check(dir.path())
                .diagnostics
                .iter()
                .any(|d| d.rule == "identity.duplicate"),
            "{name}: {id}"
        );
    }
}

#[test]
fn product_orphan_report_detects_deleted_features() {
    let dir = fixture("product-traceability");
    refresh(dir.path());
    fs::remove_file(dir.path().join("docs/feature.md")).unwrap();
    fs::remove_file(dir.path().join("docs/feature-draft.md")).unwrap();
    refresh(dir.path());
    assert_eq!(
        report(dir.path(), "orphan-requirements"),
        json!([{"requirement":"REQ-001"}])
    );
}

#[test]
fn business_policy_mutations_are_reports_not_schema_failures() {
    for (name, file, from, to, query, expected) in [
        (
            "product-traceability",
            "docs/feature-draft.md",
            "status: draft",
            "status: planned",
            "missing-epics",
            json!([{"feature":"FEAT-002"}]),
        ),
        (
            "qa-traceability",
            "docs/requirement-uncovered.md",
            "priority: normal",
            "priority: critical",
            "uncovered-critical",
            json!([{"requirement":"REQ-002"}]),
        ),
        (
            "operational-readiness",
            "docs/service.md",
            "runbook: RUN-001\n",
            "",
            "missing-operational-docs",
            json!([{"service":"SVC-001"}]),
        ),
        (
            "operational-readiness",
            "docs/service.md",
            "slo: SLO-001\n",
            "",
            "missing-slos",
            json!([{"service":"SVC-001"}]),
        ),
        (
            "operational-readiness",
            "docs/runbook.md",
            "2026-10-01",
            "2026-09-19",
            "overdue-reviews",
            json!([{"runbook":"RUN-001","reviewed_at":"2026-09-19"}]),
        ),
        (
            "architecture-governance",
            "docs/component.md",
            "interfaces: [API-001]",
            "interfaces: [API-001]\ndependsOn: [COMP-002]",
            "layer-violations",
            json!([{"source":"COMP-001","target":"COMP-002"}]),
        ),
        (
            "architecture-governance",
            "docs/adr.md",
            "components: [COMP-001, COMP-002]\n",
            "",
            "unlinked-adrs",
            json!([{"adr":"ADR-001"}]),
        ),
        (
            "project-governance",
            "docs/specification.md",
            "status: approved",
            "status: draft",
            "documentation-drift",
            json!([{"epic":"PROJ-101","specification":"SPEC-001","specification_status":"draft"}]),
        ),
        (
            "project-governance",
            "docs/epic.md",
            "status: done",
            "status: in-progress",
            "incomplete-milestones",
            json!([{"milestone":"MS-001","deliverable":"PROJ-101"}]),
        ),
    ] {
        let dir = fixture(name);
        refresh(dir.path());
        let original = fs::read_to_string(dir.path().join(file)).unwrap();
        edit(dir.path(), file, from, to);
        // Reports still read the old snapshot until an explicit refresh.
        assert_eq!(report(dir.path(), query), json!([]));
        refresh(dir.path());
        assert_eq!(report(dir.path(), query), expected, "{name}: {query}");
        fs::write(dir.path().join(file), original).unwrap();
        refresh(dir.path());
        assert_eq!(report(dir.path(), query), json!([]));
    }
}

#[test]
fn every_scenario_reindexes_deletions_and_inbound_references() {
    for (name, file, id) in [
        ("product-traceability", "docs/requirement.md", "REQ-001"),
        ("qa-traceability", "docs/test.md", "TC-001"),
        ("operational-readiness", "docs/runbook.md", "RUN-001"),
        ("architecture-governance", "docs/interface.md", "API-001"),
        ("project-governance", "docs/specification.md", "SPEC-001"),
        (
            "design-invariants",
            "docs/failure-scenario-import-network.md",
            "FAIL-1",
        ),
    ] {
        let dir = fixture(name);
        refresh(dir.path());
        fs::remove_file(dir.path().join(file)).unwrap();
        let ws = Workspace::load(dir.path()).unwrap();
        assert!(!ws.check().valid());
        assert!(ws.relations.iter().any(|r| r.status == "missing"));
        assert_eq!(index::index(&ws, false).unwrap().deleted, 1);
        assert_eq!(
            index::query(
                dir.path(),
                &format!("SELECT declared_id FROM documents WHERE declared_id='{id}'")
            )
            .unwrap()["rows"],
            json!([])
        );
        assert_eq!(
            index::query(
                dir.path(),
                &format!("SELECT section_id FROM sections WHERE document_id='{file}'")
            )
            .unwrap()["rows"],
            json!([])
        );
    }
}

#[test]
fn qa_latest_execution_is_selected_by_date_not_worst_historical_result() {
    let dir = fixture("qa-traceability");
    refresh(dir.path());
    fs::remove_file(dir.path().join("docs/execution.md")).unwrap();
    refresh(dir.path());
    assert_eq!(
        report(dir.path(), "coverage"),
        json!([
            {"requirement":"REQ-001","tests":1,"latest_result":"failed"},
            {"requirement":"REQ-002","tests":0,"latest_result":null}
        ])
    );
    edit(
        dir.path(),
        "docs/test.md",
        "status: active",
        "status: retired",
    );
    refresh(dir.path());
    assert_eq!(
        report(dir.path(), "uncovered-critical"),
        json!([{"requirement":"REQ-001"}])
    );
    assert_eq!(
        report(dir.path(), "coverage")[0],
        json!({"requirement":"REQ-001","tests":0,"latest_result":null})
    );
}

#[test]
fn operational_conditional_metadata_and_search_extract_lifecycle() {
    let dir = fixture("operational-readiness");
    edit(
        dir.path(),
        "docs/service.md",
        "oncall: identity-primary\n",
        "",
    );
    assert!(!check(dir.path()).valid());
    edit(
        dir.path(),
        "docs/service.md",
        "lifecycle: production",
        "lifecycle: development",
    );
    refresh(dir.path());
    let options = index::SearchOptions {
        text: "database connection timeout".into(),
        document_type: Some("runbook".into()),
        section: Some("Diagnosis".into()),
        limit: None,
    };
    let hits = index::search(dir.path(), &options).unwrap();
    assert_eq!(hits["results"].as_array().unwrap().len(), 1);
    assert_eq!(hits["results"][0]["documentId"], "RUN-001");
    let section = hits["results"][0]["sectionId"].as_str().unwrap();
    assert!(
        index::extract(dir.path(), section).unwrap()["markdown"]
            .as_str()
            .unwrap()
            .starts_with("# Diagnosis")
    );
    edit(
        dir.path(),
        "docs/runbook.md",
        "database connection timeout",
        "connection pool exhaustion",
    );
    refresh(dir.path());
    assert_eq!(
        index::search(dir.path(), &options).unwrap()["results"],
        json!([])
    );
}

#[test]
fn architecture_graph_traverses_dependencies_in_both_directions() {
    let dir = fixture("architecture-governance");
    refresh(dir.path());
    for (id, direction) in [("COMP-001", "incoming"), ("COMP-002", "outgoing")] {
        let graph = index::graph(dir.path(), id, direction, 1).unwrap();
        assert!(
            graph["edges"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "frontMatter.dependsOn")
        );
    }
}

#[test]
fn design_invariant_breakages_map_to_named_constraint_rules() {
    for (file, from, to, expected) in [
        (
            "docs/pattern-import.md",
            "states: [loading, success]",
            "states: [success]",
            "urn:docvera:policy:design:async-pattern-declares-progress-state",
        ),
        (
            "docs/pattern-import.md",
            "failureScenarios: [FAIL-1]\n",
            "",
            "urn:docvera:policy:design:failure-capable-pattern-has-failure-scenario",
        ),
        (
            "docs/finding-import-implementation.md",
            "status: open",
            "status: resolved",
            "urn:docvera:policy:design:stable-pattern-implemented-or-tracked",
        ),
        (
            "docs/principle-simplicity.md",
            "governs: [RULE-2]",
            "governs: []",
            "urn:docvera:policy:design:stable-principle-governs-a-rule",
        ),
        (
            "docs/principle-legacy-navigation.md",
            "lifecycle: deprecated",
            "lifecycle: stable",
            "urn:docvera:policy:design:stable-principle-governs-a-rule",
        ),
        (
            "docs/rule-plain-words.md",
            "governedBy: [PRIN-2]",
            "governedBy: [PRIN-3]",
            "urn:docvera:policy:design:deprecated-principle-unreferenced",
        ),
    ] {
        let dir = fixture("design-invariants");
        edit(dir.path(), file, from, to);
        let result = check(dir.path());
        assert!(!result.valid(), "{file}: {from}");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.rule.starts_with("shacl.") && d.message.contains(expected)),
            "{file}: {from} => {expected}, got {:?}",
            result.diagnostics
        );
        // The reciprocal governance rule fires when RULE-2 loses its governor.
        if from == "governs: [RULE-2]" {
            assert!(result.diagnostics.iter().any(|d| {
                d.message
                    .contains("urn:docvera:policy:design:stable-rule-governed-by-active-principle")
            }));
        }
    }
}
