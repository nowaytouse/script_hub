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
        .inner_margin(ui.spacing().window_margin)
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
        .min_size(egui::vec2(120.0, 40.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::{settings::Preferences, theme};

    #[test]
    fn card_keeps_long_content_inside_a_narrow_viewport() {
        let ctx = egui::Context::default();
        theme::apply(&ctx, &Preferences::default());
        let colors = theme::palette(false, Preferences::default().accent);
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(280.0, 540.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let boundary = ui.available_rect_before_wrap();
                    let content = card(ui, colors, |ui| {
                        ui.add(egui::Label::new("workspace/".repeat(40)).wrap());
                        ui.min_rect()
                    });
                    assert!(content.right() <= boundary.right());
                });
            },
        );
        output.drop_without_applying_deltas();
    }
}
