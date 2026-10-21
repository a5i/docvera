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

/// Long-lived, serialized checker snapshot. No Markdown/config/schema reads during checks.
#[napi]
pub struct NativeChecker {
    inner: std::sync::Arc<std::sync::Mutex<Option<md_engine::CheckerContext>>>,
}

impl NativeChecker {
    async fn run<F>(&self, job: F) -> napi::Result<Value>
    where
        F: FnOnce(&mut md_engine::CheckerContext) -> anyhow::Result<Value> + Send + 'static,
    {
        let inner = self.inner.clone();
        blocking(move || {
            let mut guard = inner
                .lock()
                .map_err(|_| anyhow::anyhow!("Checker lock poisoned"))?;
            job(guard
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("Checker is closed"))?)
        })
        .await
    }
}

#[napi]
impl NativeChecker {
    #[napi(factory)]
    pub async fn open(root: String) -> napi::Result<Self> {
        let context = tokio::task::spawn_blocking(move || {
            md_engine::CheckerContext::load(&PathBuf::from(root))
        })
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))?
        .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;
        Ok(Self {
            inner: std::sync::Arc::new(std::sync::Mutex::new(Some(context))),
        })
    }
    #[napi]
    pub async fn check(&self) -> napi::Result<Value> {
        self.run(|context| Ok(serde_json::to_value(context.check()?)?))
            .await
    }
    #[napi]
    pub async fn check_document(&self, path: String, text: String) -> napi::Result<Value> {
        self.run(move |context| Ok(serde_json::to_value(context.check_document(&path, &text)?)?))
            .await
    }
    #[napi]
    pub async fn check_with_text(&self, path: String, text: String) -> napi::Result<Value> {
        self.run(move |context| {
            Ok(serde_json::to_value(
                context.check_with_text(&path, &text)?,
            )?)
        })
        .await
    }
    #[napi]
    pub async fn update_schema(&self, path: String, text: String) -> napi::Result<()> {
        self.run(move |context| {
            context.update_schema(&path, text)?;
            Ok(Value::Null)
        })
        .await?;
        Ok(())
    }
    #[napi]
    pub async fn update_document(&self, path: String, text: String) -> napi::Result<()> {
        self.run(move |context| {
            context.update_document(&path, text)?;
            Ok(Value::Null)
        })
        .await?;
        Ok(())
    }
    #[napi]
    pub async fn remove_document(&self, path: String) -> napi::Result<()> {
        self.run(move |context| {
            context.remove_document(&path)?;
            Ok(Value::Null)
        })
        .await?;
        Ok(())
    }
    #[napi]
    pub fn close(&self) -> napi::Result<()> {
        self.inner
            .lock()
            .map_err(|_| napi::Error::from_reason("Checker lock poisoned"))?
            .take();
        Ok(())
    }
}
