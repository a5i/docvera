//! Reusable checker over an owned workspace snapshot. After construction, checks do no I/O.
use crate::{
    config::{Config, UntypedPolicy},
    model::*,
    parser,
    policy::Policies,
    schema::{self, Compiled, Schema},
    workspace::{self, Workspace},
};
use anyhow::{Context, Result, bail};
use globset::{Glob, GlobMatcher};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

/// Portable workspace-relative paths and UTF-8 source texts. No filesystem is required.
#[derive(Default)]
pub struct WorkspaceSources {
    pub documents: BTreeMap<String, String>,
    pub schemas: BTreeMap<String, String>,
    pub shapes: BTreeMap<String, String>,
    /// Non-Markdown files accepted as link targets (contents are not needed).
    pub assets: BTreeSet<String>,
}

#[derive(Default)]
pub(crate) struct WorkspacePaths {
    pub assets: BTreeSet<String>,
    outside: BTreeSet<String>,
}
impl WorkspacePaths {
    pub fn is_outside(&self, path: &str) -> bool {
        self.outside
            .iter()
            .any(|p| path == p || path.strip_prefix(p).is_some_and(|s| s.starts_with('/')))
    }
}

/// Lexically normalize a portable relative path; reject escapes and OS-dependent syntax.
pub(crate) fn normalize_path(path: &str) -> Result<String> {
    if path.starts_with('/') || path.contains(['\\', ':', '\0']) {
        bail!("Path must be workspace-relative: {path}");
    }
    let mut parts = vec![];
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    bail!("Path escapes workspace: {path}");
                }
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        bail!("Expected a file path: {path}");
    }
    Ok(parts.join("/"))
}

struct Prepared {
    document: Document,
    schema: Option<String>,
    skipped: bool,
}

pub struct CheckerContext {
    root: PathBuf,
    config: Config,
    includes: Vec<GlobMatcher>,
    excludes: Vec<GlobMatcher>,
    mappings: Vec<(GlobMatcher, String)>,
    schemas: BTreeMap<String, Result<Compiled, String>>,
    documents: BTreeMap<String, Prepared>,
    paths: WorkspacePaths,
    policies: Policies,
    base_fingerprint: blake3::Hasher,
    schema_hashes: BTreeMap<String, blake3::Hash>,
    fingerprint: String,
}

fn matchers(patterns: &[String]) -> Result<Vec<GlobMatcher>> {
    patterns
        .iter()
        .map(|p| Ok(Glob::new(p)?.compile_matcher()))
        .collect()
}

impl CheckerContext {
    /// Load a snapshot once. Refresh/recreate it when configuration, schemas or assets change.
    pub fn load(root: &Path) -> Result<Self> {
        let root = root.canonicalize()?;
        let (config, _) = Config::load(&root)?;
        let includes = matchers(&config.include)?;
        let excludes = matchers(&config.exclude)?;
        let policies = Policies::load(&root, config.validation.shacl.as_ref())?;
        let mut sources = WorkspaceSources::default();
        let mut outside = BTreeSet::new();
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                !e.file_type().is_dir()
                    || ![".git", ".mdx", "target", "node_modules"]
                        .contains(&e.file_name().to_string_lossy().as_ref())
            })
        {
            let entry = entry?;
            let path = workspace::portable(entry.path().strip_prefix(&root)?);
            if entry.file_type().is_symlink() {
                if workspace::within(&root, entry.path()).is_err() {
                    outside.insert(path);
                } else if entry.path().is_file() {
                    sources.assets.insert(path);
                }
                continue;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            sources.assets.insert(path.clone());
            if includes.iter().any(|g| g.is_match(&path))
                && !excludes.iter().any(|g| g.is_match(&path))
            {
                sources.documents.insert(
                    path,
                    fs::read_to_string(entry.path())
                        .with_context(|| format!("Cannot read {}", entry.path().display()))?,
                );
            }
        }
        // Load every mapped schema, including ones used by newly created editor buffers.
        // Explicit selectors used by current documents are loaded as well. New selectors
        // can be supplied via update_schema/from_sources or by refreshing this snapshot.
        let mut schema_paths: BTreeSet<String> = config.schemas.values().cloned().collect();
        for (path, source) in &sources.documents {
            let doc = parser::parse_with_options(
                path.clone(),
                source.clone(),
                &config.parser,
                &config.front_matter,
            );
            if let Some(selector) = doc
                .metadata
                .get("$schema")
                .and_then(serde_json::Value::as_str)
            {
                let joined = format!(
                    "{}/{}",
                    path.rsplit_once('/').map_or(".", |(p, _)| p),
                    selector
                );
                if let Ok(path) = normalize_path(&joined) {
                    schema_paths.insert(path);
                }
            }
        }
        let mut errors = BTreeMap::new();
        for path in schema_paths {
            let key = match normalize_path(&path) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let text = (|| {
                let absolute = workspace::within(&root, &root.join(&key))?;
                fs::read_to_string(&absolute)
                    .with_context(|| format!("Cannot read {}", absolute.display()))
            })();
            match text {
                Ok(text) => {
                    sources.schemas.insert(key, text);
                }
                Err(error) => {
                    errors.insert(key, Err(format!("{error:#}")));
                }
            }
        }
        let mut context = Self::build(config, sources, policies)?;
        context.root = root;
        context.paths.outside = outside;
        if !errors.is_empty() {
            context.schemas.extend(errors);
            context.reprepare()?;
        }
        Ok(context)
    }

    /// Construct a checker entirely from caller-owned text (also usable on WASM).
    pub fn from_sources(config: Config, sources: WorkspaceSources) -> Result<Self> {
        let policies = Policies::from_sources(config.validation.shacl.as_ref(), &sources.shapes)?;
        Self::build(config, sources, policies)
    }

    fn build(config: Config, sources: WorkspaceSources, policies: Policies) -> Result<Self> {
        config.validate()?;
        let includes = matchers(&config.include)?;
        let excludes = matchers(&config.exclude)?;
        let mappings = config
            .schemas
            .iter()
            .map(|(p, s)| Ok((Glob::new(p)?.compile_matcher(), s.clone())))
            .collect::<Result<_>>()?;
        let mut schemas = BTreeMap::new();
        let mut fingerprint = blake3::Hasher::new();
        fingerprint.update(env!("CARGO_PKG_VERSION").as_bytes());
        fingerprint.update(toml::to_string(&config)?.as_bytes());
        policies.fingerprint(&mut fingerprint);
        let mut schema_hashes = BTreeMap::new();
        for (path, text) in sources.schemas {
            let key = normalize_path(&path)?;
            if schemas.contains_key(&key) {
                bail!("Duplicate normalized schema path: {key}");
            }
            schema_hashes.insert(key.clone(), blake3::hash(text.as_bytes()));
            let compiled = (|| {
                let schema: Schema = serde_saphyr::from_str(&text)?;
                schema.check_definition()?;
                Compiled::compile(text, &schema)
            })()
            .map_err(|e: anyhow::Error| format!("{e:#}"));
            schemas.insert(key, compiled);
        }
        let mut context = Self {
            root: PathBuf::new(),
            config,
            includes,
            excludes,
            mappings,
            schemas,
            documents: BTreeMap::new(),
            paths: WorkspacePaths {
                assets: sources
                    .assets
                    .into_iter()
                    .map(|p| normalize_path(&p))
                    .collect::<Result<_>>()?,
                outside: BTreeSet::new(),
            },
            policies,
            base_fingerprint: fingerprint,
            schema_hashes,
            fingerprint: String::new(),
        };
        context.refresh_fingerprint();
        let mut document_paths = BTreeSet::new();
        for (path, text) in sources.documents {
            let path = normalize_path(&path)?;
            if !document_paths.insert(path.clone()) {
                bail!("Duplicate normalized document path: {path}");
            }
            context.update_document(&path, text)?;
        }
        Ok(context)
    }

    fn refresh_fingerprint(&mut self) {
        let mut fingerprint = self.base_fingerprint.clone();
        for (key, hash) in &self.schema_hashes {
            fingerprint.update(&(key.len() as u64).to_le_bytes());
            fingerprint.update(key.as_bytes());
            fingerprint.update(hash.as_bytes());
        }
        self.fingerprint = fingerprint.finalize().to_hex().to_string();
    }

    /// Register or replace schema text without I/O (e.g. an unsaved/new $schema selector).
    /// Invalid definitions remain schema.resolve diagnostics on selecting documents.
    pub fn update_schema(&mut self, path: &str, text: String) -> Result<()> {
        let key = normalize_path(path)?;
        let hash = blake3::hash(text.as_bytes());
        let compiled = (|| {
            let schema: Schema = serde_saphyr::from_str(&text)?;
            schema.check_definition()?;
            Compiled::compile(text, &schema)
        })()
        .map_err(|e: anyhow::Error| format!("{e:#}"));
        self.schemas.insert(key.clone(), compiled);
        self.schema_hashes.insert(key, hash);
        self.refresh_fingerprint();
        self.reprepare()
    }

    fn included(&self, path: &str) -> bool {
        !path
            .split('/')
            .any(|p| [".git", ".mdx", "target", "node_modules"].contains(&p))
            && self.includes.iter().any(|g| g.is_match(path))
            && !self.excludes.iter().any(|g| g.is_match(path))
    }

    fn prepare(&self, path: String, text: String) -> Prepared {
        let mut doc = parser::parse_with_options(
            path.clone(),
            text,
            &self.config.parser,
            &self.config.front_matter,
        );
        let mut skipped = false;
        let selected: Result<Option<String>> = (|| {
            if let Some(selector) = doc.metadata.get("$schema") {
                let selector = selector.as_str().context("$schema must be a string")?;
                // Check before joining: an absolute selector must never become relative.
                if selector.starts_with('/') || selector.contains(['\\', ':']) {
                    bail!("Schema path must be workspace-relative");
                }
                let parent = path.rsplit_once('/').map_or(".", |(p, _)| p);
                return Ok(Some(normalize_path(&format!("{parent}/{selector}"))?));
            }
            let matches: Vec<_> = self
                .mappings
                .iter()
                .filter(|(g, _)| g.is_match(&path))
                .collect();
            match matches.as_slice() {
                [(_, schema)] => Ok(Some(normalize_path(schema)?)),
                [] if self.config.validation.untyped == UntypedPolicy::Skip => {
                    skipped = true;
                    Ok(None)
                }
                [] => bail!("No schema mapping for {path}"),
                _ => bail!("Multiple schema mappings for {path}; use explicit $schema"),
            }
        })();
        let mut selected_key = None;
        let resolved: Result<()> = (|| {
            if let Some(key) = selected? {
                let compiled = self
                    .schemas
                    .get(&key)
                    .with_context(|| format!("Schema unavailable in snapshot: {key}"))?
                    .as_ref()
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                schema::validate(&mut doc, compiled.schema(), compiled)?;
                selected_key = Some(key);
            }
            Ok(())
        })();
        if let Err(error) = resolved {
            doc.diagnostic(doc.metadata_span(), "schema.resolve", format!("{error:#}"));
        }
        if skipped {
            doc.diagnostics.clear();
        }
        Prepared {
            document: doc,
            schema: selected_key,
            skipped,
        }
    }

    fn reprepare(&mut self) -> Result<()> {
        let texts: Vec<_> = self
            .documents
            .values()
            .map(|p| (p.document.path.clone(), p.document.source.clone()))
            .collect();
        for (path, text) in texts {
            self.update_document(&path, text)?;
        }
        Ok(())
    }

    /// Replace one document in the baseline without disk I/O; the old ID/anchors disappear.
    pub fn update_document(&mut self, path: &str, text: String) -> Result<()> {
        let path = normalize_path(path)?;
        if self.included(&path) {
            let prepared = self.prepare(path.clone(), text);
            self.documents.insert(path, prepared);
        }
        Ok(())
    }
    pub fn remove_document(&mut self, path: &str) -> Result<()> {
        self.documents.remove(&normalize_path(path)?);
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> Result<Workspace> {
        self.overlay(None)
    }
    pub fn check(&self) -> Result<CheckResult> {
        Ok(self.snapshot()?.check())
    }

    /// Check an overlay without mutating the baseline. Includes impacts on other documents.
    pub fn check_with_text(&self, path: &str, text: &str) -> Result<CheckResult> {
        let path = normalize_path(path)?;
        Ok(self.overlay(Some((&path, text)))?.check())
    }

    /// Same findings as check_with_text, restricted to this document. Excluded paths return
    /// documents=0; untyped skipped paths return documents=1 and no diagnostics.
    pub fn check_document(&self, path: &str, text: &str) -> Result<CheckResult> {
        let path = normalize_path(path)?;
        if !self.included(&path) {
            return Ok(CheckResult {
                contract_version: CONTRACT_VERSION,
                documents: 0,
                diagnostics: vec![],
            });
        }
        let mut result = self.check_with_text(&path, text)?;
        result.documents = 1;
        result.diagnostics.retain(|d| d.path == path);
        Ok(result)
    }

    fn overlay(&self, overlay: Option<(&str, &str)>) -> Result<Workspace> {
        let replacement = overlay
            .filter(|(p, _)| self.included(p))
            .map(|(p, t)| self.prepare(p.into(), t.into()));
        let mut prepared: BTreeMap<&str, &Prepared> = self
            .documents
            .iter()
            .map(|(p, d)| (p.as_str(), d))
            .collect();
        if let Some(doc) = &replacement {
            prepared.insert(&doc.document.path, doc);
        }
        let documents = prepared.values().map(|p| p.document.clone()).collect();
        let schemas = prepared
            .values()
            .map(|p| {
                p.schema
                    .as_ref()
                    .and_then(|k| self.schemas.get(k))
                    .and_then(|c| c.as_ref().ok())
                    .map(|c| c.schema().clone())
            })
            .collect();
        let mut snapshot = workspace::validate_documents(
            self.root.clone(),
            documents,
            schemas,
            &self.paths,
            &self.policies,
            self.fingerprint.clone(),
        )?;
        for doc in &mut snapshot.documents {
            if prepared[doc.path.as_str()].skipped {
                doc.diagnostics.clear();
            }
        }
        Ok(snapshot)
    }
}
