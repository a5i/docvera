pub mod config;
pub mod index;
pub mod model;
pub mod parser;
mod policy;
pub mod schema;
mod shacl_report;
mod source_map;
pub mod workspace;

use anyhow::{Result, bail};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
pub use workspace::Workspace;

pub const EXAMPLE_FILES: &[(&str, &str)] = &[
    (
        "mdx.toml",
        include_str!("../../../examples/workspace/mdx.toml"),
    ),
    (
        "schemas/adr.yaml",
        include_str!("../../../examples/workspace/schemas/adr.yaml"),
    ),
    (
        "schemas/feature.yaml",
        include_str!("../../../examples/workspace/schemas/feature.yaml"),
    ),
    (
        "schemas/test-plan.yaml",
        include_str!("../../../examples/workspace/schemas/test-plan.yaml"),
    ),
    (
        "docs/adr-0001.md",
        include_str!("../../../examples/workspace/docs/adr-0001.md"),
    ),
    (
        "docs/feature-0042.md",
        include_str!("../../../examples/workspace/docs/feature-0042.md"),
    ),
    (
        "docs/test-0001.md",
        include_str!("../../../examples/workspace/docs/test-0001.md"),
    ),
];

/// Never overwrite existing files. Intended for a fresh demonstration directory.
pub fn init(root: &Path) -> Result<Vec<String>> {
    if root.join("mdx.toml").try_exists()? || root.join("mdx.yaml").try_exists()? {
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
    for (relative, text) in EXAMPLE_FILES {
        let path = root.join(relative);
        let parent = path.parent().unwrap();
        fs::create_dir_all(parent)?;
        workspace::within(&canonical_root, parent)?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(text.as_bytes())?;
        created.push(relative.to_string());
    }
    Ok(created)
}
