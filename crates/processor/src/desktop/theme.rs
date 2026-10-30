use super::settings::{Accent, Density, Preferences, Theme};
use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle};

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
}

pub fn palette(dark: bool, accent: Accent) -> Palette {
    let (canvas, surface, raised, border, text, muted, success, warning, error) = if dark {
        (
            [19, 24, 33],
            [25, 32, 43],
            [34, 43, 57],
            [73, 88, 108],
            [233, 238, 245],
            [170, 185, 204],
            [92, 213, 170],
            [244, 194, 102],
            [255, 152, 156],
        )
    } else {
        (
            [245, 247, 251],
            [255, 255, 255],
            [234, 240, 249],
            [157, 172, 192],
            [25, 38, 58],
            [76, 95, 118],
            [12, 114, 86],
            [147, 87, 8],
            [179, 38, 48],
        )
    };
    let accent = match (accent, dark) {
        (Accent::Blue, false) => [37, 99, 235],
        (Accent::Blue, true) => [125, 177, 255],
        (Accent::Teal, false) => [13, 112, 111],
        (Accent::Teal, true) => [94, 211, 197],
        (Accent::Amber, false) => [150, 81, 12],
        (Accent::Amber, true) => [243, 187, 99],
    };
    let color = |rgb: [u8; 3]| Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
    Palette {
        canvas: color(canvas),
        surface: color(surface),
        raised: color(raised),
        border: color(border),
        text: color(text),
        muted: color(muted),
        accent: color(accent),
        on_accent: if dark {
            color([19, 24, 33])
        } else {
            Color32::WHITE
        },
        success: color(success),
        warning: color(warning),
        error: color(error),
    }
}

pub fn apply(ctx: &egui::Context, preferences: &Preferences) {
    ctx.set_theme(match preferences.theme {
        Theme::System => egui::ThemePreference::System,
        Theme::Light => egui::ThemePreference::Light,
        Theme::Dark => egui::ThemePreference::Dark,
    });
    ctx.set_zoom_factor(f32::from(preferences.scale_percent) / 100.0);
    let colors = palette(ctx.global_style().visuals.dark_mode, preferences.accent);
    ctx.global_style_mut(|style| {
        style.animation_time = 0.0;
        let compact = preferences.density == Density::Compact;
        style.spacing.item_spacing = egui::Vec2::splat(if compact { 8.0 } else { 12.0 });
        style.spacing.button_padding = egui::vec2(12.0, if compact { 6.0 } else { 10.0 });
        style.spacing.window_margin = egui::Margin::same(if compact { 16 } else { 20 });
        style.spacing.interact_size.y = if compact { 32.0 } else { 40.0 };
        for (kind, size) in [
            (TextStyle::Body, 15.0),
            (TextStyle::Button, 14.0),
            (TextStyle::Small, 13.0),
            (TextStyle::Heading, 26.0),
            (TextStyle::Monospace, 13.0),
        ] {
            let family = if kind == TextStyle::Monospace {
                FontFamily::Monospace
            } else {
                FontFamily::Proportional
            };
            style.text_styles.insert(kind, FontId::new(size, family));
        }
        let visuals = &mut style.visuals;
        visuals.override_text_color = Some(colors.text);
        visuals.weak_text_color = Some(colors.muted);
        visuals.panel_fill = colors.canvas;
        visuals.window_fill = colors.surface;
        visuals.extreme_bg_color = colors.surface;
        visuals.faint_bg_color = colors.raised;
        visuals.hyperlink_color = colors.accent;
        visuals.selection.bg_fill = colors.accent.gamma_multiply(0.18);
        visuals.selection.stroke = Stroke::new(1.5, colors.accent);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, colors.border);
        visuals.widgets.noninteractive.bg_fill = colors.raised;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, colors.muted);
        visuals.widgets.inactive.bg_fill = colors.raised;
        visuals.widgets.inactive.weak_bg_fill = colors.surface;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, colors.border);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, colors.text);
        visuals.widgets.hovered.bg_fill = colors.raised;
        visuals.widgets.hovered.weak_bg_fill = colors.raised;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, colors.accent);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, colors.text);
        visuals.widgets.active.bg_fill = colors.raised;
        visuals.widgets.active.weak_bg_fill = colors.raised;
        visuals.widgets.active.bg_stroke = Stroke::new(1.5, colors.accent);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, colors.text);
        visuals.widgets.open.bg_fill = colors.raised;
        visuals.widgets.open.weak_bg_fill = colors.raised;
        visuals.widgets.open.bg_stroke = Stroke::new(1.5, colors.accent);
        visuals.widgets.open.fg_stroke = Stroke::new(1.0, colors.text);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn contrast(a: Color32, b: Color32) -> f32 {
        let luminance = |c: Color32| {
            let linear = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
        };
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
    #[test]
    fn semantic_text_colors_meet_aa_on_both_surfaces() {
        for dark in [false, true] {
            for accent in [Accent::Blue, Accent::Teal, Accent::Amber] {
                let p = palette(dark, accent);
                for foreground in [p.text, p.muted, p.accent, p.success, p.warning, p.error] {
                    for background in [p.surface, p.canvas] {
                        assert!(
                            contrast(foreground, background) >= 4.5,
                            "{foreground:?} on {background:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn primary_action_text_meets_aa_for_every_theme_and_accent() {
        for dark in [false, true] {
            for accent in [Accent::Blue, Accent::Teal, Accent::Amber] {
                let colors = palette(dark, accent);
                assert!(contrast(colors.on_accent, colors.accent) >= 4.5);
            }
        }
    }

    #[test]
    fn interactive_text_remains_readable_in_each_active_state() {
        for theme in [Theme::Light, Theme::Dark] {
            for accent in [Accent::Blue, Accent::Teal, Accent::Amber] {
                let ctx = egui::Context::default();
                apply(
                    &ctx,
                    &Preferences {
                        theme,
                        accent,
                        ..Default::default()
                    },
                );
                let style = ctx.global_style();
                for state in [
                    style.visuals.widgets.inactive,
                    style.visuals.widgets.hovered,
                    style.visuals.widgets.active,
                    style.visuals.widgets.open,
                ] {
                    assert!(contrast(state.fg_stroke.color, state.bg_fill) >= 4.5);
                }
            }
        }
    }

    #[test]
    fn density_changes_spacing_without_shrinking_controls_below_target_size() {
        let ctx = egui::Context::default();
        for (density, margin) in [(Density::Compact, 16), (Density::Comfortable, 20)] {
            apply(
                &ctx,
                &Preferences {
                    density,
                    ..Default::default()
                },
            );
            let style = ctx.global_style();
            assert_eq!(style.spacing.window_margin.left, margin);
            assert!(style.spacing.interact_size.y >= 32.0);
        }
    }
}
