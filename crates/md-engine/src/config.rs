//! Workspace configuration. TOML is canonical; YAML is a migration fallback.
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    pub schemas: BTreeMap<String, String>,
    #[serde(default)]
    pub validation: Validation,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validation {
    pub shacl: Option<Shacl>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shacl {
    /// Explicit local Turtle paths, relative to the workspace configuration.
    pub shapes: Vec<String>,
}

impl Config {
    pub fn load(root: &Path) -> Result<(Self, String)> {
        let toml_path = root.join("mdx.toml");
        let yaml_path = root.join("mdx.yaml");
        let (config, text): (Self, String) =
            match (toml_path.try_exists()?, yaml_path.try_exists()?) {
                (true, true) => {
                    bail!("Both mdx.toml and mdx.yaml exist; keep exactly one workspace config")
                }
                (true, false) => {
                    let text = fs::read_to_string(&toml_path).context("Cannot read mdx.toml")?;
                    let config = toml::from_str(&text).context("Invalid mdx.toml")?;
                    (config, text)
                }
                (false, true) => {
                    let text = fs::read_to_string(&yaml_path).context("Cannot read mdx.yaml")?;
                    let config = serde_saphyr::from_str(&text).context("Invalid mdx.yaml")?;
                    (config, text)
                }
                (false, false) => bail!("Missing mdx.toml (or legacy mdx.yaml); run mdx init"),
            };
        if config.version != 1 {
            bail!("Unsupported workspace version: {}", config.version);
        }
        if config
            .validation
            .shacl
            .as_ref()
            .is_some_and(|s| s.shapes.is_empty())
        {
            bail!(
                "validation.shacl.shapes must not be empty; omit validation.shacl to disable SHACL"
            );
        }
        Ok((config, text))
    }
}
