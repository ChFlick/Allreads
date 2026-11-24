//! Thin UniFFI wrappers over `allreads-core`. Kept deliberately small.

/// Returns the core version. Placeholder until UniFFI is wired up in step 5.
pub fn core_version() -> String {
    allreads_core::version().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_version_is_forwarded() {
        assert_eq!(core_version(), allreads_core::version());
    }
}
