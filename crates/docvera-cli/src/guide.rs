/// The offline guide is a generated CLI result, not source-document extraction.
/// Keep its Markdown identical across LF and CRLF checkouts in every format.
pub fn text() -> String {
    assemble(&[
        include_str!("../assets/cli-guide.md"),
        include_str!("../assets/schema.md"),
        include_str!("../assets/shacl.md"),
    ])
}

fn assemble(parts: &[&str]) -> String {
    parts.join("\n---\n\n").replace("\r\n", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lf_crlf_and_mixed_checkouts_produce_identical_markdown() {
        let lf = [
            "# docvera CLI guide\n\nUsage.\n",
            "## Schema\n",
            "## SHACL\n",
        ];
        let crlf = lf.map(|part| part.replace('\n', "\r\n"));
        let expected = lf.join("\n---\n\n");
        assert_eq!(assemble(&lf), expected);
        assert_eq!(assemble(&crlf.each_ref().map(String::as_str)), expected);
        assert_eq!(assemble(&[lf[0], &crlf[1], lf[2]]), expected);
    }

    #[test]
    fn embedded_guide_has_a_stable_lf_header_and_no_carriage_returns() {
        let guide = text();
        assert!(guide.starts_with("# docvera CLI guide\n"));
        assert!(!guide.contains('\r'));
    }
}
