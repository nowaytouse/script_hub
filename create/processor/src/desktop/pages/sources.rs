use super::card;
use crate::desktop::app::{HubApp, Pending};
use crate::desktop::i18n::{Text, text};
use crate::pipeline::custom_sources::{CustomSource, SourceCategory};
use eframe::egui::{self, RichText};

impl HubApp {
    pub(crate) fn sources_page(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        let ready = self.workspace_ready() && !self.active();
        egui::ScrollArea::vertical().id_salt("sources").show(ui, |ui| {
            card(ui, self.colors, |ui| {
                ui.label(RichText::new(tr(Text::AddSource)).size(18.0).strong()); ui.label(tr(Text::SourceTrust)); ui.add_space(12.0);
                ui.add_enabled_ui(ready, |ui| {
                    ui.label(tr(Text::SourceCategory));
                    egui::ComboBox::from_id_salt("source-category").selected_text(source_category_name(self.source_category)).show_ui(ui, |ui| {
                        for category in [SourceCategory::Wool, SourceCategory::Weibo, SourceCategory::Devtools] { ui.selectable_value(&mut self.source_category, category, source_category_name(category)); }
                    });
                    ui.label(tr(Text::SourceName)); ui.add(egui::TextEdit::singleline(&mut self.source_name).desired_width(f32::INFINITY));
                    ui.label(tr(Text::SourceUrl)); ui.add(egui::TextEdit::singleline(&mut self.source_url).desired_width(f32::INFINITY).hint_text("https://raw.githubusercontent.com/owner/repo/main/module.sgmodule"));
                    ui.add_space(8.0);
                    if ui.add_enabled(!self.source_name.trim().is_empty() && !self.source_url.trim().is_empty(), egui::Button::new(tr(Text::SaveSource))).clicked()
                        && let Some(root) = self.loaded_root.clone() {
                            self.pending = Some(Pending::AddSource(root, CustomSource { category: self.source_category, name: self.source_name.trim().to_string(), url: self.source_url.trim().to_string() }));
                        }
                });
                ui.add_space(8.0); ui.weak(tr(Text::HelpSources));
            });
            ui.add_space(16.0);
            if self.sources.is_empty() { ui.label(tr(Text::NoSources)); }
            for source in self.sources.clone() {
                card(ui, self.colors, |ui| {
                    ui.label(RichText::new(&source.name).strong()); ui.weak(source_category_name(source.category)); ui.label(&source.url);
                    if ui.add_enabled(ready, egui::Button::new(tr(Text::RemoveSource))).clicked()
                        && let Some(root) = self.loaded_root.clone() { self.pending = Some(Pending::RemoveSource(root, source.category, source.name.clone())); }
                }); ui.add_space(10.0);
            }
        });
    }
}

fn source_category_name(category: SourceCategory) -> &'static str {
    match category {
        SourceCategory::Wool => "WOOL · PROMAX",
        SourceCategory::Weibo => "WEIBO · PROMAX",
        SourceCategory::Devtools => "DEVTOOLS · Script Hub / BoxJs / Sub-Store",
    }
}
