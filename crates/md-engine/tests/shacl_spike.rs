//! Dependency/conformance gate for the pinned Rudof adapter (RFC WP0).
use rudof_rdf::{
    rdf_core::RDFFormat,
    rdf_impl::{OxigraphInMemory, ReaderMode},
};
use shacl::validator::{
    ShaclConfig, ShaclValidationMode,
    processor::{GraphValidation, ShaclProcessor},
    store::{Graph, ShaclDataManager},
};
use std::io::Cursor;

fn validate(data: &str, shapes: &str) -> shacl::validator::report::ValidationReport {
    let graph = OxigraphInMemory::from_reader(
        &mut Cursor::new(data),
        "data",
        &RDFFormat::Turtle,
        None,
        &ReaderMode::default(),
    )
    .unwrap();
    let schema =
        ShaclDataManager::load(&mut Cursor::new(shapes), "shapes", &RDFFormat::Turtle, None)
            .unwrap();
    GraphValidation::new(Graph::try_from(graph).unwrap())
        .validate(
            &schema,
            &ShaclValidationMode::Native,
            &ShaclConfig::default(),
        )
        .unwrap()
}

#[test]
fn native_core_and_sparql_reports_are_observable() {
    let core = r#"@prefix sh: <http://www.w3.org/ns/shacl#> .
        <urn:shape> a sh:NodeShape ; sh:targetNode <urn:feature> ;
            sh:property [ sh:path <urn:epic> ; sh:minCount 1 ] ."#;
    assert!(validate("<urn:feature> <urn:epic> <urn:parent> .", core).conforms());
    let report = validate("", core);
    assert!(!report.conforms());
    assert_eq!(report.results().len(), 1);
    assert!(
        report.results()[0]
            .constraint_component()
            .to_string()
            .contains("MinCount")
    );
    assert!(
        report.results()[0]
            .focus_node()
            .to_string()
            .contains("urn:feature")
    );
    let sparql = r#"@prefix sh: <http://www.w3.org/ns/shacl#> .
        <urn:shape> a sh:NodeShape ; sh:targetNode <urn:epic> ; sh:sparql [
            sh:message "Draft epic has a non-draft feature" ;
            sh:select """SELECT $this WHERE {
                $this <urn:status> "draft" .
                ?feature <urn:epic> $this .
                FILTER NOT EXISTS { ?feature <urn:status> "draft" }
            }""" ] ."#;
    let bad = "<urn:epic> <urn:status> \"draft\" . <urn:feature> <urn:epic> <urn:epic> ; <urn:status> \"planned\" .";
    let report = validate(bad, sparql);
    assert!(
        !report.conforms(),
        "SPARQL must not silently pass: {report:?}"
    );
    assert_eq!(report.results().len(), 1);
    assert!(validate(&bad.replace("planned", "draft"), sparql).conforms());
}
