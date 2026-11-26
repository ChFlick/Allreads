//! Domain logic for Allreads. Books, text, vocabulary and storage live here.

pub mod anki;
pub mod article;
pub mod book;
pub mod db;
pub mod dictionary;
pub mod epub;
pub mod library;
pub mod presets;
pub mod reader;
pub mod text;
pub mod translate;
pub mod txt;
pub mod word_tally;

/// Returns the version of the core library.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_package_version() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
