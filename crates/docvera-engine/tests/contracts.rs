use docvera_engine::{Workspace, config::Config, contracts, model::*, schema::Schema};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn value(schema: schemars::Schema) -> Value {
    serde_json::to_value(schema).unwrap()
}
fn validator(schema: schemars::Schema) -> jsonschema::Validator {
    let schema = value(schema);
    jsonschema::meta::validate(&schema).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

#[test]
fn generated_schemas_are_valid_deterministic_and_feature_aware() {
    let schemas = contracts::json_schemas();
    assert_eq!(
        schemas.contains_key("index-result"),
        cfg!(feature = "index")
    );
    for (name, schema) in schemas {
        let data = value(schema);
        assert_eq!(
            data["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(data["$id"], format!("urn:docvera:schema:{name}:v1"));
        jsonschema::meta::validate(&data).unwrap();
        jsonschema::validator_for(&data).unwrap();
        assert_eq!(data, value(contracts::json_schemas().remove(name).unwrap()));
    }
}

#[test]
fn config_schema_tracks_serde_names_required_fields_defaults_and_unknown_fields() {
    let schema = value(contracts::config_schema());
    let validate = validator(contracts::config_schema());
    let minimal = json!({"version":1,"include":["docs/*.md"],"schemas":{}});
    assert!(validate.is_valid(&minimal));
    let config: Config = serde_json::from_value(minimal.clone()).unwrap();
    config.validate().unwrap();
    assert_eq!(schema["properties"]["exclude"]["default"], json!([]));
    assert_eq!(
        schema["$defs"]["ParserOptions"]["properties"]["taskLists"]["default"],
        true
    );
    assert_eq!(
        schema["$defs"]["FrontMatterOptions"]["properties"]["yaml"]["default"],
        true
    );
    assert_eq!(
        schema["$defs"]["Validation"]["properties"]["untyped"]["default"],
        "error"
    );
    assert_eq!(
        schema["properties"]["version"]["const"],
        docvera_engine::config::CONFIG_VERSION
    );
    let mut all = minimal.clone();
    all["parser"] = json!({"tables":false,"taskLists":false,"wikilinks":true});
    all["frontMatter"] = json!({"yaml":true,"toml":false});
    all["validation"] = json!({"untyped":"skip","shacl":{"shapes":["policy.ttl"]}});
    assert!(validate.is_valid(&all));
    serde_json::from_value::<Config>(all)
        .unwrap()
        .validate()
        .unwrap();
    for (key, bad) in [
        ("unknown", json!(true)),
        ("parser", json!({"task_lists":true})),
        ("frontMatter", json!({"unknown":true})),
        ("validation", json!({"untyped":"ignore"})),
        ("schemas", json!([])),
        ("include", json!("*.md")),
    ] {
        let mut candidate = minimal.clone();
        candidate[key] = bad;
        assert!(!validate.is_valid(&candidate), "{candidate}");
        assert!(serde_json::from_value::<Config>(candidate).is_err());
    }
    for field in ["version", "include", "schemas"] {
        let mut candidate = minimal.clone();
        candidate.as_object_mut().unwrap().remove(field);
        assert!(!validate.is_valid(&candidate));
        assert!(serde_json::from_value::<Config>(candidate).is_err());
    }
    let mut wrong_version = minimal;
    wrong_version["version"] = json!(2);
    assert!(!validate.is_valid(&wrong_version));
    assert!(
        serde_json::from_value::<Config>(wrong_version)
            .unwrap()
            .validate()
            .is_err()
    );
}

#[test]
fn document_schema_covers_recursive_dsl_and_does_not_confuse_embedded_front_matter_schema() {
    let validate = validator(contracts::document_schema());
    let minimal = json!({"schemaVersion":1,"documentType":"note"});
    assert!(validate.is_valid(&minimal));
    serde_json::from_value::<Schema>(minimal.clone())
        .unwrap()
        .check_definition()
        .unwrap();
    let document = json!({
        "schemaVersion":1,"documentType":"note",
        "frontMatter":{"type":"object","$defs":{"id":{"type":"string"}},"properties":{"id":{"$ref":"#/$defs/id"}}},
        "body":{"sections":[{"title":"Note","min":0,"children":{"sections":[{"pattern":"^Child$","content":[{"kind":"code","language":"rust"}]}]}}]},
        "identity":{"field":"frontMatter.id","uniqueWithin":"workspace"},
        "relations":[{"from":"frontMatter.related","targetTypes":["note","task"]}],
        "lint":{"noSkippedHeadings":true}
    });
    assert!(validate.is_valid(&document));
    serde_json::from_value::<Schema>(document)
        .unwrap()
        .check_definition()
        .unwrap();
    for (field, bad) in [
        ("schemaVersion", json!(2)),
        ("body", json!({"sections":[{"title":"Note","typo":true}]})),
        ("lint", json!({"unknown":true})),
        ("unknown", json!(true)),
    ] {
        let mut candidate = minimal.clone();
        candidate[field] = bad;
        assert!(!validate.is_valid(&candidate), "{candidate}");
    }
    // Arbitrary Rust regex validation and cross-field invariants are not inferred
    // from custom Rust code; generated structural schemas do not replace it.
    let regex =
        json!({"schemaVersion":1,"documentType":"note","body":{"sections":[{"pattern":"["}]}});
    assert!(validate.is_valid(&regex));
    assert!(
        serde_json::from_value::<Schema>(regex)
            .unwrap()
            .check_definition()
            .is_err()
    );
}

#[test]
fn every_example_config_and_document_schema_matches_its_generated_contract() {
    let configs = validator(contracts::config_schema());
    let documents = validator(contracts::document_schema());
    // Examples live in two places: the crate-owned assets/workspace embedded by
    // `init`, and the multi-root use-case workspaces under <repo>/examples.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let roots = [manifest.join("assets"), manifest.join("../../examples")];
    let mut counts = (0, 0);
    for examples in roots {
        for entry in walkdir::WalkDir::new(examples) {
            let entry = entry.unwrap();
            let path = entry.path();
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.file_name() == "docvera.toml" {
                let data: Value = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
                assert!(configs.is_valid(&data), "{}", path.display());
                counts.0 += 1;
            } else if path
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|n| n == "schemas")
                && path.extension().is_some_and(|e| e == "yaml" || e == "json")
            {
                let data: Value =
                    serde_saphyr::from_str(&fs::read_to_string(path).unwrap()).unwrap();
                assert!(documents.is_valid(&data), "{}", path.display());
                counts.1 += 1;
            }
        }
    }
    assert!(counts.0 >= 7 && counts.1 >= 3, "{counts:?}");
}

#[test]
fn output_schemas_validate_actual_results_and_structured_shacl_variants() {
    let checks = validator(contracts::check_result_schema());
    let diagnostics = validator(contracts::diagnostic_schema());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/workspace");
    let actual = serde_json::to_value(Workspace::load(&root).unwrap().check()).unwrap();
    assert!(checks.is_valid(&actual));
    let mut diagnostic = json!({"path":"note.md","span":{"start":0,"end":1,"line":1,"column":1,"endLine":1,"endColumn":2},"rule":"shacl.Constraint","severity":"error","message":"Invalid"});
    assert!(diagnostics.is_valid(&diagnostic));
    diagnostic["shacl"] = json!({
        "focusNode":{"kind":"iri","value":"urn:note"},"constraintComponent":"urn:constraint","resultSeverity":"urn:Violation",
        "sourceShape":{"kind":"blankNode"},
        "resultPath":{"kind":"sequence","paths":[{"kind":"predicate","iri":"urn:field"},{"kind":"inverse","path":{"kind":"zeroOrMore","path":{"kind":"predicate","iri":"urn:other"}}}]},
        "value":{"kind":"literal","value":"planned","datatype":"http://www.w3.org/2001/XMLSchema#string"},
        "messages":[{"value":"Wrong","language":"en"}],"sourceField":"/status"
    });
    let typed: Diagnostic = serde_json::from_value(diagnostic).unwrap();
    let wire = serde_json::to_value(typed).unwrap();
    assert!(diagnostics.is_valid(&wire));
    let envelope = json!({"contractVersion":1,"documents":1,"diagnostics":[wire.clone()]});
    assert!(checks.is_valid(&envelope));
    let mut incomplete = envelope.clone();
    incomplete.as_object_mut().unwrap().remove("diagnostics");
    assert!(
        !checks.is_valid(&incomplete),
        "output schema must not make serialized fields optional"
    );
    let mut wrong_version = envelope;
    wrong_version["contractVersion"] = json!(2);
    assert!(!checks.is_valid(&wrong_version));
    let mut wrong_tag = wire;
    wrong_tag["shacl"]["focusNode"]["kind"] = json!("url");
    assert!(!diagnostics.is_valid(&wrong_tag));
}

#[test]
fn serde_skipped_provenance_never_leaks_into_generated_document_ir() {
    let schema = schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<Document>();
    let schema = value(schema);
    assert!(schema["properties"].get("metadataSpans").is_none());
    assert!(schema["properties"].get("metadata_spans").is_none());
}

#[cfg(feature = "index")]
#[test]
fn index_result_contract_tracks_required_serialized_fields() {
    let validate = validator(contracts::index_result_schema());
    let result = docvera_engine::index::IndexResult {
        contract_version: CONTRACT_VERSION,
        documents: 3,
        updated: 2,
        deleted: 1,
        diagnostics: 0,
    };
    let mut data = serde_json::to_value(result).unwrap();
    assert!(validate.is_valid(&data));
    data.as_object_mut().unwrap().remove("updated");
    assert!(!validate.is_valid(&data));
}
