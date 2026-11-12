use super::{URLS, VaultError};

const ARTIFACTS: &str = include_str!(concat!(env!("OUT_DIR"), "/template-artifacts.txt"));
const REPOSITORY_PREFIXES: &[&str] = &[
    "https://raw.githubusercontent.com/nowaytouse/script_hub/master/",
    "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/",
];

pub(super) fn artifact_paths() -> impl Iterator<Item = &'static str> {
    ARTIFACTS.lines()
}

pub(super) fn validate_url(url: &str) -> Result<(), VaultError> {
    let Some(relative) = REPOSITORY_PREFIXES
        .iter()
        .find_map(|&prefix| url.strip_prefix(prefix))
    else {
        return Ok(());
    };
    let path = relative.split(['?', '#']).next().unwrap_or(relative);
    let path = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .map_err(|_| VaultError::Format)?;
    if path.starts_with("rulesets/") && !artifact_paths().any(|known| known == path.as_ref()) {
        return Err(VaultError::MissingArtifact);
    }
    Ok(())
}

pub(super) fn validate_urls(content: &str) -> Result<(), VaultError> {
    URLS.find_iter(content)
        .try_for_each(|url| validate_url(url.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_current_repository_rule_artifacts_are_exportable() {
        for prefix in REPOSITORY_PREFIXES {
            for retired in ["GitHub.list", "github/GitHub.list", "Microsoft_extra.list"] {
                assert_eq!(
                    validate_url(&format!("{prefix}rulesets/RULE-SET/{retired}?cache=1")),
                    Err(VaultError::MissingArtifact)
                );
            }
            for current in [
                "RULE-SET/Microsoft_proxy.list",
                "RULE-SET/github/Microsoft_proxy_extra.list",
                "Mihomo/Microsoft_proxy.yaml",
                "SingBox/Microsoft_proxy_Singbox.srs",
            ] {
                assert!(validate_url(&format!("{prefix}rulesets/{current}")).is_ok());
            }
        }
        assert!(validate_url("https://example.invalid/GitHub.list").is_ok());
    }
}
