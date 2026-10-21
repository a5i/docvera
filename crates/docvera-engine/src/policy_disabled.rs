//! Fail closed when a checker-only build encounters configured SHACL policies.
use crate::{
    config::Shacl,
    model::{Document, Relation},
};
use anyhow::{Result, bail};
use std::path::Path;

pub struct Policies;
impl Policies {
    pub fn load(_: &Path, config: Option<&Shacl>) -> Result<Self> {
        if config.is_some() {
            bail!("SHACL is configured but this build has no shacl feature");
        }
        Ok(Self)
    }
    pub fn from_sources(
        config: Option<&Shacl>,
        _: &std::collections::BTreeMap<String, String>,
    ) -> Result<Self> {
        Self::load(Path::new(""), config)
    }
    pub fn fingerprint(&self, _: &mut blake3::Hasher) {}
    pub fn validate(&self, _: &mut [Document], _: &[Relation]) -> Result<()> {
        Ok(())
    }
}
