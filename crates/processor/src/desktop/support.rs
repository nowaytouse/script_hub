//! Localized legal documents, build identity and dependency notices.

use super::i18n::Text;
use crate::runtime::settings::{Language, Preferences};

pub(crate) const LICENSE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../LICENSE"));
const NOTICES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../THIRD_PARTY_NOTICES.md"
));
const PRIVACY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../PRIVACY.md"));
const NOTICES_ZH_CN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../THIRD_PARTY_NOTICES.zh-CN.md"
));
const PRIVACY_ZH_CN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../PRIVACY.zh-CN.md"
));
pub(crate) const DEPENDENCIES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/THIRD_PARTY_LICENSES.txt"
));
pub(crate) const REPOSITORY: &str = "https://github.com/nowaytouse/script_hub";
const NOTICE_SEPARATOR: &str =
    "==============================================================================";
pub(crate) const NOTICE_RESULT_LIMIT: usize = 20;

fn build_revision() -> Option<&'static str> {
    option_env!("GITHUB_SHA")
        .filter(|sha| sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub(crate) fn build_details() -> String {
    let mut details = format!(
        "Script Hub {}\nTarget: {} / {}\nBuild: {}\nLicense: {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        env!("CARGO_PKG_LICENSE"),
    );
    if let Some(sha) = build_revision() {
        details.push_str("\nRevision: ");
        details.push_str(sha);
    }
    details
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    On,
    Off,
    Paused,
}

impl Status {
    fn from_setting(value: Option<bool>) -> Self {
        match value {
            Some(true) => Self::On,
            Some(false) => Self::Off,
            None => Self::Paused,
        }
    }

    fn label(self) -> Text {
        match self {
            Self::On => Text::SupportStatusOn,
            Self::Off => Text::SupportStatusOff,
            Self::Paused => Text::SupportStatusPaused,
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
            Self::Paused => "paused",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Diagnostics {
    history: Status,
    logs: Status,
    checker_selected: bool,
}

impl Diagnostics {
    pub(crate) fn new(saved: Option<&Preferences>, checker_selected: bool) -> Self {
        Self {
            history: Status::from_setting(saved.map(|saved| saved.record_history)),
            logs: Status::from_setting(saved.map(|saved| {
                saved.record_history && saved.developer_mode && saved.diagnostic_logs
            })),
            checker_selected,
        }
    }

    pub(crate) fn history(self) -> Text {
        self.history.label()
    }

    pub(crate) fn logs(self) -> Text {
        self.logs.label()
    }

    pub(crate) fn checker(self) -> Text {
        if self.checker_selected {
            Text::RelayEngineReady
        } else {
            Text::RelayEngineMissing
        }
    }

    pub(crate) fn summary(self) -> String {
        format!(
            "{}\nTask history: {}\nDiagnostic log excerpts: {}\nConnection checker selected: {}",
            build_details(),
            self.history.summary(),
            self.logs.summary(),
            if self.checker_selected { "yes" } else { "no" },
        )
    }
}

pub(crate) fn matching_notices<'content>(
    content: &'content str,
    words: &[String],
) -> impl Iterator<Item = (usize, &'content str, &'content str, &'content str)> {
    // Borrow each block, preserving the original license text and its line endings.
    let mut remaining = Some(content);
    let mut blocks = std::iter::from_fn(move || {
        let block = remaining.take()?;
        let mut start = 0;
        for line in block.split_inclusive('\n') {
            let end = start + line.len();
            if line.trim_end_matches(['\r', '\n']) == NOTICE_SEPARATOR {
                remaining = Some(&block[end..]);
                return Some(&block[..start]);
            }
            start = end;
        }
        Some(block)
    });
    blocks.next();
    std::iter::from_fn(move || Some((blocks.next()?, blocks.next()?)))
        .enumerate()
        .filter_map(move |(index, (header, body))| {
            let mut lines = header.lines();
            let name = lines.next()?;
            let crates = lines.next()?.strip_prefix("Used by: ")?;
            if !words.is_empty() {
                let metadata = header.to_ascii_lowercase();
                if !words.iter().all(|word| metadata.contains(word)) {
                    return None;
                }
            }
            Some((index, name, crates, body.trim_matches(['\r', '\n'])))
        })
}

pub(crate) fn localized_documents(language: Language) -> (&'static str, &'static str) {
    match language.resolved() {
        Language::ZhCn => (NOTICES_ZH_CN, PRIVACY_ZH_CN),
        _ => (NOTICES, PRIVACY),
    }
}

pub(crate) fn build_source_url() -> String {
    build_revision().map_or_else(
        || REPOSITORY.to_owned(),
        |sha| format!("{REPOSITORY}/tree/{sha}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_report_effective_saved_settings_without_private_paths() {
        let mut saved = Preferences {
            root: "private-workspace".into(),
            singbox: "private-checker".into(),
            record_history: false,
            diagnostic_logs: true,
            ..Preferences::default()
        };
        let diagnostics = Diagnostics::new(Some(&saved), true);
        assert_eq!(diagnostics.history(), Text::SupportStatusOff);
        assert_eq!(diagnostics.logs(), Text::SupportStatusOff);
        assert_eq!(diagnostics.checker(), Text::RelayEngineReady);
        assert!(
            diagnostics
                .summary()
                .contains("Connection checker selected: yes")
        );
        assert!(!diagnostics.summary().contains("private-"));

        saved.record_history = true;
        let diagnostics = Diagnostics::new(Some(&saved), false);
        assert_eq!(diagnostics.history(), Text::SupportStatusOn);
        assert_eq!(diagnostics.logs(), Text::SupportStatusOff);
        assert_eq!(diagnostics.checker(), Text::RelayEngineMissing);
        saved.developer_mode = true;
        assert_eq!(
            Diagnostics::new(Some(&saved), false).logs(),
            Text::SupportStatusOn
        );

        let unavailable = Diagnostics::new(None, false);
        assert_eq!(unavailable.history(), Text::SupportStatusPaused);
        assert_eq!(unavailable.logs(), Text::SupportStatusPaused);
    }

    #[test]
    fn support_documents_follow_the_selected_language() {
        assert_eq!(localized_documents(Language::En), (NOTICES, PRIVACY));
        assert_eq!(
            localized_documents(Language::ZhCn),
            (NOTICES_ZH_CN, PRIVACY_ZH_CN)
        );
        assert_eq!(localized_documents(Language::Ja), (NOTICES, PRIVACY));
        assert_ne!(PRIVACY, PRIVACY_ZH_CN);
        assert_ne!(NOTICES, NOTICES_ZH_CN);
    }
    #[test]
    fn legal_material_is_bundled_for_offline_use() {
        assert!(LICENSE.contains("GNU GENERAL PUBLIC LICENSE"));
        assert!(NOTICES.contains("app2smile"));
        assert!(PRIVACY.contains("SQLite"));
        assert!(DEPENDENCIES.contains("its bundled SQLite amalgamation"));
    }

    #[test]
    fn search_covers_the_embedded_dependency_inventory_not_just_license_names() {
        let all = matching_notices(DEPENDENCIES, &[]).count();
        assert_eq!(
            all,
            DEPENDENCIES
                .lines()
                .filter(|line| line.starts_with("Used by: "))
                .count()
        );
        let words = ["rusqlite".to_owned(), "mit".to_owned()];
        let (_, name, crates, body) = matching_notices(DEPENDENCIES, &words)
            .next()
            .expect("rusqlite is distributed under MIT");
        assert_eq!(name, "MIT License");
        assert!(crates.contains("rusqlite "));
        assert!(body.contains("Permission is hereby granted"));
        let build = build_details();
        assert!(build.contains(env!("CARGO_PKG_VERSION")));
        assert!(!build.contains(env!("CARGO_MANIFEST_DIR")));
    }

    #[test]
    fn notice_search_accepts_crlf_and_mixed_line_endings() {
        let lf = format!(
            "Preamble\n{NOTICE_SEPARATOR}\nMIT License\nUsed by: example 1.0;\n{NOTICE_SEPARATOR}\n\nLine one\nLine two\n\n{NOTICE_SEPARATOR}\nOther License\nUsed by: other 2.0;\n{NOTICE_SEPARATOR}\nAnother body"
        );
        let crlf = lf.replace('\n', "\r\n");
        let mixed = lf
            .replace("Used by: example 1.0;\n", "Used by: example 1.0;\r\n")
            .replace(
                &format!("{NOTICE_SEPARATOR}\n"),
                &format!("{NOTICE_SEPARATOR}\r\n"),
            );
        let words = ["mit".to_owned(), "example".to_owned()];
        for (content, body) in [
            (&lf, "Line one\nLine two"),
            (&crlf, "Line one\r\nLine two"),
            (&mixed, "Line one\nLine two"),
        ] {
            let mut matches = matching_notices(content, &words);
            assert_eq!(
                matches.next(),
                Some((0, "MIT License", "example 1.0;", body))
            );
            assert!(matches.next().is_none());
            assert_eq!(matching_notices(content, &[]).count(), 2);
        }
    }

    #[test]
    fn sqlite_notice_matches_the_locked_version() {
        let package = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"))
            .split("[[package]]")
            .find(|package| package.lines().any(|line| line == "name = \"rusqlite\""))
            .expect("rusqlite must be present in the lockfile");
        let version = package
            .lines()
            .find_map(|line| {
                line.strip_prefix("version = \"")
                    .and_then(|s| s.strip_suffix('"'))
            })
            .expect("the locked rusqlite package must declare its version");
        assert!(DEPENDENCIES.contains(&format!("rusqlite {version};")));
    }
}
