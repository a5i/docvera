use md_engine::{Workspace, index};
use napi_derive::napi;
use serde_json::Value;
use std::path::PathBuf;

async fn blocking<F>(job: F) -> napi::Result<Value>
where
    F: FnOnce() -> anyhow::Result<Value> + Send + 'static,
{
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))?
        .map_err(|e| napi::Error::from_reason(format!("{e:#}")))
}

#[napi]
pub async fn check(root: String) -> napi::Result<Value> {
    blocking(move || {
        Ok(serde_json::to_value(
            Workspace::load(&PathBuf::from(root))?.check(),
        )?)
    })
    .await
}
#[napi]
pub async fn index(root: String, rebuild: bool) -> napi::Result<Value> {
    blocking(move || {
        Ok(serde_json::to_value(index::index(
            &Workspace::load(&PathBuf::from(root))?,
            rebuild,
        )?)?)
    })
    .await
}
#[napi]
pub async fn query_sql(root: String, sql: String) -> napi::Result<Value> {
    blocking(move || index::query(&PathBuf::from(root), &sql)).await
}
#[napi]
pub async fn references(
    root: String,
    id: String,
    direction: String,
    depth: u32,
) -> napi::Result<Value> {
    blocking(move || index::graph(&PathBuf::from(root), &id, &direction, depth as usize)).await
}
#[napi]
pub async fn search(root: String, options: Value) -> napi::Result<Value> {
    blocking(move || index::search(&PathBuf::from(root), &serde_json::from_value(options)?)).await
}
#[napi]
pub async fn extract(root: String, section_id: String) -> napi::Result<Value> {
    blocking(move || index::extract(&PathBuf::from(root), &section_id)).await
}
