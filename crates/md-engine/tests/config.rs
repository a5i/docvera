use md_engine::{Workspace, init};
use std::fs;

#[test]
fn init_uses_toml_and_legacy_yaml_remains_a_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let created = init(dir.path()).unwrap();
    assert!(created.iter().any(|path| path == "mdx.toml"));
    assert!(!dir.path().join("mdx.yaml").exists());
    let before = serde_json::to_value(Workspace::load(dir.path()).unwrap().check()).unwrap();
    fs::remove_file(dir.path().join("mdx.toml")).unwrap();
    fs::write(dir.path().join("mdx.yaml"), "version: 1\ninclude: ['docs/**/*.md']\nschemas:\n  'docs/adr-*.md': schemas/adr.yaml\n  'docs/feature-*.md': schemas/feature.yaml\n  'docs/test-*.md': schemas/test-plan.yaml\n").unwrap();
    assert_eq!(
        before,
        serde_json::to_value(Workspace::load(dir.path()).unwrap().check()).unwrap()
    );
    assert!(
        init(dir.path()).is_err(),
        "init must not create conflicting TOML alongside YAML"
    );
    assert!(!dir.path().join("mdx.toml").exists());
    fs::write(dir.path().join("mdx.toml"), "broken TOML").unwrap();
    let error = Workspace::load(dir.path()).err().unwrap().to_string();
    assert!(error.contains("Both mdx.toml and mdx.yaml"));
}

#[test]
fn malformed_unknown_and_unsupported_configs_are_errors() {
    for text in [
        "not TOML",
        "version = 2\ninclude = []\n[schemas]\n",
        "version = 1\ninclude = []\nunknown = true\n[schemas]\n",
        "version = 1\ninclude = []\n[schemas]\n[validation]\nunknown = true\n",
        "version = 1\ninclude = []\n[schemas]\n[validation.shacl]\nshapes = []\n",
        "version = 1\ninclude = []\n[schemas]\n[validation.shacl]\nshapes = [\"a.ttl\"]\nenabled = true\n",
    ] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("mdx.toml"), text).unwrap();
        assert!(Workspace::load(dir.path()).is_err(), "{text}");
    }
    let dir = tempfile::tempdir().unwrap();
    assert!(
        Workspace::load(dir.path())
            .err()
            .unwrap()
            .to_string()
            .contains("Missing mdx.toml")
    );
}

#[test]
fn configs_without_shacl_keep_local_validation_only() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("mdx.toml"),
        "version = 1\ninclude = []\n[schemas]\n",
    )
    .unwrap();
    assert!(Workspace::load(dir.path()).unwrap().check().valid());
    assert!(!dir.path().join(".mdx").exists());
}
