//! What this executable is: its version and the commit it was built from (stamped by `build.rs`), and
//! links to the documentation *of that commit*.
//!
//! The documentation is linked at the commit rather than at `main`, so a link always describes the build
//! that is running, however old. That is also why every change updates its documentation in the same
//! commit: a link to a commit whose pages are stale stays stale forever.

/// The version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The full commit hash, or `unknown` outside a git checkout.
pub const COMMIT: &str = env!("QVT_COMMIT");

/// The commit's first eight characters.
pub const COMMIT_SHORT: &str = env!("QVT_COMMIT_SHORT");

/// `-dirty` when built from a tree with uncommitted changes, else empty.
pub const DIRTY: &str = env!("QVT_DIRTY");

/// The repository on GitHub.
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// A file of the repository as it is at this build's commit (`main` for an unknown build).
pub fn doc_url(path: &str) -> String {
    format!("{REPOSITORY}/blob/{}/{path}", env!("QVT_DOC_REF"))
}

/// The documentation's entry point at this build's commit.
pub fn documentation() -> String {
    doc_url("README.md")
}

/// The answer to `--version`, also what the release readme is written from: name and version, then
/// `commit:` and `documentation:` lines.
pub fn version_text() -> String {
    format!(
        "Quick Video Transcoder {VERSION}\ncommit: {COMMIT}{DIRTY}\ndocumentation: {}",
        documentation()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_the_documentation_of_the_build() {
        let text = version_text();
        assert!(text.starts_with(&format!("Quick Video Transcoder {VERSION}\n")));
        let link = text
            .lines()
            .find_map(|l| l.strip_prefix("documentation: "))
            .unwrap();
        assert!(link.starts_with(REPOSITORY), "{link}");
        assert!(link.ends_with("/README.md"), "{link}");
        if COMMIT != "unknown" {
            assert!(link.contains(COMMIT), "{link}");
            assert_eq!(COMMIT_SHORT, &COMMIT[..8]);
        }
    }
}
