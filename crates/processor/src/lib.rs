//! Shared ruleset processing, artifact publication and desktop update workflows.

mod filesystem;
mod rulesets;

pub mod app;
#[cfg(all(feature = "gui", any(target_os = "windows", target_os = "macos")))]
pub mod gui;
pub mod pipeline;
pub mod promax;
#[cfg(feature = "gui")]
pub mod relay;
pub mod runtime;
pub mod transforms;
#[cfg(feature = "gui")]
pub mod vault;

pub use filesystem::safe_write_file_internal;
pub(crate) use filesystem::verify_missing_directory;
pub use rulesets::{RulesetInput, is_ruleset_watermark, process_ruleset, strip_inline_comment};
pub(crate) use rulesets::{generate_header, normalize_rule};

// Preserve crate-root interfaces while implementations live with their owners.
pub use app::{update_cli, validate_cli};
pub use pipeline::{
    firewall_sync, functional_update, general_update, merge_bundles, mihomo_generator,
    module_catalog, srs_generator,
};
pub use runtime::job as gui_job;
pub use transforms::{mitm_manager, smart_cleanup, url_rewriter};

pub(crate) fn redact_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let host = authority.rsplit('@').next().unwrap_or(authority);
    format!("{scheme}://{host}{}", &rest[authority_end..])
}

pub fn publication_now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().with_timezone(
        &chrono::FixedOffset::east_opt(8 * 60 * 60).expect("UTC+8 is a valid fixed offset"),
    )
}
