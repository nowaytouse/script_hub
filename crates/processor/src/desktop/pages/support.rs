use super::card;
use crate::desktop::app::{HubApp, SupportTab};
use crate::desktop::i18n::{Text, text};
use eframe::egui::{self, RichText};

const LICENSE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../LICENSE"));
const NOTICES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../THIRD_PARTY_NOTICES.md"
));
const PRIVACY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../PRIVACY.md"));
const DEPENDENCIES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/THIRD_PARTY_LICENSES.txt"
));

impl HubApp {
    pub(crate) fn support_page(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        ui.heading(tr(Text::Help));
        ui.label(tr(Text::HelpIntro));
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
                        for (title, body) in [
                            (Text::Workspace, Text::HelpWorkspace), (Text::Tasks, Text::HelpUpdates),
                            (Text::Sources, Text::HelpSources), (Text::Repository, Text::HelpGit),
                            (Text::KeyboardShortcuts, Text::HelpShortcuts), (Text::Privacy, Text::HelpPrivacy),
                            (Text::RecordHistory, Text::RecordHistoryHelp), (Text::DeveloperMode, Text::DeveloperModeHelp),
                        ] {
                            egui::CollapsingHeader::new(tr(title)).show(ui, |ui| { ui.label(tr(body)); });
                        }
                        ui.add_space(12.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.hyperlink_to(tr(Text::Documentation), "https://github.com/nowaytouse/script_hub#readme");
                            ui.hyperlink_to(tr(Text::IssueTracker), "https://github.com/nowaytouse/script_hub/issues");
                        });
                    }
                    SupportTab::About => {
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
                        let source = option_env!("GITHUB_SHA").filter(|sha| sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
                            .map_or_else(|| "https://github.com/nowaytouse/script_hub".to_string(), |sha| format!("https://github.com/nowaytouse/script_hub/tree/{sha}"));
                        ui.hyperlink_to(tr(Text::SourceCode), source);
                    }
                    SupportTab::Licenses => {
                        ui.label(tr(Text::LicenseScope));
                        ui.add_space(16.0);
                        ui.label(RichText::new(tr(Text::DependencyLicenses)).strong());
                        ui.label(tr(Text::DependencyLicenseHelp));
                        if ui.button(tr(Text::CopyLicenseText)).clicked() {
                            ui.ctx().copy_text(DEPENDENCIES.to_string());
                        }
                        ui.add_space(16.0);
                        document(ui, NOTICES);
                        egui::CollapsingHeader::new("GNU General Public License v3.0").show(ui, |ui| { ui.label(LICENSE); });
                        ui.hyperlink_to(tr(Text::LicensesAndNotices), "https://github.com/nowaytouse/script_hub/tree/master/rulesets/Sources/licenses");
                    }
                    SupportTab::Privacy => {
                        ui.label(tr(Text::HelpPrivacy));
                        ui.add_space(12.0);
                        document(ui, PRIVACY);
                    }
                }
            });
        });
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
    use super::{DEPENDENCIES, LICENSE, NOTICES, PRIVACY};
    #[test]
    fn legal_material_is_bundled_for_offline_use() {
        assert!(LICENSE.contains("GNU GENERAL PUBLIC LICENSE"));
        assert!(NOTICES.contains("app2smile"));
        assert!(PRIVACY.contains("SQLite"));
        assert!(DEPENDENCIES.contains("Ubuntu Font"));
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
