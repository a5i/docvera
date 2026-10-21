use docvera_engine::contracts;
use serde_json::Value;
use std::process::Command;

#[test]
fn schema_command_is_offline_deterministic_and_emits_bare_schemas_in_every_format() {
    let temp = tempfile::tempdir().unwrap();
    let absent = temp.path().join("does-not-exist");
    for (name, schema) in contracts::json_schemas() {
        let expected = serde_json::to_value(schema).unwrap();
        for format in [None, Some("human"), Some("json"), Some("jsonl")] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_docvera"));
            command.arg("--root").arg(&absent);
            if let Some(format) = format {
                command.args(["--format", format]);
            }
            let output = command.args(["schema", name]).output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            let schema: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(schema, expected, "{name}: {format:?}");
            assert!(schema.get("contractVersion").is_none());
            if format == Some("jsonl") {
                assert_eq!(output.stdout.iter().filter(|b| **b == b'\n').count(), 1);
            }
        }
    }
    assert!(
        !absent.exists(),
        "schema generation must not create a workspace"
    );
}

#[test]
fn contract_and_guide_embed_exactly_the_generated_schemas() {
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_docvera"))
        .current_dir(temp.path())
        .args(["contract", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let descriptor: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(descriptor["contractVersion"], 1);
    assert_eq!(
        descriptor["results"]["check"],
        descriptor["jsonSchemas"]["check-result"]["required"]
    );
    assert_eq!(
        descriptor["results"]["index"],
        descriptor["jsonSchemas"]["index-result"]["required"]
    );
    assert_eq!(
        descriptor["diagnostic"],
        descriptor["jsonSchemas"]["diagnostic"]["required"]
    );
    assert_eq!(
        descriptor["jsonSchemas"],
        serde_json::to_value(contracts::json_schemas()).unwrap()
    );
    let guide = Command::new(env!("CARGO_BIN_EXE_docvera"))
        .current_dir(temp.path())
        .args(["guide", "--format", "json"])
        .output()
        .unwrap();
    assert!(guide.status.success());
    let guide: Value = serde_json::from_slice(&guide.stdout).unwrap();
    assert_eq!(guide["contract"], descriptor);
}

#[test]
fn schema_help_and_invalid_names_use_standard_clap_behavior() {
    let help = Command::new(env!("CARGO_BIN_EXE_docvera"))
        .args(["schema", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    for name in contracts::json_schemas().keys() {
        assert!(text.contains(name), "{name}");
    }
    for args in [vec!["schema"], vec!["schema", "unknown"]] {
        let invalid = Command::new(env!("CARGO_BIN_EXE_docvera"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(invalid.status.code(), Some(2));
        assert!(invalid.stdout.is_empty());
        assert!(!invalid.stderr.is_empty());
    }
}
