use docvera_engine::{CheckerContext, Workspace, WorkspaceSources, config::Config, parser};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const CONFIG: &str = "version = 1\ninclude = ['docs/*.md']\nexclude = ['docs/excluded.md']\n[schemas]\n'docs/n*.md' = 'schemas/note.yaml'\n";
const SCHEMA: &str = "schemaVersion: 1\ndocumentType: note\nidentity: {field: frontMatter.id, uniqueWithin: workspace}\nbody: {allowPreamble: true, additionalSections: true}\nrelations:\n  - {from: frontMatter.related, targetType: note}\n";
const NOTE: &str = "---\nid: N-1\n---\n# Привет 🦀\n\n[asset](asset.txt)\n";
const OTHER: &str = "---\nid: N-2\nrelated: N-1\n---\n# Other\n";
fn config(text: &str) -> Config {
    toml::from_str(text).unwrap()
}
fn sources() -> WorkspaceSources {
    WorkspaceSources {
        documents: BTreeMap::from([
            ("docs/n1.md".into(), NOTE.into()),
            ("docs/n2.md".into(), OTHER.into()),
        ]),
        schemas: BTreeMap::from([("schemas/note.yaml".into(), SCHEMA.into())]),
        assets: ["docs/asset.txt".into()].into(),
        ..Default::default()
    }
}
fn json_result(result: &docvera_engine::model::CheckResult) -> Value {
    serde_json::to_value(result).unwrap()
}
fn put(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn disk_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "docvera.toml", CONFIG);
    let sources = sources();
    for (path, text) in sources.documents.into_iter().chain(sources.schemas) {
        put(dir.path(), &path, &text);
    }
    put(dir.path(), "docs/asset.txt", "asset");
    dir
}

#[test]
fn in_memory_and_disk_checks_are_identical_and_checks_need_no_files() {
    let dir = disk_fixture();
    let context = CheckerContext::load(dir.path()).unwrap();
    let memory = CheckerContext::from_sources(config(CONFIG), sources()).unwrap();
    let baseline = Workspace::load(dir.path()).unwrap().check();
    assert!(baseline.valid());
    assert_eq!(
        json_result(&baseline),
        json_result(&memory.check().unwrap())
    );
    let edited = NOTE
        .replace("N-1", "N-2")
        .replace("asset.txt", "missing.txt");
    put(dir.path(), "docs/n1.md", &edited);
    let expected = Workspace::load(dir.path()).unwrap().check();
    assert_eq!(
        json_result(&expected),
        json_result(&context.check_with_text("docs/n1.md", &edited).unwrap())
    );
    dir.close().unwrap(); // Disk, schema and config are gone; the cached context is still usable.
    assert_eq!(
        json_result(&expected),
        json_result(&context.check_with_text("docs/n1.md", &edited).unwrap())
    );
    let one = context.check_document("docs/n1.md", &edited).unwrap();
    assert_eq!(one.documents, 1);
    assert_eq!(
        one.diagnostics.len(),
        expected
            .diagnostics
            .iter()
            .filter(|d| d.path == "docs/n1.md")
            .count()
    );
    assert!(
        context.check().unwrap().valid(),
        "overlay must not mutate baseline"
    );
}

#[test]
fn replacement_removes_old_identity_and_updates_inbound_references() {
    let mut context = CheckerContext::from_sources(config(CONFIG), sources()).unwrap();
    let changed = NOTE.replace("N-1", "N-3");
    let result = context.check_with_text("docs/n1.md", &changed).unwrap();
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.path == "docs/n2.md" && d.rule == "relation.target")
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.rule == "identity.duplicate")
    );
    context.update_document("docs/n1.md", changed).unwrap();
    assert!(!context.check().unwrap().valid());
    context
        .update_document("docs/n2.md", OTHER.replace("related: N-1", "related: N-3"))
        .unwrap();
    assert!(context.check().unwrap().valid());
    context.remove_document("docs/n1.md").unwrap();
    assert!(!context.check().unwrap().valid());
}

#[test]
fn new_unsaved_paths_normalization_and_excludes() {
    let context = CheckerContext::from_sources(config(CONFIG), sources()).unwrap();
    assert!(
        context
            .check_document("docs/./n3.md", "---\nid: N-3\n---\n# New\n")
            .unwrap()
            .valid()
    );
    let duplicate = context.check_document("docs/n3.md", NOTE).unwrap();
    assert!(
        duplicate
            .diagnostics
            .iter()
            .any(|d| d.rule == "identity.duplicate")
    );
    assert_eq!(
        context
            .check_document("docs/excluded.md", "bad")
            .unwrap()
            .documents,
        0
    );
    for path in ["../n.md", "/n.md", "C:\\n.md", "docs/../../n.md", ""] {
        assert!(context.check_document(path, NOTE).is_err(), "{path}");
    }
}

#[test]
fn untyped_skip_is_opt_in_and_does_not_hide_explicit_schema_errors() {
    let strict = CheckerContext::from_sources(config(CONFIG), sources()).unwrap();
    assert!(
        strict
            .check_document("docs/free.md", "[bad](missing.md)")
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.rule == "schema.resolve")
    );
    let cfg = format!("{CONFIG}\n[validation]\nuntyped = 'skip'\n");
    let context = CheckerContext::from_sources(config(&cfg), sources()).unwrap();
    let skipped = context
        .check_document("docs/free.md", "---\n[broken YAML\n---\n[bad](missing.md)")
        .unwrap();
    assert_eq!(skipped.documents, 1);
    assert!(skipped.diagnostics.is_empty());
    for text in [
        "---\n$schema: missing.yaml\n---\n",
        "---\n$schema: 7\n---\n",
        "---\n$schema: /schemas/note.yaml\n---\n",
    ] {
        assert!(
            context
                .check_document("docs/free.md", text)
                .unwrap()
                .diagnostics
                .iter()
                .any(|d| d.rule == "schema.resolve")
        );
    }
    let ambiguous = cfg.replace("[schemas]", "[schemas]\n'docs/*.md' = 'schemas/note.yaml'");
    let context = CheckerContext::from_sources(config(&ambiguous), sources()).unwrap();
    assert!(
        context
            .check_document("docs/n1.md", NOTE)
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.rule == "schema.resolve")
    );
}

#[test]
fn explicit_selector_uses_document_directory_and_overrides_mapping() {
    let context = CheckerContext::from_sources(config(CONFIG), sources()).unwrap();
    assert!(
        context
            .check_document(
                "docs/free.md",
                &NOTE.replace("id: N-1", "$schema: ../schemas/note.yaml\nid: N-3")
            )
            .unwrap()
            .valid()
    );
    assert!(
        context
            .check_document(
                "docs/n1.md",
                &NOTE.replace("id: N-1", "$schema: ../../escape.yaml\nid: N-1")
            )
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.rule == "schema.resolve")
    );
}

#[test]
fn front_matter_formats_bom_crlf_offsets_and_source_bytes() {
    for (open, metadata, close) in [
        ("---", "id: N-1\r\ntitle: Привет 🦀", "---"),
        ("---", "id: N-1\r\ntitle: Привет 🦀", "..."),
        ("+++", "id = 'N-1'\r\ntitle = 'Привет 🦀'", "+++"),
    ] {
        let source = format!("\u{feff}{open}\r\n{metadata}\r\n{close}\r\n# Привет 🦀\r\n");
        let doc = parser::parse("docs/n1.md".into(), source.clone());
        assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
        assert_eq!(doc.metadata["title"], "Привет 🦀");
        assert_eq!(doc.source, source);
        assert_eq!(
            doc.hash,
            blake3::hash(source.as_bytes()).to_hex().to_string()
        );
        assert_eq!(&source[doc.body_start..], "# Привет 🦀\r\n");
        let heading = &doc.sections[0];
        assert_eq!(heading.heading, "Привет 🦀");
        assert_eq!(heading.span.start, doc.body_start);
        assert_eq!(heading.span.line, 5);
    }
    let source = "\u{feff}# First\n";
    let doc = parser::parse("n.md".into(), source.into());
    assert_eq!(doc.sections[0].heading, "First");
    assert_eq!(doc.sections[0].span.start, 3);
}

#[test]
fn toml_nested_values_and_array_items_keep_parser_ranges() {
    let source = "\u{feff}+++\r\nid = 'N-1'\r\nrelated = ['N-2', 'MISSING']\r\n[review]\r\nowner = 'Лёша 🦀'\r\nstates = ['draft', 'planned']\r\n+++\r\n# Note\r\n";
    let doc = parser::parse("docs/n1.md".into(), source.into());
    for (pointer, expected) in [
        ("/id", "'N-1'"),
        ("/related/1", "'MISSING'"),
        ("/review/owner", "'Лёша 🦀'"),
        ("/review/states/1", "'planned'"),
    ] {
        let span = doc
            .metadata_spans
            .get(pointer)
            .unwrap_or_else(|| panic!("{pointer}: {:?}", doc.metadata_spans));
        assert_eq!(&source[span.start..span.end], expected);
    }
    let context = CheckerContext::from_sources(config(CONFIG), sources()).unwrap();
    let result = context.check_document("docs/n1.md", source).unwrap();
    let error = result
        .diagnostics
        .iter()
        .find(|d| d.rule == "relation.target")
        .unwrap();
    assert_eq!(&source[error.span.start..error.span.end], "'MISSING'");
}

#[test]
fn disabled_or_noninitial_metadata_is_markdown_and_unclosed_enabled_blocks_are_diagnostics() {
    let cfg = config(&format!(
        "{CONFIG}\n[frontMatter]\nyaml = false\ntoml = false\n"
    ));
    for source in ["---\nid: X\n---\n# Note\n", "+++\nid = 'X'\n+++\n# Note\n"] {
        let doc = parser::parse_with_options(
            "n.md".into(),
            source.into(),
            &cfg.parser,
            &cfg.front_matter,
        );
        assert_eq!(doc.metadata, json!({}));
        assert_eq!(doc.body_start, 0);
        assert!(doc.diagnostics.is_empty());
    }
    for source in [
        "\n---\nid: X\n---\n",
        "text\n+++\nid = 'X'\n+++\n",
        " ---\nid: X\n---\n",
    ] {
        assert_eq!(
            parser::parse("n.md".into(), source.into()).metadata,
            json!({})
        );
    }
    for (source, rule) in [
        ("---\nid: X\n", "yaml.unclosed"),
        ("+++\nid = 'X'\n", "toml.unclosed"),
        ("+++\nid = [\n+++\n", "toml.parse"),
    ] {
        assert!(
            parser::parse("n.md".into(), source.into())
                .diagnostics
                .iter()
                .any(|d| d.rule == rule)
        );
    }
}

#[test]
fn parser_flags_are_configured_and_metadata_flags_cannot_be_enabled() {
    let source = "# ~~Strike~~ [[Target]] $x$\n\n| A |\n|---|\n| B |\n";
    let cfg = config(&format!(
        "{CONFIG}\n[parser]\ntables = false\nstrikethrough = false\nwikilinks = true\nmath = true\nfootnotes = true\ngfm = true\n"
    ));
    let doc =
        parser::parse_with_options("n.md".into(), source.into(), &cfg.parser, &cfg.front_matter);
    assert!(!doc.nodes.iter().any(|n| n.kind == "table"));
    assert!(doc.sections[0].heading.contains("~~Strike~~"));
    assert!(
        doc.sections[0].heading.ends_with(" x"),
        "inline math must contribute heading text"
    );
    assert!(doc.links.iter().any(|l| l.target == "Target"));
    let default = parser::parse("n.md".into(), source.into());
    assert!(default.nodes.iter().any(|n| n.kind == "table"));
    assert!(!default.sections[0].heading.contains("~~"));
    for key in [
        "yamlStyleMetadataBlocks",
        "unknown",
        "plusesDelimitedMetadataBlocks",
    ] {
        assert!(toml::from_str::<Config>(&format!("{CONFIG}\n[parser]\n{key} = true\n")).is_err());
    }
}

#[cfg(not(feature = "shacl"))]
#[test]
fn disabled_shacl_fails_closed_instead_of_silently_skipping_policies() {
    let cfg = format!("{CONFIG}\n[validation.shacl]\nshapes = ['policy.ttl']\n");
    let error = CheckerContext::from_sources(config(&cfg), sources())
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("no shacl feature"), "{error}");
}

#[cfg(feature = "shacl")]
#[test]
fn shacl_checks_use_unsaved_graph_and_match_disk_results() {
    let cfg = format!("{CONFIG}\n[validation.shacl]\nshapes = ['policy.ttl']\n");
    let shapes = "@prefix sh: <http://www.w3.org/ns/shacl#> . <urn:shape> a sh:NodeShape; sh:targetClass <urn:docvera:v1:type:note>; sh:property [sh:path <urn:docvera:v1:field:status>; sh:hasValue 'ready'] .";
    let dir = disk_fixture();
    put(dir.path(), "docvera.toml", &cfg);
    put(dir.path(), "policy.ttl", shapes);
    let mut input = sources();
    input.shapes.insert("policy.ttl".into(), shapes.into());
    let context = CheckerContext::from_sources(config(&cfg), input).unwrap();
    let edited = NOTE.replace("id: N-1", "id: N-1\nstatus: ready");
    put(dir.path(), "docs/n1.md", &edited);
    let expected = Workspace::load(dir.path()).unwrap().check();
    assert_eq!(
        json_result(&expected),
        json_result(&context.check_with_text("docs/n1.md", &edited).unwrap())
    );
    assert!(!context.check_document("docs/n1.md", NOTE).unwrap().valid());
    assert!(
        context
            .check_document("docs/n1.md", &edited)
            .unwrap()
            .valid()
    );
}

#[test]
fn toml_dates_are_strings_nonfinite_numbers_are_errors_and_duplicate_keys_fail() {
    let source = "+++\nid = 'N-1'\ndate = 2026-10-21\nwhen = 2026-10-21T12:34:56Z\ncount = 3\nratio = 1.25\n+++\n# Note\n";
    let doc = parser::parse("n.md".into(), source.into());
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    assert_eq!(doc.metadata["date"], "2026-10-21");
    assert_eq!(doc.metadata["when"], "2026-10-21T12:34:56Z");
    assert_eq!(doc.metadata["count"], 3);
    assert_eq!(doc.metadata["ratio"], 1.25);
    let span = &doc.metadata_spans["/id"];
    assert_eq!(&source[span.start..span.end], "'N-1'");
    for text in ["x = nan", "x = inf", "x = -inf", "id = 'A'\nid = 'B'"] {
        let source = format!("+++\n{text}\n+++\n");
        assert!(
            parser::parse("n.md".into(), source)
                .diagnostics
                .iter()
                .any(|d| d.rule == "toml.parse")
        );
    }
}

#[test]
fn duplicate_normalized_source_paths_are_rejected() {
    let mut input = sources();
    input.documents.insert("docs/./n1.md".into(), NOTE.into());
    assert!(CheckerContext::from_sources(config(CONFIG), input).is_err());
    let mut input = sources();
    input
        .schemas
        .insert("schemas/./note.yaml".into(), SCHEMA.into());
    assert!(CheckerContext::from_sources(config(CONFIG), input).is_err());
}

#[test]
fn schema_updates_revalidate_cached_documents_and_enable_new_unsaved_selectors() {
    let mut context = CheckerContext::from_sources(config(CONFIG), sources()).unwrap();
    context
        .update_schema(
            "schemas/note.yaml",
            "schemaVersion: 999\ndocumentType: bad".into(),
        )
        .unwrap();
    assert!(
        context
            .check()
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.rule == "schema.resolve")
    );
    context
        .update_schema("schemas/note.yaml", SCHEMA.into())
        .unwrap();
    assert!(context.check().unwrap().valid());
    let text = NOTE.replace("id: N-1", "$schema: ../schemas/new.yaml\nid: N-3");
    assert!(!context.check_document("docs/n3.md", &text).unwrap().valid());
    context
        .update_schema("schemas/new.yaml", SCHEMA.into())
        .unwrap();
    assert!(context.check_document("docs/n3.md", &text).unwrap().valid());
    assert!(
        context
            .update_schema("../escape.yaml", SCHEMA.into())
            .is_err()
    );
}
