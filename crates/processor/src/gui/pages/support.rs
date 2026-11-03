use super::card;
use crate::gui::app::{HubApp, Page, SupportTab};
use crate::gui::i18n::{Text, text};
use crate::runtime::settings::Language;
use eframe::egui::{self, RichText};

const LICENSE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../LICENSE"));
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
const DEPENDENCIES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/THIRD_PARTY_LICENSES.txt"
));
const REPOSITORY: &str = "https://github.com/nowaytouse/script_hub";
const NOTICE_SEPARATOR: &str =
    "==============================================================================";
const NOTICE_RESULT_LIMIT: usize = 20;

fn build_revision() -> Option<&'static str> {
    option_env!("GITHUB_SHA")
        .filter(|sha| sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn build_details() -> String {
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

#[derive(Clone, Copy)]
enum DiagnosticStatus {
    On,
    Off,
    Paused,
}

impl DiagnosticStatus {
    fn from_enabled(enabled: bool) -> Self {
        if enabled { Self::On } else { Self::Off }
    }

    fn label(self) -> Text {
        match self {
            Self::On => Text::SupportStatusOn,
            Self::Off => Text::SupportStatusOff,
            Self::Paused => Text::SupportStatusPaused,
        }
    }

    fn summary_value(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
            Self::Paused => "paused",
        }
    }
}

struct DiagnosticStatusSnapshot {
    history: DiagnosticStatus,
    log_excerpts: DiagnosticStatus,
    checker_selected: bool,
}

impl DiagnosticStatusSnapshot {
    fn read(app: &HubApp) -> Self {
        let (history, log_excerpts) = if app.settings_load_failed() {
            (DiagnosticStatus::Paused, DiagnosticStatus::Paused)
        } else {
            let settings = app.configured_history();
            (
                DiagnosticStatus::from_enabled(settings.is_some()),
                DiagnosticStatus::from_enabled(settings.is_some_and(|options| options.keep_logs)),
            )
        };
        Self {
            history,
            log_excerpts,
            checker_selected: app.relay.selected_engine.is_some(),
        }
    }

    fn summary(&self) -> String {
        format!(
            "{}\nTask history: {}\nDiagnostic log excerpts: {}\nConnection checker selected: {}",
            build_details(),
            self.history.summary_value(),
            self.log_excerpts.summary_value(),
            if self.checker_selected { "yes" } else { "no" },
        )
    }
}

fn matching_notices<'a>(
    content: &'a str,
    words: &'a [String],
) -> impl Iterator<Item = (usize, &'a str, &'a str, &'a str)> {
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

fn localized_documents(language: Language) -> (&'static str, &'static str) {
    match language.resolved() {
        Language::ZhCn => (NOTICES_ZH_CN, PRIVACY_ZH_CN),
        _ => (NOTICES, PRIVACY),
    }
}

impl HubApp {
    pub(crate) fn support_page(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        let (notices, privacy) = localized_documents(language);
        ui.heading(tr(match self.support_tab {
            SupportTab::Help => Text::Help,
            SupportTab::About => Text::About,
            SupportTab::Licenses => Text::LicensesAndNotices,
            SupportTab::Privacy => Text::Privacy,
        }));
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            for (tab, key) in [
                (SupportTab::Help, Text::Help),
                (SupportTab::About, Text::About),
                (SupportTab::Licenses, Text::LicensesAndNotices),
                (SupportTab::Privacy, Text::Privacy),
            ] {
                ui.selectable_value(&mut self.support_tab, tab, tr(key));
            }
        });
        ui.add_space(16.0);
        egui::ScrollArea::vertical().id_salt(("support", self.support_tab as u8)).show(ui, |ui| {
            card(ui, self.colors, |ui| {
                match self.support_tab {
                    SupportTab::Help => {
                        ui.label(tr(Text::HelpIntro));
                        ui.horizontal_wrapped(|ui| {
                            if ui.button(tr(Text::Relay)).clicked() {
                                self.page = Page::Relay;
                            }
                            if ui.button(tr(Text::Settings)).clicked() {
                                self.page = Page::Settings;
                            }
                            self.demo_launcher(ui);
                        });
                        self.demo_launcher_feedback(ui);
                        ui.add_space(12.0);
                        egui::CollapsingHeader::new(tr(Text::DataAndDiagnostics)).show(ui, |ui| {
                            self.support_diagnostics(ui, language);
                        });
                        for (title, body) in [
                            (Text::Relay, Text::HelpRelay),
                            (Text::Workspace, Text::HelpWorkspace), (Text::Tasks, Text::HelpUpdates),
                            (Text::Sources, Text::HelpSources), (Text::Repository, Text::HelpGit),
                            (Text::KeyboardShortcuts, Text::HelpShortcuts), (Text::Privacy, Text::HelpPrivacy),
                            (Text::RecordHistory, Text::RecordHistoryHelp), (Text::DeveloperMode, Text::DeveloperModeHelp),
                        ] {
                            egui::CollapsingHeader::new(tr(title)).show(ui, |ui| { ui.label(tr(body)); });
                        }
                        ui.add_space(12.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.hyperlink_to(tr(Text::Documentation), if language.resolved() == Language::ZhCn {
                                "https://github.com/nowaytouse/script_hub/blob/master/README.zh-CN.md"
                            } else {
                                "https://github.com/nowaytouse/script_hub#readme"
                            });
                            ui.hyperlink_to(tr(Text::IssueTracker), "https://github.com/nowaytouse/script_hub/issues");
                        });
                    }
                    SupportTab::About => {
                        if let Some(texture) = ui.ctx().data(|data| data.get_temp::<egui::TextureHandle>(egui::Id::unique("script-hub-logo"))) {
                            ui.image((texture.id(), egui::vec2(64.0, 64.0)));
                        }
                        ui.label(RichText::new("Script Hub").size(30.0).strong().color(self.colors.accent));
                        ui.label(tr(Text::AboutDescription));
                        ui.add_space(16.0);
                        egui::Grid::new("about-details").spacing([24.0, 12.0]).show(ui, |ui| {
                            for (label, value) in [
                                (Text::Version, env!("CARGO_PKG_VERSION").to_string()),
                                (Text::License, env!("CARGO_PKG_LICENSE").to_string()),
                                (Text::BuildTarget, format!("{} / {}", std::env::consts::OS, std::env::consts::ARCH)),
                                (Text::DesktopPlatforms, "Windows / macOS".to_string()),
                                (Text::BuildMode, tr(if cfg!(debug_assertions) { Text::DevelopmentBuild } else { Text::ReleaseBuild }).to_string()),
                            ] {
                                ui.weak(tr(label)); ui.label(value); ui.end_row();
                            }
                        });
                        ui.add_space(16.0);
                        ui.label(tr(Text::LicenseScope));
                        let source = build_revision()
                            .map_or_else(|| REPOSITORY.to_owned(), |sha| format!("{REPOSITORY}/tree/{sha}"));
                        ui.horizontal_wrapped(|ui| {
                            if ui.button(tr(Text::CopyBuildDetails)).clicked() {
                                self.copy(ui.ctx(), &build_details());
                            }
                            if ui.button(tr(Text::LicensesAndNotices)).clicked() {
                                self.support_tab = SupportTab::Licenses;
                            }
                            ui.hyperlink_to(tr(Text::SourceCode), source);
                        });
                    }
                    SupportTab::Licenses => {
                        ui.label(tr(Text::LicenseScope));
                        ui.add_space(16.0);
                        egui::CollapsingHeader::new(tr(Text::ProjectLicense)).show(ui, |ui| {
                            ui.label(LICENSE);
                        });
                        ui.add_space(16.0);
                        ui.label(RichText::new(tr(Text::DependencyLicenses)).strong());
                        ui.label(tr(Text::DependencyLicenseHelp));
                        dependency_search(ui, language);
                        if ui.button(tr(Text::CopyLicenseText)).clicked() {
                            self.copy(ui.ctx(), DEPENDENCIES);
                        }
                        ui.add_space(16.0);
                        document(ui, notices);
                        ui.hyperlink_to(tr(Text::LicensesAndNotices), if language.resolved() == Language::ZhCn {
                            "https://github.com/nowaytouse/script_hub/blob/master/rulesets/Sources/licenses/README.zh-CN.md"
                        } else {
                            "https://github.com/nowaytouse/script_hub/tree/master/rulesets/Sources/licenses"
                        });
                    }
                    SupportTab::Privacy => {
                        ui.label(tr(Text::HelpPrivacy));
                        ui.add_space(12.0);
                        document(ui, privacy);
                    }
                }
            });
        });
    }

    fn support_diagnostics(&mut self, ui: &mut egui::Ui, language: Language) {
        let tr = |key| text(language, key);
        let status = DiagnosticStatusSnapshot::read(self);
        for (setting, value) in [
            (Text::History, status.history),
            (Text::DiagnosticLogs, status.log_excerpts),
        ] {
            ui.label(format!("{}: {}", tr(setting), tr(value.label())));
        }
        ui.label(tr(if status.checker_selected {
            Text::RelayEngineReady
        } else {
            Text::RelayEngineMissing
        }));
        ui.weak(tr(Text::SupportDiagnosticsSafe));
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr(Text::SupportCopyDiagnostics)).clicked() {
                self.copy(ui.ctx(), &status.summary());
            }
            if ui.button(tr(Text::Settings)).clicked() {
                self.page = Page::Settings;
            }
        });
    }
}

fn dependency_search(ui: &mut egui::Ui, language: Language) {
    let tr = |key| text(language, key);
    let id = ui.make_persistent_id("dependency-license-search");
    let mut query = ui
        .data_mut(|data| data.get_temp::<String>(id))
        .unwrap_or_default();
    let label = ui.label(tr(Text::SearchDependencyLicenses));
    let edited = ui
        .add(
            egui::TextEdit::singleline(&mut query)
                .char_limit(96)
                .desired_width(320.0_f32.min(ui.available_width())),
        )
        .labelled_by(label.id)
        .changed();
    if edited {
        ui.data_mut(|data| data.insert_temp(id, query.clone()));
    }
    let words: Vec<_> = query
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect();
    if words.is_empty() {
        return;
    }
    let mut matches = matching_notices(DEPENDENCIES, &words).peekable();
    if matches.peek().is_none() {
        ui.weak(tr(Text::NoDependencyLicenseMatches));
    }
    for (index, name, crates, body) in matches.by_ref().take(NOTICE_RESULT_LIMIT) {
        egui::CollapsingHeader::new(name)
            .id_salt(index)
            .show(ui, |ui| {
                ui.label(crates);
                ui.add_space(8.0);
                ui.label(body);
            });
    }
    if matches.next().is_some() {
        ui.weak(tr(Text::RefineDependencyLicenseSearch));
    }
}

fn document(ui: &mut egui::Ui, content: &str) {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            ui.add_space(6.0);
        } else if let Some(heading) = line.strip_prefix("# ").or_else(|| line.strip_prefix("## ")) {
            ui.add_space(6.0);
            ui.label(RichText::new(heading).strong().size(18.0));
        } else {
            ui.add(egui::Label::new(line).wrap());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEPENDENCIES, DiagnosticStatusSnapshot, LICENSE, NOTICE_SEPARATOR, NOTICES, NOTICES_ZH_CN,
        PRIVACY, PRIVACY_ZH_CN, build_details, localized_documents, matching_notices,
    };
    use crate::gui::app::test_app;
    use crate::runtime::settings::{Language, Preferences};

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
        assert!(DEPENDENCIES.contains("Ubuntu Font"));
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

    #[test]
    fn diagnostics_use_effective_settings_without_copying_private_paths() {
        let mut app = test_app(Preferences {
            record_history: false,
            ..Preferences::default()
        });
        app.settings_path = Some("private-settings-path/config.json".into());
        app.relay.selected_engine = Some("private-checker-path/sing-box".into());
        app.draft.record_history = true;
        app.draft.diagnostic_logs = true;
        app.draft.developer_mode = true;

        let summary = DiagnosticStatusSnapshot::read(&app).summary();
        assert!(summary.contains("Task history: off"));
        assert!(summary.contains("Diagnostic log excerpts: off"));
        assert!(summary.contains("Connection checker selected: yes"));
        assert!(!summary.contains("private-"));

        app.preferences.record_history = true;
        app.preferences.diagnostic_logs = true;
        let summary = DiagnosticStatusSnapshot::read(&app).summary();
        assert!(summary.contains("Task history: on"));
        assert!(summary.contains("Diagnostic log excerpts: off"));
        app.preferences.developer_mode = true;
        let summary = DiagnosticStatusSnapshot::read(&app).summary();
        assert!(summary.contains("Diagnostic log excerpts: on"));
        assert!(!summary.contains("private-"));
    }
}
