//! Thin WASM adapter: callers supply all config and source text; no filesystem API.
#![deny(unsafe_code)]
use docvera_engine::{CheckerContext, WorkspaceSources, config::Config, sql::SqlSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::prelude::*;

fn error(e: impl std::fmt::Display) -> JsValue {
    // Construct a real JS Error, rather than throwing a string.
    wasm_bindgen::JsError::new(&e.to_string()).into()
}

fn to_js(value: &impl Serialize) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(error)
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Sources {
    documents: BTreeMap<String, String>,
    schemas: BTreeMap<String, String>,
    shapes: BTreeMap<String, String>,
    assets: BTreeSet<String>,
}

/// Reusable checker baseline. SQL snapshots are explicit and independent of edits.
#[wasm_bindgen(js_name = Checker)]
pub struct WasmChecker {
    inner: CheckerContext,
}

#[wasm_bindgen(js_class = Checker)]
impl WasmChecker {
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue, sources: JsValue) -> Result<WasmChecker, JsValue> {
        let config: Config = serde_wasm_bindgen::from_value(config).map_err(error)?;
        let sources: Sources = serde_wasm_bindgen::from_value(sources).map_err(error)?;
        let inner = CheckerContext::from_sources(
            config,
            WorkspaceSources {
                documents: sources.documents,
                schemas: sources.schemas,
                shapes: sources.shapes,
                assets: sources.assets,
            },
        )
        .map_err(error)?;
        Ok(Self { inner })
    }

    pub fn check(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.check().map_err(error)?)
    }

    #[wasm_bindgen(js_name = checkDocument)]
    pub fn check_document(&self, path: &str, text: &str) -> Result<JsValue, JsValue> {
        to_js(&self.inner.check_document(path, text).map_err(error)?)
    }

    #[wasm_bindgen(js_name = checkWithText)]
    pub fn check_with_text(&self, path: &str, text: &str) -> Result<JsValue, JsValue> {
        to_js(&self.inner.check_with_text(path, text).map_err(error)?)
    }

    #[wasm_bindgen(js_name = updateDocument)]
    pub fn update_document(&mut self, path: &str, text: String) -> Result<(), JsValue> {
        self.inner.update_document(path, text).map_err(error)
    }

    #[wasm_bindgen(js_name = removeDocument)]
    pub fn remove_document(&mut self, path: &str) -> Result<(), JsValue> {
        self.inner.remove_document(path).map_err(error)
    }

    #[wasm_bindgen(js_name = updateSchema)]
    pub fn update_schema(&mut self, path: &str, text: String) -> Result<(), JsValue> {
        self.inner.update_schema(path, text).map_err(error)
    }

    #[wasm_bindgen(js_name = sqlSnapshot)]
    pub fn sql_snapshot(&self) -> Result<WasmSqlSnapshot, JsValue> {
        let workspace = self.inner.snapshot().map_err(error)?;
        Ok(WasmSqlSnapshot {
            inner: SqlSnapshot::from_workspace(&workspace).map_err(error)?,
        })
    }
}

#[wasm_bindgen(js_name = SqlSnapshot)]
pub struct WasmSqlSnapshot {
    inner: SqlSnapshot,
}

#[wasm_bindgen(js_class = SqlSnapshot)]
impl WasmSqlSnapshot {
    pub fn query(&mut self, sql: &str) -> Result<JsValue, JsValue> {
        to_js(&self.inner.query(sql).map_err(error)?)
    }
}
