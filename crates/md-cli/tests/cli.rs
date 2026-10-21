use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
fn json(root: &Path, args: &[&str], code: i32) -> Value {
    let mut flags = vec!["--format", "json"];
    flags.extend_from_slice(args);
    let output = run(root, &flags);
    assert_eq!(
        output.status.code(),
        Some(code),
        "stderr: {} stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "JSON operation contaminated stderr"
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["contractVersion"], 1);
    value
}

#[test]
fn cli_full_cycle_and_contract() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert_eq!(
        json(root, &["init"], 0)["created"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    assert_eq!(
        json(root, &["check"], 0)["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(json(root, &["index"], 0)["updated"], 3);
    assert_eq!(json(root, &["index"], 0)["updated"], 0);
    assert_eq!(
        json(
            root,
            &["query", "sql", "SELECT count(*) AS n FROM documents"],
            0
        )["rows"][0]["n"],
        3
    );
    assert_eq!(
        json(
            root,
            &["graph", "incoming", "FEATURE-0042", "--depth", "2"],
            0
        )["nodes"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let hits = json(
        root,
        &[
            "search",
            "OAuth",
            "--type",
            "feature",
            "--section",
            "Security",
        ],
        0,
    );
    assert_eq!(hits["results"].as_array().unwrap().len(), 1);
    let section_id = hits["results"][0]["sectionId"].as_str().unwrap();
    assert!(
        json(root, &["extract", section_id], 0)["markdown"]
            .as_str()
            .unwrap()
            .contains("PKCE")
    );
    assert!(json(root, &["contract"], 0)["commands"]["check"].is_string());
    let human = run(root, &["check"]);
    assert!(String::from_utf8_lossy(&human.stdout).contains("Checked 3 documents; 0 diagnostics"));
    let output = run(root, &["check", "--format", "jsonl"]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 1);
    serde_json::from_slice::<Value>(&output.stdout).unwrap();
}

#[test]
fn validation_failure_has_code_one_and_operational_errors_have_code_two() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert_eq!(
        json(root, &["check"], 2)["error"]["code"],
        "operation_failed"
    );
    json(root, &["init"], 0);
    json(root, &["init"], 2);
    json(root, &["query", "sql", "SELECT 1"], 2);
    let path = root.join("docs/adr-0001.md");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, original.replace("## Context", "## Wrong")).unwrap();
    let result = json(root, &["check"], 1);
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["rule"] == "body.additional")
    );
    let human = run(root, &["check"]);
    assert_eq!(human.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&human.stdout).contains("[body.additional]"));
    // Invalid documents are still inspectable in the index.
    json(root, &["index"], 0);
    let diagnostics = json(root, &["query", "sql", "SELECT rule FROM diagnostics"], 0);
    assert!(!diagnostics["rows"].as_array().unwrap().is_empty());
    json(root, &["query", "sql", "DROP TABLE documents"], 2);
    json(root, &["graph", "outgoing", "MISSING"], 2);
}

#[test]
fn warnings_as_errors_is_an_explicit_gate() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    json(root, &["init"], 0);
    let schema_path = root.join("schemas/adr.yaml");
    let schema = fs::read_to_string(&schema_path).unwrap();
    fs::write(
        schema_path,
        schema.replace(
            "noSkippedHeadings: true",
            "noSkippedHeadings: true\n  noTrailingSpaces: true",
        ),
    )
    .unwrap();
    let path = root.join("docs/adr-0001.md");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(
        path,
        text.replace(
            "Documentation must remain available offline.",
            "Documentation must remain available offline.  ",
        ),
    )
    .unwrap();
    let result = json(root, &["check"], 0);
    assert_eq!(result["diagnostics"][0]["severity"], "warning");
    json(root, &["check", "--warnings-as-errors"], 1);
}

#[test]
fn failed_init_leaves_no_config_and_can_be_retried() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("docs"), "not a directory").unwrap();
    assert_eq!(
        json(root, &["init"], 2)["error"]["code"],
        "operation_failed"
    );
    fs::remove_file(root.join("docs")).unwrap();
    let created = json(root, &["init"], 0)["created"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        created,
        [
            "mdx.toml",
            "schemas/adr.yaml",
            "schemas/feature.yaml",
            "schemas/test-plan.yaml",
            "docs/adr-0001.md",
            "docs/feature-0042.md",
            "docs/test-0001.md",
        ]
    );
}

#[test]
fn guide_is_complete_offline_and_available_in_all_formats() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("nonexistent-workspace");
    let human = run(&root, &["guide"]);
    assert!(human.status.success());
    assert!(human.stderr.is_empty());
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.starts_with("# mdx CLI guide\n"));
    assert!(text.contains(include_str!("../../../docs/schema.md")));
    assert!(text.contains(include_str!("../../../docs/shacl.md")));
    assert!(text.contains("## Workflow for an AI agent"));

    let result = json(&root, &["guide"], 0);
    assert_eq!(result["guide"], text);
    let contract = json(&root, &["contract"], 0);
    assert_eq!(result["contract"], contract);
    for command in contract["commands"].as_object().unwrap().keys() {
        assert!(text.contains(&format!("### {command}\n")), "{command}");
    }
    let compact = run(&root, &["guide", "--format", "jsonl"]);
    assert!(compact.status.success());
    assert!(compact.stderr.is_empty());
    assert_eq!(String::from_utf8_lossy(&compact.stdout).lines().count(), 1);
    assert_eq!(
        serde_json::from_slice::<Value>(&compact.stdout).unwrap(),
        result
    );
    let help = run(&root, &["--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("mdx guide"));
    assert!(help.contains("mdx guide --format json"));
    assert!(!root.exists(), "documentation must not create a workspace");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn guide_sql_and_graph_examples_work_on_initialized_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    json(root, &["init"], 0);
    json(root, &["index"], 0);
    for sql in [
        "SELECT declared_id,type,path FROM documents ORDER BY path",
        "SELECT * FROM sections LIMIT 1",
        "SELECT * FROM nodes LIMIT 1",
        "SELECT * FROM relations LIMIT 10",
        "SELECT document_id,rule,severity,message FROM diagnostics ORDER BY document_id,rule",
        "SELECT document_id,json_extract(shacl,'$.sourceField') AS field FROM diagnostics WHERE shacl IS NOT NULL",
    ] {
        assert!(include_str!("../../../docs/cli-guide.md").contains(sql));
        assert!(json(root, &["query", "sql", sql], 0)["rows"].is_array());
    }
    json(
        root,
        &["graph", "outgoing", "docs/test-0001.md", "--depth", "1"],
        0,
    );
}
