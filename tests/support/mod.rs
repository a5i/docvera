use std::{fs, path::Path};
use tempfile::TempDir;

pub const SCENARIOS: &[(&str, usize)] = &[
    ("product-traceability", 6),
    ("qa-traceability", 6),
    ("operational-readiness", 4),
    ("architecture-governance", 5),
    ("project-governance", 4),
];

pub fn fixture(name: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/use-cases")
        .join(name);
    copy_tree(&source, dir.path());
    dir
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        // Never copy a developer's derived index into a test fixture.
        if entry.file_name() == ".mdx" {
            continue;
        }
        let destination = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

pub fn edit(root: &Path, file: &str, from: &str, to: &str) {
    let path = root.join(file);
    let source = fs::read_to_string(&path).unwrap();
    assert!(source.contains(from), "Missing {from:?} in {file}");
    fs::write(path, source.replace(from, to)).unwrap();
}

pub fn reports(root: &Path) -> Vec<std::path::PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(root.join("queries"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    paths.sort();
    paths
}
