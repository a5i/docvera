pub mod checker;
pub mod config;
pub mod contracts;
pub mod model;
pub mod parser;
#[cfg(feature = "shacl")]
mod policy;
#[cfg(not(feature = "shacl"))]
#[path = "policy_disabled.rs"]
mod policy;
pub mod retrieval;
pub mod schema;
#[cfg(feature = "shacl")]
mod shacl_report;
mod source_map;
#[cfg(feature = "sql")]
pub mod sql;
mod toml_source_map;
pub mod workspace;

use anyhow::{Context, Result, bail};
pub use checker::{CheckerContext, WorkspaceSources};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
pub use workspace::Workspace;

pub const EXAMPLE_FILES: &[(&str, &str)] = &[
    (
        "docvera.toml",
        include_str!("../assets/workspace/docvera.toml"),
    ),
    (
        "schemas/adr.yaml",
        include_str!("../assets/workspace/schemas/adr.yaml"),
    ),
    (
        "schemas/feature.yaml",
        include_str!("../assets/workspace/schemas/feature.yaml"),
    ),
    (
        "schemas/test-plan.yaml",
        include_str!("../assets/workspace/schemas/test-plan.yaml"),
    ),
    (
        "docs/adr-0001.md",
        include_str!("../assets/workspace/docs/adr-0001.md"),
    ),
    (
        "docs/feature-0042.md",
        include_str!("../assets/workspace/docs/feature-0042.md"),
    ),
    (
        "docs/test-0001.md",
        include_str!("../assets/workspace/docs/test-0001.md"),
    ),
];

/// Never overwrite existing files. Intended for a fresh demonstration directory.
pub fn init(root: &Path) -> Result<Vec<String>> {
    if root.join("docvera.toml").try_exists()? || root.join("docvera.yaml").try_exists()? {
        bail!(
            "Workspace config already exists; refusing to overwrite or create a conflicting config"
        );
    }
    for (path, _) in EXAMPLE_FILES {
        if root.join(path).exists() {
            bail!("Refusing to overwrite {}", root.join(path).display());
        }
    }
    fs::create_dir_all(root)?;
    let canonical_root = root.canonicalize()?;
    let mut created = vec![];
    // The config gate is written last: an init that fails partway (for
    // example on a blocker where a directory is expected) leaves no docvera.toml
    // behind and can simply be retried once the blocker is removed.
    for (relative, text) in EXAMPLE_FILES.iter().rev() {
        let path = root.join(relative);
        let parent = path.parent().unwrap();
        fs::create_dir_all(parent)
            .with_context(|| format!("Cannot create {}", parent.display()))?;
        workspace::within(&canonical_root, parent)?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(text.as_bytes())?;
        created.push(relative.to_string());
    }
    created.reverse();
    Ok(created)
}
