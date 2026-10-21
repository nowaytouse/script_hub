use super::card;
use crate::desktop::app::HubApp;
use crate::desktop::i18n::Text;
use eframe::egui::{self, RichText};

impl HubApp {
    pub(crate) fn support_page(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.tr(Text::Help));
        ui.label(self.tr(Text::HelpIntro));
        ui.add_space(18.0);
        egui::ScrollArea::vertical()
            .id_salt("help-and-about")
            .show(ui, |ui| {
                card(ui, self.colors, |ui| {
                    for (title, body) in [
                        (Text::Workspace, Text::HelpWorkspace),
                        (Text::Tasks, Text::HelpUpdates),
                        (Text::Sources, Text::HelpSources),
                        (Text::Repository, Text::HelpGit),
                        (Text::KeyboardShortcuts, Text::HelpShortcuts),
                        (Text::LocalOnly, Text::HelpPrivacy),
                    ] {
                        egui::CollapsingHeader::new(self.tr(title)).show(ui, |ui| {
                            ui.label(self.tr(body));
                        });
                    }
                    ui.add_space(10.0);
                    ui.weak("Ctrl / Command + 1 · 2 · 3 · 4");
                    ui.horizontal_wrapped(|ui| {
                        ui.hyperlink_to(
                            self.tr(Text::Documentation),
                            "https://github.com/nowaytouse/script_hub#readme",
                        );
                        ui.hyperlink_to(
                            self.tr(Text::IssueTracker),
                            "https://github.com/nowaytouse/script_hub/issues",
                        );
                    });
                });
                ui.add_space(24.0);
                ui.label(RichText::new(self.tr(Text::About)).size(20.0).strong());
                ui.add_space(10.0);
                card(ui, self.colors, |ui| {
                    ui.label(
                        RichText::new("Script Hub")
                            .size(22.0)
                            .strong()
                            .color(self.colors.accent),
                    );
                    ui.label(self.tr(Text::AboutDescription));
                    ui.add_space(12.0);
                    ui.label(format!(
                        "{} · {}",
                        self.tr(Text::Version),
                        env!("CARGO_PKG_VERSION")
                    ));
                    ui.label(format!(
                        "{} · {} / {}",
                        self.tr(Text::BuildTarget),
                        std::env::consts::OS,
                        std::env::consts::ARCH
                    ));
                    ui.label(format!(
                        "{} · Windows / macOS",
                        self.tr(Text::DesktopPlatforms)
                    ));
                    ui.hyperlink_to(
                        self.tr(Text::ProjectHome),
                        "https://github.com/nowaytouse/script_hub",
                    );
                });
            });
    }
}
