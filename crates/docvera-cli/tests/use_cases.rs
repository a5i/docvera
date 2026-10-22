#[path = "../../../tests/support/mod.rs"]
mod support;

use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
use support::{SCENARIOS, edit, fixture, reports};

fn run(root: &Path, args: &[&str], code: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_docvera"))
        .arg("--root")
        .arg(root)
        .args(["--format", "json"])
        .args(args)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(code), "{:?}", output);
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["contractVersion"], 1);
    value
}

#[test]
fn all_use_cases_check_query_and_fail_with_validation_exit_code() {
    for &(name, _count) in SCENARIOS {
        let dir = fixture(name);
        let root = dir.path();
        assert_eq!(run(root, &["check"], 0)["diagnostics"], json!([]));
        for path in reports(root) {
            let sql = fs::read_to_string(&path).unwrap();
            let result = run(root, &["query", "sql", &sql], 0);
            let expected = if path.file_stem().unwrap() == "coverage" {
                json!([
                    {"requirement":"REQ-001","tests":1,"latest_result":"passed"},
                    {"requirement":"REQ-002","tests":0,"latest_result":null}
                ])
            } else {
                json!([])
            };
            assert_eq!(result["rows"], expected, "{name}: {}", path.display());
        }
        let file = match name {
            "product-traceability" => "docs/feature.md",
            "qa-traceability" => "docs/requirement.md",
            "operational-readiness" => "docs/service.md",
            "architecture-governance" => "docs/component.md",
            "design-invariants" => "docs/pattern-import.md",
            _ => "docs/epic.md",
        };
        edit(root, file, "owner: identity-team\n", "");
        assert!(
            !run(root, &["check"], 1)["diagnostics"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn operational_extract_and_architecture_graph_commands() {
    let dir = fixture("operational-readiness");
    let sections = run(
        dir.path(),
        &[
            "query",
            "sql",
            "SELECT s.section_id FROM sections s JOIN documents d ON d.path = s.document_id WHERE d.declared_id = 'RUN-001' AND s.heading = 'Diagnosis'",
        ],
        0,
    );
    let section = sections["rows"][0]["section_id"].as_str().unwrap();
    assert!(
        run(dir.path(), &["extract", section], 0)["markdown"]
            .as_str()
            .unwrap()
            .starts_with("# Diagnosis")
    );
    let dir = fixture("architecture-governance");
    for (direction, id) in [("incoming", "COMP-001"), ("outgoing", "COMP-002")] {
        let graph = run(dir.path(), &["graph", direction, id, "--depth", "1"], 0);
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
fn shacl_violations_and_load_failures_have_distinct_exit_codes() {
    let dir = fixture("design-invariants");
    edit(
        dir.path(),
        "docs/pattern-import.md",
        "states: [loading, success]",
        "states: [success]",
    );
    let result = run(dir.path(), &["check"], 1);
    assert!(result["diagnostics"].as_array().unwrap().iter().any(|d| {
        d["rule"].as_str().unwrap().starts_with("shacl.")
            && d["message"]
                .as_str()
                .unwrap()
                .contains("async-pattern-declares-progress-state")
    }));
    fs::write(
        dir.path().join("policies/design-invariants.ttl"),
        "not Turtle",
    )
    .unwrap();
    assert!(
        run(dir.path(), &["check"], 2)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("SHACL")
    );
}

#[test]
fn warnings_as_errors_does_not_promote_shacl_info() {
    let dir = fixture("design-invariants");
    let policy = "@prefix sh: <http://www.w3.org/ns/shacl#> . <urn:p> a sh:NodeShape ; sh:targetClass <urn:docvera:v1:type:interaction-pattern> ; sh:property [ sh:path <urn:missing> ; sh:minCount 1 ; sh:severity sh:Info ] .";
    fs::write(dir.path().join("policies/design-invariants.ttl"), policy).unwrap();
    let result = run(dir.path(), &["check", "--warnings-as-errors"], 0);
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["severity"] == "info")
    );
    fs::write(
        dir.path().join("policies/design-invariants.ttl"),
        policy.replace("sh:Info", "sh:Warning"),
    )
    .unwrap();
    run(dir.path(), &["check"], 0);
    run(dir.path(), &["check", "--warnings-as-errors"], 1);
}

#[test]
fn jsonl_keeps_one_physical_line_and_parity_for_structured_shacl_output() {
    let dir = fixture("design-invariants");
    // A Turtle triple-quoted literal puts a real newline inside sh:message;
    // jsonl must stay a single physical line through proper JSON escaping.
    fs::write(
        dir.path().join("policies/design-invariants.ttl"),
        "@prefix sh: <http://www.w3.org/ns/shacl#> . <urn:p> a sh:NodeShape ; sh:targetClass <urn:docvera:v1:type:interaction-pattern> ; sh:sparql [ sh:message \"\"\"line one\nline two\"\"\" ; sh:select \"SELECT $this WHERE {}\" ] .",
    )
    .unwrap();
    let output = |format: &str| {
        let out = Command::new(env!("CARGO_BIN_EXE_docvera"))
            .arg("--root")
            .arg(dir.path())
            .args(["--format", format, "check"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stderr.is_empty());
        out.stdout
    };
    let jsonl = String::from_utf8(output("jsonl")).unwrap();
    assert_eq!(jsonl.lines().count(), 1, "{jsonl}");
    let line: Value = serde_json::from_str(jsonl.trim_end()).unwrap();
    assert_eq!(line["contractVersion"], 1);
    let diagnostics = line["diagnostics"].as_array().unwrap();
    assert_eq!(
        diagnostics.len(),
        2,
        "only interaction-patterns are targeted"
    );
    for diagnostic in diagnostics {
        assert_eq!(
            diagnostic["shacl"]["messages"][0]["value"],
            "line one\nline two"
        );
        assert!(diagnostic["shacl"]["sourceField"].is_null());
        assert_eq!(
            diagnostic["shacl"]["constraintComponent"],
            "http://www.w3.org/ns/shacl#SPARQLConstraintComponent"
        );
    }
    let json: Value = serde_json::from_slice(&output("json")).unwrap();
    assert_eq!(json, line, "jsonl and json must carry identical payloads");
}
