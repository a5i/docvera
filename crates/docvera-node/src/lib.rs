use docvera_engine::{CheckerContext, sql::SqlSnapshot};
use napi_derive::napi;
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

fn failure(error: impl std::fmt::Display) -> napi::Error {
    napi::Error::from_reason(error.to_string())
}
async fn blocking<T: Send + 'static>(
    job: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> napi::Result<T> {
    tokio::task::spawn_blocking(job)
        .await
        .map_err(failure)?
        .map_err(|e| failure(format!("{e:#}")))
}

/// An explicitly owned in-memory baseline. No disk reads after construction.
#[napi]
pub struct NativeChecker {
    inner: Arc<Mutex<Option<CheckerContext>>>,
}
impl NativeChecker {
    async fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut CheckerContext) -> anyhow::Result<T> + Send + 'static,
    ) -> napi::Result<T> {
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
        let context = blocking(move || CheckerContext::load(&PathBuf::from(root))).await?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Some(context))),
        })
    }
    #[napi(factory)]
    pub async fn from_sources(config: Value, sources: Value) -> napi::Result<Self> {
        let context = blocking(move || {
            CheckerContext::from_sources(
                serde_json::from_value(config)?,
                serde_json::from_value(sources)?,
            )
        })
        .await?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Some(context))),
        })
    }
    #[napi]
    pub async fn check(&self) -> napi::Result<Value> {
        self.run(|c| Ok(serde_json::to_value(c.check()?)?)).await
    }
    #[napi]
    pub async fn check_document(&self, path: String, text: String) -> napi::Result<Value> {
        self.run(move |c| Ok(serde_json::to_value(c.check_document(&path, &text)?)?))
            .await
    }
    #[napi]
    pub async fn check_with_text(&self, path: String, text: String) -> napi::Result<Value> {
        self.run(move |c| Ok(serde_json::to_value(c.check_with_text(&path, &text)?)?))
            .await
    }
    #[napi]
    pub async fn update_document(&self, path: String, text: String) -> napi::Result<()> {
        self.run(move |c| c.update_document(&path, text)).await
    }
    #[napi]
    pub async fn update_schema(&self, path: String, text: String) -> napi::Result<()> {
        self.run(move |c| c.update_schema(&path, text)).await
    }
    #[napi]
    pub async fn remove_document(&self, path: String) -> napi::Result<()> {
        self.run(move |c| c.remove_document(&path)).await
    }
    #[napi]
    pub async fn query_sql(&self, sql: String) -> napi::Result<Value> {
        self.run(move |c| c.snapshot()?.query_sql(&sql)).await
    }
    #[napi]
    pub async fn references(
        &self,
        id: String,
        direction: String,
        depth: u32,
    ) -> napi::Result<Value> {
        self.run(move |c| c.snapshot()?.graph(&id, &direction, depth as usize))
            .await
    }
    #[napi]
    pub async fn extract(&self, section_id: String) -> napi::Result<Value> {
        self.run(move |c| c.snapshot()?.extract(&section_id)).await
    }
    #[napi]
    pub async fn sql_snapshot(&self) -> napi::Result<NativeSqlSnapshot> {
        let snapshot = self
            .run(|c| SqlSnapshot::from_workspace(&c.snapshot()?))
            .await?;
        Ok(NativeSqlSnapshot {
            inner: Arc::new(Mutex::new(Some(snapshot))),
        })
    }
    #[napi]
    pub fn close(&self) -> napi::Result<()> {
        self.inner.lock().map_err(failure)?.take();
        Ok(())
    }
}

#[napi]
pub struct NativeSqlSnapshot {
    inner: Arc<Mutex<Option<SqlSnapshot>>>,
}
#[napi]
impl NativeSqlSnapshot {
    #[napi]
    pub async fn query_sql(&self, sql: String) -> napi::Result<Value> {
        let inner = self.inner.clone();
        blocking(move || {
            inner
                .lock()
                .map_err(|_| anyhow::anyhow!("SQL snapshot lock poisoned"))?
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("SQL snapshot is closed"))?
                .query(&sql)
        })
        .await
    }
    #[napi]
    pub fn close(&self) -> napi::Result<()> {
        self.inner.lock().map_err(failure)?.take();
        Ok(())
    }
}
