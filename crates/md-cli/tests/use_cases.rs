#[path = "../../../tests/support/mod.rs"]
mod support;

use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
use support::{SCENARIOS, edit, fixture, reports};

fn run(root: &Path, args: &[&str], code: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
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
fn all_use_cases_check_index_query_and_fail_with_validation_exit_code() {
    for &(name, count) in SCENARIOS {
        let dir = fixture(name);
        let root = dir.path();
        assert_eq!(run(root, &["check"], 0)["diagnostics"], json!([]));
        assert_eq!(run(root, &["index"], 0)["updated"], count);
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
fn operational_search_extract_and_architecture_graph_commands() {
    let dir = fixture("operational-readiness");
    run(dir.path(), &["index"], 0);
    let hits = run(
        dir.path(),
        &[
            "search",
            "database connection timeout",
            "--type",
            "runbook",
            "--section",
            "Diagnosis",
        ],
        0,
    );
    assert_eq!(hits["results"].as_array().unwrap().len(), 1);
    assert_eq!(hits["results"][0]["documentId"], "RUN-001");
    let section = hits["results"][0]["sectionId"].as_str().unwrap();
    assert!(
        run(dir.path(), &["extract", section], 0)["markdown"]
            .as_str()
            .unwrap()
            .starts_with("# Diagnosis")
    );
    let dir = fixture("architecture-governance");
    run(dir.path(), &["index"], 0);
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
