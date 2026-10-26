//! Colours and egui style. Dark by default, like a calm writing desk.

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Visuals};
use notebook_core::Rgba;

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub sidebar: Color32,
    pub panel: Color32,
    pub raised: Color32,
    pub hover: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub accent: Color32,
    pub danger: Color32,
    pub desk: Color32,
}

pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x1e, 0x1e, 0x20),
    sidebar: Color32::from_rgb(0x19, 0x19, 0x1b),
    panel: Color32::from_rgb(0x26, 0x26, 0x29),
    raised: Color32::from_rgb(0x2f, 0x2f, 0x33),
    hover: Color32::from_rgb(0x38, 0x38, 0x3d),
    text: Color32::from_rgb(0xf2, 0xf2, 0xf4),
    muted: Color32::from_rgb(0x8e, 0x8e, 0x93),
    line: Color32::from_rgb(0x3a, 0x3a, 0x3f),
    accent: Color32::from_rgb(0x0a, 0x84, 0xff),
    danger: Color32::from_rgb(0xff, 0x45, 0x3a),
    desk: Color32::from_rgb(0x14, 0x14, 0x16),
};

pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xf6, 0xf5, 0xf2),
    sidebar: Color32::from_rgb(0xee, 0xed, 0xea),
    panel: Color32::from_rgb(0xff, 0xff, 0xff),
    raised: Color32::from_rgb(0xff, 0xff, 0xff),
    hover: Color32::from_rgb(0xe6, 0xe5, 0xe1),
    text: Color32::from_rgb(0x1d, 0x1d, 0x1f),
    muted: Color32::from_rgb(0x6e, 0x6e, 0x73),
    line: Color32::from_rgb(0xdd, 0xdb, 0xd6),
    accent: Color32::from_rgb(0x00, 0x7a, 0xff),
    danger: Color32::from_rgb(0xd9, 0x30, 0x25),
    desk: Color32::from_rgb(0xe3, 0xe1, 0xdc),
};

pub fn palette(dark: bool) -> Palette {
    if dark { DARK } else { LIGHT }
}

pub fn c32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0, c.1, c.2, c.3)
}

pub fn rgba(c: Color32) -> Rgba {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Rgba(r, g, b, a)
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = p.bg;
    v.window_fill = p.raised;
    v.extreme_bg_color = p.panel;
    v.faint_bg_color = p.panel;
    v.window_stroke = Stroke::new(1.0, p.line);
    v.window_corner_radius = CornerRadius::same(14);
    v.menu_corner_radius = CornerRadius::same(12);
    v.selection.bg_fill = p.accent;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.accent;
    v.override_text_color = Some(p.text);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(10);
        w.fg_stroke.color = p.text;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    v.widgets.inactive.weak_bg_fill = p.raised;
    // bg_fill is also the slider rail and checkbox box, so it must contrast with popups.
    v.widgets.inactive.bg_fill = p.hover;
    v.slider_trailing_fill = true;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.active.weak_bg_fill = p.hover;
    v.widgets.active.bg_fill = p.accent;
    v.widgets.active.bg_stroke = Stroke::NONE;
    ctx.set_visuals(v);

    ctx.all_styles_mut(|s| {
        s.text_styles = [
            (
                TextStyle::Small,
                FontId::new(12.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(15.5, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(15.5, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(28.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(14.0, FontFamily::Monospace),
            ),
        ]
        .into();
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 7.0);
        s.spacing.interact_size = egui::vec2(40.0, 34.0);
        s.spacing.menu_margin = Margin::same(8);
        s.spacing.window_margin = Margin::same(16);
        s.spacing.slider_width = 200.0;
        s.interaction.tooltip_delay = 0.6;
    });
}
