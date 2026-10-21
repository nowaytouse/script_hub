use super::card;
use crate::desktop::app::{CatalogTab, HubApp};
use crate::desktop::i18n::{Text, text};
use eframe::egui::{self, RichText};

impl HubApp {
    pub(crate) fn catalog_page(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.tr(Text::Modules));
        ui.add_space(14.0);
        ui.horizontal_wrapped(|ui| {
            for (tab, key) in [
                (CatalogTab::Modules, Text::Modules),
                (CatalogTab::Rules, Text::Rules),
                (CatalogTab::Sources, Text::Sources),
            ] {
                if ui
                    .selectable_label(self.catalog_tab == tab, self.tr(key))
                    .clicked()
                {
                    self.catalog_tab = tab;
                }
            }
        });
        ui.add_space(12.0);
        match self.catalog_tab {
            CatalogTab::Modules => self.modules_view(ui),
            CatalogTab::Rules => self.rules_view(ui),
            CatalogTab::Sources => self.sources_page(ui),
        }
    }

    fn modules_view(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        ui.horizontal_wrapped(|ui| {
            ui.label(tr(Text::Platform));
            let old = self.shadowrocket;
            ui.add_enabled_ui(!self.active(), |ui| {
                ui.selectable_value(&mut self.shadowrocket, false, "Surge");
                ui.selectable_value(&mut self.shadowrocket, true, "Shadowrocket");
            });
            if old != self.shadowrocket {
                self.reload();
                self.category.clear();
            }
            egui::ComboBox::from_id_salt("module-category")
                .selected_text(if self.category.is_empty() {
                    tr(Text::AllCategories)
                } else {
                    &self.category
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.category, String::new(), tr(Text::AllCategories));
                    let categories: std::collections::BTreeSet<_> = self
                        .catalog
                        .modules
                        .iter()
                        .map(|module| module.category.clone())
                        .collect();
                    for category in categories {
                        ui.selectable_value(&mut self.category, category.clone(), category);
                    }
                });
        });
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.module_search)
                .hint_text(tr(Text::SearchModules))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(12.0);
        let query = self.module_search.trim().to_lowercase();
        let indices: Vec<_> = self
            .catalog
            .modules
            .iter()
            .enumerate()
            .filter_map(|(index, module)| module.matches(&query, &self.category).then_some(index))
            .collect();
        if !self
            .selected_module
            .is_some_and(|index| indices.contains(&index))
        {
            self.selected_module = indices.first().copied();
        }
        let wide = ui.available_width() > 820.0;
        if wide {
            egui::Panel::right("module-details")
                .default_size(370.0)
                .size_range(280.0..=520.0)
                .frame(
                    egui::Frame::new()
                        .fill(self.colors.surface)
                        .inner_margin(18),
                )
                .show(ui, |ui| self.module_details(ui));
        }
        let height = if wide {
            ui.available_height()
        } else {
            (ui.available_height() * 0.47).max(100.0)
        };
        egui::ScrollArea::vertical()
            .id_salt("module-list")
            .max_height(height)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if indices.is_empty() {
                    ui.label(tr(Text::NoResults));
                }
                for index in indices {
                    let module = &self.catalog.modules[index];
                    let response = ui.add(
                        egui::Button::new("")
                            .left_text(RichText::new(&module.name).strong())
                            .right_text(&module.category)
                            .selected(self.selected_module == Some(index))
                            .min_size(egui::vec2(ui.available_width(), 54.0))
                            .wrap()
                            .corner_radius(7),
                    );
                    if response.clicked() {
                        self.selected_module = Some(index);
                    }
                }
            });
        if !wide {
            ui.add_space(12.0);
            card(ui, self.colors, |ui| self.module_details(ui));
        }
    }

    fn module_details(&mut self, ui: &mut egui::Ui) {
        let Some(module) = self
            .selected_module
            .and_then(|index| self.catalog.modules.get(index))
            .cloned()
        else {
            ui.label(self.tr(Text::SelectItem));
            return;
        };
        egui::ScrollArea::vertical()
            .id_salt("module-detail-scroll")
            .show(ui, |ui| {
                ui.label(RichText::new(&module.name).size(19.0).strong());
                ui.add_space(12.0);
                ui.weak(&module.category);
                if !module.author.is_empty() {
                    ui.label(format!("{} · {}", self.tr(Text::Author), module.author));
                }
                if !module.date.is_empty() {
                    ui.weak(format!("{} · {}", self.tr(Text::Updated), module.date));
                }
                ui.label(self.tr(if self.catalog.counterpart_exists(&module) {
                    Text::OtherPlatformAvailable
                } else {
                    Text::OtherPlatformMissing
                }));
                ui.add_space(14.0);
                if module.merged_into.is_empty() {
                    self.subscription_controls(ui, &module.install_url, &module.github_install_url);
                } else {
                    ui.colored_label(
                        self.colors.warning,
                        format!("{} {}", self.tr(Text::MergedInto), module.merged_into),
                    );
                }
                ui.add_space(18.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label(RichText::new(self.tr(Text::Description)).strong());
                ui.label(module.desc.replace("\\n", "\n"));
            });
    }

    fn rules_view(&mut self, ui: &mut egui::Ui) {
        let hint = self.tr(Text::SearchRules);
        ui.add(
            egui::TextEdit::singleline(&mut self.rule_search)
                .hint_text(hint)
                .desired_width(f32::INFINITY),
        );
        ui.add_space(12.0);
        let query = self.rule_search.trim().to_lowercase();
        let indices: Vec<_> = self
            .catalog
            .rules
            .iter()
            .enumerate()
            .filter_map(|(index, rule)| {
                (rule.name.to_lowercase().contains(&query)
                    || rule.category.to_lowercase().contains(&query))
                .then_some(index)
            })
            .collect();
        if !self
            .selected_rule
            .is_some_and(|index| indices.contains(&index))
        {
            self.selected_rule = indices.first().copied();
        }
        let wide = ui.available_width() > 820.0;
        if wide {
            egui::Panel::right("rule-details")
                .default_size(370.0)
                .size_range(280.0..=520.0)
                .frame(
                    egui::Frame::new()
                        .fill(self.colors.surface)
                        .inner_margin(18),
                )
                .show(ui, |ui| self.rule_details(ui));
        }
        let height = if wide {
            ui.available_height()
        } else {
            (ui.available_height() * 0.47).max(100.0)
        };
        egui::ScrollArea::vertical()
            .id_salt("rules-list")
            .max_height(height)
            .show(ui, |ui| {
                if indices.is_empty() {
                    ui.label(self.tr(Text::NoResults));
                }
                let row_height = if self.draft.density == crate::desktop::settings::Density::Compact
                {
                    38.0
                } else {
                    46.0
                };
                for index in indices {
                    let rule = &self.catalog.rules[index];
                    if ui
                        .add(
                            egui::Button::new("")
                                .left_text(&rule.name)
                                .right_text(rule.count.map_or_else(
                                    || rule.category.clone(),
                                    |count| count.to_string(),
                                ))
                                .selected(self.selected_rule == Some(index))
                                .min_size(egui::vec2(ui.available_width(), row_height))
                                .wrap()
                                .corner_radius(7),
                        )
                        .clicked()
                    {
                        self.selected_rule = Some(index);
                    }
                }
            });
        if !wide {
            ui.add_space(12.0);
            card(ui, self.colors, |ui| self.rule_details(ui));
        }
    }

    fn rule_details(&mut self, ui: &mut egui::Ui) {
        let Some(rule) = self
            .selected_rule
            .and_then(|index| self.catalog.rules.get(index))
            .cloned()
        else {
            ui.label(self.tr(Text::SelectItem));
            return;
        };
        egui::ScrollArea::vertical()
            .id_salt("rule-details-scroll")
            .show(ui, |ui| {
                ui.label(RichText::new(&rule.name).size(19.0).strong());
                ui.weak(&rule.category);
                ui.add_space(12.0);
                if let Some(count) = rule.count {
                    ui.label(format!("{} · {count}", self.tr(Text::RuleCount)));
                }
                self.subscription_controls(ui, &rule.cdn, &rule.github);
            });
    }

    fn subscription_controls(&mut self, ui: &mut egui::Ui, cdn: &str, github: &str) {
        ui.horizontal_wrapped(|ui| {
            if ui.button(self.tr(Text::CopyCdn)).clicked() {
                self.copy(ui.ctx(), cdn);
            }
            if ui.button(self.tr(Text::CopyGithub)).clicked() {
                self.copy(ui.ctx(), github);
            }
        });
        ui.weak(self.tr(Text::InstallHint));
        egui::CollapsingHeader::new(self.tr(Text::Details)).show(ui, |ui| {
            for value in [cdn, github] {
                let mut value = value;
                ui.add(
                    egui::TextEdit::multiline(&mut value)
                        .desired_width(f32::INFINITY)
                        .desired_rows(2)
                        .font(egui::TextStyle::Monospace),
                );
            }
        });
    }
}
