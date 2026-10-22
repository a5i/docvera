//! JSON Schema contracts generated from the same types used by Serde.
//! Input schemas describe deserialization; result schemas describe serialization.
use crate::{
    config::{CONFIG_VERSION, Config},
    model::{CONTRACT_VERSION, CheckResult, Diagnostic},
    schema::{SCHEMA_VERSION, Schema as DocumentSchema},
};
use schemars::{JsonSchema, Schema, generate::SchemaSettings};
use std::collections::BTreeMap;

fn generate<T: JsonSchema>(name: &str, version: u32, output: bool) -> Schema {
    let settings = SchemaSettings::draft2020_12();
    let settings = if output {
        settings.for_serialize()
    } else {
        settings.for_deserialize()
    };
    let mut schema = settings.into_generator().into_root_schema_for::<T>();
    schema.insert(
        "$id".into(),
        format!("urn:docvera:schema:{name}:v{version}").into(),
    );
    schema
}

/// Structural contract for docvera.toml (or legacy docvera.yaml), after parsing to JSON data.
/// Filesystem/glob checks and other semantic rules remain in Rust.
pub fn config_schema() -> Schema {
    generate::<Config>("config", CONFIG_VERSION, false)
}

/// Document-schema envelope, not the JSON Schema of a Markdown document's metadata.
pub fn document_schema() -> Schema {
    generate::<DocumentSchema>("document", SCHEMA_VERSION, false)
}

pub fn check_result_schema() -> Schema {
    generate::<CheckResult>("check-result", CONTRACT_VERSION, true)
}
pub fn diagnostic_schema() -> Schema {
    generate::<Diagnostic>("diagnostic", CONTRACT_VERSION, true)
}

/// Only contracts backed by Rust types are included; dynamic SQL/retrieval results
/// and CLI command descriptions are not inferred from example values.
pub fn json_schemas() -> BTreeMap<&'static str, Schema> {
    BTreeMap::from([
        ("config", config_schema()),
        ("document", document_schema()),
        ("check-result", check_result_schema()),
        ("diagnostic", diagnostic_schema()),
    ])
}
