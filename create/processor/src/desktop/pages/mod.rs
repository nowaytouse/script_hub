mod catalog;
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
        .stroke(egui::Stroke::new(1.0, colors.border))
        .corner_radius(10)
        .inner_margin(18)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui)
        })
        .inner
}
