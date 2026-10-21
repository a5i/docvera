//! Workspace configuration. TOML is canonical; YAML is a migration fallback.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

/// Supported workspace configuration format.
pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    #[schemars(extend("const" = CONFIG_VERSION))]
    pub version: u32,
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    pub schemas: BTreeMap<String, String>,
    #[serde(default)]
    pub validation: Validation,
    #[serde(default)]
    pub parser: ParserOptions,
    #[serde(default)]
    pub front_matter: FrontMatterOptions,
}

#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Validation {
    /// Keep strict CI behavior by default; skip only documents with no schema selector or mapping.
    #[serde(default)]
    pub untyped: UntypedPolicy,
    pub shacl: Option<Shacl>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Shacl {
    /// Explicit local Turtle paths, relative to the workspace configuration.
    pub shapes: Vec<String>,
}

#[derive(
    Debug, Default, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq,
)]
#[serde(rename_all = "camelCase")]
pub enum UntypedPolicy {
    #[default]
    Error,
    Skip,
}

fn yes() -> bool {
    true
}

/// Metadata extensions are deliberately absent: front matter is handled separately.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct ParserOptions {
    pub tables: bool,
    pub strikethrough: bool,
    pub task_lists: bool,
    pub footnotes: bool,
    pub gfm: bool,
    pub math: bool,
    pub wikilinks: bool,
    pub smart_punctuation: bool,
    pub heading_attributes: bool,
    pub definition_lists: bool,
    pub superscript: bool,
    pub subscript: bool,
}
impl Default for ParserOptions {
    fn default() -> Self {
        Self {
            tables: true,
            strikethrough: true,
            task_lists: true,
            footnotes: false,
            gfm: false,
            math: false,
            wikilinks: false,
            smart_punctuation: false,
            heading_attributes: false,
            definition_lists: false,
            superscript: false,
            subscript: false,
        }
    }
}
impl ParserOptions {
    pub fn options(&self) -> pulldown_cmark::Options {
        use pulldown_cmark::Options as O;
        let mut options = O::empty();
        for (enabled, flag) in [
            (self.tables, O::ENABLE_TABLES),
            (self.strikethrough, O::ENABLE_STRIKETHROUGH),
            (self.task_lists, O::ENABLE_TASKLISTS),
            (self.footnotes, O::ENABLE_FOOTNOTES),
            (self.gfm, O::ENABLE_GFM),
            (self.math, O::ENABLE_MATH),
            (self.wikilinks, O::ENABLE_WIKILINKS),
            (self.smart_punctuation, O::ENABLE_SMART_PUNCTUATION),
            (self.heading_attributes, O::ENABLE_HEADING_ATTRIBUTES),
            (self.definition_lists, O::ENABLE_DEFINITION_LIST),
            (self.superscript, O::ENABLE_SUPERSCRIPT),
            (self.subscript, O::ENABLE_SUBSCRIPT),
        ] {
            options.set(flag, enabled);
        }
        options
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontMatterOptions {
    #[serde(default = "yes")]
    pub yaml: bool,
    #[serde(default = "yes")]
    pub toml: bool,
}
impl Default for FrontMatterOptions {
    fn default() -> Self {
        Self {
            yaml: true,
            toml: true,
        }
    }
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
        config.validate()?;
        Ok((config, text))
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != CONFIG_VERSION {
            bail!("Unsupported workspace version: {}", self.version);
        }
        if self
            .validation
            .shacl
            .as_ref()
            .is_some_and(|s| s.shapes.is_empty())
        {
            bail!(
                "validation.shacl.shapes must not be empty; omit validation.shacl to disable SHACL"
            );
        }
        Ok(())
    }
}
