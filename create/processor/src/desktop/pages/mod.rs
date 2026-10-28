mod catalog;
mod history;
mod overview;
mod repository;
mod settings;
mod sources;
mod support;
mod tasks;

use super::theme::Palette;
use eframe::egui;

fn card<R>(ui: &mut egui::Ui, colors: Palette, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(colors.surface)
        .stroke(egui::Stroke::new(1.0, colors.border.gamma_multiply(0.45)))
        .corner_radius(12)
        .inner_margin(20)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui)
        })
        .inner
}

fn primary_button(colors: Palette, label: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(label).color(colors.on_accent))
        .fill(colors.accent)
        .corner_radius(8)
        .min_size(egui::vec2(120.0, 36.0))
}
