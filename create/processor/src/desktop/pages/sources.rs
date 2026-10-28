use super::{card, primary_button};
use crate::desktop::app::{HubApp, Pending};
use crate::desktop::i18n::{Text, text};
use crate::pipeline::custom_sources::{self, CustomSource, SourceCategory};
use eframe::egui::{self, RichText};

impl HubApp {
    pub(crate) fn sources_page(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        let ready = self.workspace_ready() && !self.active();
        egui::ScrollArea::vertical().id_salt("sources").show(ui, |ui| {
            card(ui, self.colors, |ui| {
                ui.label(RichText::new(tr(Text::AddSource)).size(18.0).strong());
                ui.label(tr(Text::SourceTrust));
                ui.add_space(12.0);
                ui.add_enabled_ui(ready, |ui| {
                    ui.label(tr(Text::SourceCategory));
                    egui::ComboBox::from_id_salt("source-category")
                        .selected_text(tr(source_category_name(self.source_category))).show_ui(ui, |ui| {
                            for category in [SourceCategory::Wool, SourceCategory::Weibo, SourceCategory::Devtools] {
                                ui.selectable_value(&mut self.source_category, category, tr(source_category_name(category)));
                            }
                        });
                    ui.label(tr(Text::SourceName));
                    ui.add(egui::TextEdit::singleline(&mut self.source_name).desired_width(f32::INFINITY));
                    let name_error = if self.source_name.trim().is_empty() { None }
                        else { custom_sources::validate_name(self.source_name.trim()).err() };
                    if let Some(error) = &name_error { ui.colored_label(self.colors.error, error); }
                    ui.label(tr(Text::SourceUrl));
                    ui.add(egui::TextEdit::singleline(&mut self.source_url).desired_width(f32::INFINITY)
                        .hint_text("https://raw.githubusercontent.com/owner/repo/main/module.sgmodule"));
                    ui.weak(tr(Text::SourceValidationHint));
                    let url_error = if self.source_url.trim().is_empty() { None }
                        else { custom_sources::validate_url(self.source_url.trim()).err() };
                    if let Some(error) = &url_error { ui.colored_label(self.colors.error, error); }
                    ui.add_space(8.0);
                    if ui.add_enabled(!self.source_name.trim().is_empty() && !self.source_url.trim().is_empty() && name_error.is_none() && url_error.is_none(),
                        primary_button(self.colors, tr(Text::SaveSource))).clicked()
                        && let Some(root) = self.loaded_root.clone() {
                            self.pending = Some(Pending::AddSource(root, CustomSource {
                                category: self.source_category,
                                name: self.source_name.trim().to_string(), url: self.source_url.trim().to_string(),
                            }));
                        }
                });
                ui.add_space(10.0);
                ui.weak(tr(Text::HelpSources));
            });
            ui.add_space(16.0);
            ui.label(RichText::new(format!("{} · {}", tr(Text::Sources), self.sources.len())).strong());
            ui.add_space(8.0);
            if self.sources.is_empty() { ui.label(tr(Text::NoSources)); }
            let mut remove = None;
            for source in &self.sources {
                card(ui, self.colors, |ui| {
                    ui.label(RichText::new(&source.name).strong());
                    ui.weak(tr(source_category_name(source.category)));
                    ui.add(egui::Label::new(&source.url).wrap());
                    if ui.add_enabled(ready, egui::Button::new(tr(Text::RemoveSource))).clicked() {
                        remove = Some((source.category, source.name.clone()));
                    }
                });
                ui.add_space(10.0);
            }
            if let Some((category, name)) = remove && let Some(root) = self.loaded_root.clone() {
                self.pending = Some(Pending::RemoveSource(root, category, name));
            }
        });
    }
}

fn source_category_name(category: SourceCategory) -> Text {
    match category {
        SourceCategory::Wool => Text::WoolSource,
        SourceCategory::Weibo => Text::WeiboSource,
        SourceCategory::Devtools => Text::DevtoolsSource,
    }
}
