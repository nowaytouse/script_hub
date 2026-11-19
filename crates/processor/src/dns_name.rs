//! DNS label operations shared by rule inputs, protection and projections.

/// Yield a name and its parent suffixes through the last label. The caller
/// owns normalization and decides which suffixes are eligible to match.
pub(crate) fn ancestors(host: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(host), |name| {
        name.split_once('.').map(|(_, tail)| tail)
    })
}

/// Check the ASCII, dotted host form used by the native domain-only exporters.
/// The source and client still decide whether public suffixes can be rules.
pub(crate) fn is_dotted_ascii_host(domain: &str) -> bool {
    domain.len() <= 253
        && domain.rsplit_once('.').is_some_and(|(_, suffix)| {
            suffix.bytes().all(|byte| byte.is_ascii_alphabetic()) || suffix.starts_with("xn--")
        })
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_helpers_leave_normalization_and_suffix_scope_to_the_caller() {
        assert_eq!(
            ancestors("notexample.Example.test").collect::<Vec<_>>(),
            ["notexample.Example.test", "Example.test", "test"]
        );
        assert!(ancestors("localhost").eq(["localhost"]));
        assert!(is_dotted_ascii_host("_api.xn--bcher-kva.test"));
        assert!(!is_dotted_ascii_host("192.0.2.1"));
        assert!(!is_dotted_ascii_host("example..test"));
    }
}
