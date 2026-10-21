use std::{fs, path::Path};

/// Adapt mutation templates, not source bytes, to the fixture's line endings.
/// Production parsing/extraction and fixture copying remain byte-preserving.
pub fn line_endings_like(source: &str, text: &str) -> String {
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    text.replace("\r\n", "\n").replace('\n', newline)
}

pub fn edit(root: &Path, file: &str, from: &str, to: &str) {
    let path = root.join(file);
    let source = fs::read_to_string(&path).unwrap();
    let from = line_endings_like(&source, from);
    let to = line_endings_like(&source, to);
    assert!(source.contains(&from), "Missing {from:?} in {file}");
    fs::write(path, source.replace(&from, &to)).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_edits_preserve_lf_crlf_bom_and_unrelated_bytes() {
        for newline in ["\n", "\r\n"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("feature.md");
            let prefix = format!("\u{feff}---{newline}id: FEATURE-001{newline}");
            let suffix = format!("---{newline}# Résumé{newline}Body.  {newline}");
            let source = format!("{prefix}owner: identity-team{newline}{suffix}");
            fs::write(&path, &source).unwrap();
            edit(dir.path(), "feature.md", "owner: identity-team\n", "");
            assert_eq!(
                fs::read(&path).unwrap(),
                format!("{prefix}{suffix}").as_bytes()
            );

            fs::write(&path, &source).unwrap();
            edit(
                dir.path(),
                "feature.md",
                "owner: identity-team\r\n",
                "owner: platform-team\nstatus: planned\n",
            );
            let expected =
                format!("{prefix}owner: platform-team{newline}status: planned{newline}{suffix}");
            assert_eq!(fs::read(&path).unwrap(), expected.as_bytes());
        }
    }

    #[test]
    #[should_panic(expected = "Missing")]
    fn fixture_edit_still_rejects_a_missing_template() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("feature.md"), "owner: other-team\r\n").unwrap();
        edit(dir.path(), "feature.md", "owner: identity-team\n", "");
    }
}
