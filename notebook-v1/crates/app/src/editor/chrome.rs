//! Toolbars, tool panels, menus and keyboard shortcuts around the canvas.

use super::mesh::{place, samples_mesh};
use super::*;
use crate::icons::{self, Icon};
use crate::prefs::{HL_SIZES_MM, PALETTE, PEN_SIZES_MM};
use crate::textures;
use crate::theme::{self, Palette, c32};
use egui::{
    Align, Align2, Color32, CornerRadius, Frame, Id as EId, Key, Layout, Margin, Order, RichText,
    Sense, Stroke as EStroke, Ui,
};
use notebook_core::ink;

fn section(ui: &mut Ui, title: &str, pal: &Palette) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).strong().size(14.0).color(pal.muted));
}

fn toggle_row(ui: &mut Ui, label: &str, value: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            changed = ui.add(egui::Checkbox::without_text(value)).changed();
        });
    });
    changed
}

/// A row of colour dots; returns the picked colour. Right-click/long-press removes a favourite.
fn swatch_row(
    ui: &mut Ui,
    colors: &mut Vec<Rgba>,
    current: Rgba,
    pal: &Palette,
    removable: bool,
) -> Option<Rgba> {
    let mut picked = None;
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (i, c) in colors.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
            ui.painter().circle_filled(r.center(), 13.0, c32(*c));
            ui.painter()
                .circle_stroke(r.center(), 13.5, EStroke::new(1.0, pal.line));
            if *c == current {
                ui.painter()
                    .circle_stroke(r.center(), 16.5, EStroke::new(2.5, pal.accent));
            }
            if resp.clicked() {
                picked = Some(*c);
            }
            if removable && colors.len() > 1 {
                resp.context_menu(|ui| {
                    if ui.button("Remove colour").clicked() {
                        remove = Some(i);
                        ui.close();
                    }
                });
            }
        }
    });
    if let Some(i) = remove {
        colors.remove(i);
    }
    picked
}

fn palette_grid(ui: &mut Ui, current: Rgba, pal: &Palette) -> Option<Rgba> {
    let mut picked = None;
    egui::Grid::new(ui.next_auto_id())
        .spacing(vec2(4.0, 4.0))
        .show(ui, |ui| {
            for (i, hex) in PALETTE.iter().enumerate() {
                let c = Rgba::from_hex(hex).unwrap_or(Rgba::BLACK);
                let (r, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
                ui.painter().rect_filled(r.shrink(1.0), 6.0, c32(c));
                ui.painter().rect_stroke(
                    r.shrink(1.0),
                    6.0,
                    EStroke::new(1.0, pal.line),
                    egui::StrokeKind::Inside,
                );
                if c == current {
                    ui.painter().rect_stroke(
                        r,
                        7.0,
                        EStroke::new(2.5, pal.accent),
                        egui::StrokeKind::Outside,
                    );
                }
                if resp.clicked() {
                    picked = Some(c);
                }
                if i % 8 == 7 {
                    ui.end_row();
                }
            }
        });
    picked
}

fn custom_color(ui: &mut Ui, current: Rgba) -> Option<Rgba> {
    let mut c = theme::c32(current);
    let mut out = None;
    ui.horizontal(|ui| {
        ui.label("Custom");
        if ui.color_edit_button_srgba(&mut c).changed() {
            out = Some(theme::rgba(c));
        }
    });
    out
}

impl Editor {
    pub fn ui(&mut self, ui: &mut Ui, prefs: &mut Prefs, clipboard: &mut Vec<Item>) {
        let pal = theme::palette(prefs.dark_ui);
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx, prefs, clipboard);

        if !self.fullscreen {
            egui::Panel::top("editor-top")
                .exact_size(50.0)
                .frame(
                    Frame::new()
                        .fill(pal.sidebar)
                        .inner_margin(Margin::symmetric(8, 4)),
                )
                .show(ui, |ui| self.top_bar(ui, prefs, &pal));
            egui::Panel::left("editor-tools")
                .exact_size(58.0)
                .resizable(false)
                .frame(
                    Frame::new()
                        .fill(pal.sidebar)
                        .inner_margin(Margin::symmetric(8, 8)),
                )
                .show(ui, |ui| self.tool_bar(ui, prefs, clipboard, &pal));
            if self.sidebar {
                egui::Panel::right("editor-side")
                    .default_size(190.0)
                    .resizable(true)
                    .frame(Frame::new().fill(pal.sidebar).inner_margin(Margin::same(8)))
                    .show(ui, |ui| self.sidebar_ui(ui, prefs, &pal));
            }
        }
        egui::CentralPanel::no_frame().show(ui, |ui| self.canvas_ui(ui, prefs, clipboard));

        self.floating_controls(&ctx, prefs, &pal);
        self.popover_ui(&ctx, prefs, clipboard, &pal);
        if self.overview {
            self.overview_ui(&ctx, prefs, &pal);
        }
        self.dialogs(&ctx);
    }

    fn top_bar(&mut self, ui: &mut Ui, prefs: &mut Prefs, pal: &Palette) {
        ui.horizontal_centered(|ui| {
            if icons::button(ui, Icon::Back, false, "Library").clicked() {
                self.finish_text();
                self.events.push(EditorEvent::Close);
            }
            ui.add_space(6.0);
            let title =
                egui::Button::new(RichText::new(&self.title).strong().size(16.0)).frame(false);
            if ui.add(title).on_hover_text("Rename").clicked() {
                self.rename = Some(self.title.clone());
            }
            ui.label(RichText::new(self.page_count_label()).color(pal.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let more = icons::button(ui, Icon::More, self.popover == Popover::More, "More");
                if more.clicked() {
                    self.toggle_popover(Popover::More, more.rect);
                }
                if icons::button(ui, Icon::Fullscreen, false, "Focus mode").clicked() {
                    self.fullscreen = true;
                }
                if icons::button(ui, Icon::Sidebar, self.sidebar, "Pages").clicked() {
                    self.sidebar = !self.sidebar;
                }
                if let Some(b) = &self.doc.pages[self.current].bookmark {
                    ui.label(RichText::new(format!("🔖 {b}")).color(pal.muted));
                }
                let _ = prefs;
            });
        });
    }

    fn toggle_popover(&mut self, p: Popover, anchor: Rect) {
        self.popover = if self.popover == p { Popover::None } else { p };
        self.popover_anchor = anchor;
    }

    fn tool_bar(
        &mut self,
        ui: &mut Ui,
        prefs: &mut Prefs,
        clipboard: &mut Vec<Item>,
        pal: &Palette,
    ) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                if icons::button(ui, Icon::Grid, self.overview, "All pages").clicked() {
                    self.overview = !self.overview;
                }
                ui.add_space(4.0);
                let tools = [
                    (Tool::Pen, Icon::Pen, "Pen (P)"),
                    (Tool::Highlighter, Icon::Highlighter, "Highlighter (H)"),
                    (Tool::Eraser, Icon::Eraser, "Eraser (E)"),
                    (Tool::Lasso, Icon::Lasso, "Lasso (L)"),
                    (Tool::Hand, Icon::Hand, "Hand (Space)"),
                    (Tool::Laser, Icon::Laser, "Laser pointer"),
                    (Tool::Shape, Icon::Shapes, "Shapes (S)"),
                    (Tool::Text, Icon::Text, "Text (T)"),
                ];
                for (t, ic, tip) in tools {
                    let active = self.tool == t;
                    let r = icons::button(ui, ic, active, tip);
                    if r.clicked() {
                        if active {
                            self.toggle_popover(Popover::ToolSettings(t), r.rect);
                        } else {
                            self.set_tool(t);
                            self.popover = Popover::None;
                        }
                    }
                    if r.secondary_clicked() || r.long_touched() {
                        self.set_tool(t);
                        self.toggle_popover(Popover::ToolSettings(t), r.rect);
                    }
                }
                ui.add_space(4.0);
                let ins = icons::button(
                    ui,
                    Icon::PlusCircle,
                    self.popover == Popover::Insert,
                    "Insert",
                );
                if ins.clicked() {
                    self.toggle_popover(Popover::Insert, ins.rect);
                }
                if ui
                    .add_enabled_ui(self.history.can_undo(), |ui| {
                        icons::button(ui, Icon::Undo, false, "Undo (⌘Z)")
                    })
                    .inner
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled_ui(self.history.can_redo(), |ui| {
                        icons::button(ui, Icon::Redo, false, "Redo (⇧⌘Z)")
                    })
                    .inner
                    .clicked()
                {
                    self.redo();
                }
                ui.add_space(6.0);
                // Current colour; tap opens the colour panel.
                let color = match self.tool {
                    Tool::Highlighter => prefs.hl_color,
                    Tool::Shape => prefs.shape_color,
                    Tool::Text => prefs.text_color,
                    Tool::Laser => prefs.laser_color,
                    _ => prefs.pen_color,
                };
                let (r, resp) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
                ui.painter().circle_filled(r.center(), 14.0, c32(color));
                ui.painter()
                    .circle_stroke(r.center(), 16.0, EStroke::new(2.0, pal.accent));
                if resp.on_hover_text("Colour").clicked() {
                    self.toggle_popover(Popover::Color, r);
                }
                // Quick favourites for the pen.
                if matches!(self.tool, Tool::Pen | Tool::Highlighter) {
                    let favs = if self.tool == Tool::Pen {
                        prefs.swatches.clone()
                    } else {
                        prefs.hl_swatches.clone()
                    };
                    for c in favs.iter().take(4) {
                        let (r, resp) = ui.allocate_exact_size(vec2(40.0, 30.0), Sense::click());
                        ui.painter().circle_filled(r.center(), 9.0, c32(*c));
                        if resp.clicked() {
                            if self.tool == Tool::Pen {
                                prefs.pen_color = *c;
                            } else {
                                prefs.hl_color = *c;
                            }
                        }
                    }
                }
                let _ = clipboard;
            });
        });
    }

    fn floating_controls(&mut self, ctx: &egui::Context, prefs: &mut Prefs, pal: &Palette) {
        let c = self.canvas;
        if !c.is_positive() {
            return;
        }
        if self.fullscreen {
            egui::Area::new(EId::unique("exit-focus"))
                .order(Order::Foreground)
                .fixed_pos(c.left_top() + vec2(12.0, 12.0))
                .show(ctx, |ui| {
                    Frame::popup(ui.style())
                        .corner_radius(12)
                        .inner_margin(4)
                        .show(ui, |ui| {
                            if icons::button(ui, Icon::Fullscreen, true, "Exit focus mode")
                                .clicked()
                            {
                                self.fullscreen = false;
                            }
                        });
                });
        }
        egui::Area::new(EId::unique("zoom-ctl"))
            .order(Order::Foreground)
            .pivot(Align2::RIGHT_BOTTOM)
            .fixed_pos(c.right_bottom() - vec2(14.0, 14.0))
            .show(ctx, |ui| {
                Frame::popup(ui.style())
                    .corner_radius(14)
                    .inner_margin(4)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            if self.ruler.is_some() {
                                for (label, deg) in [("−15°", -15f32), ("+15°", 15.0)] {
                                    let btn = egui::Button::new(RichText::new(label).size(13.0))
                                        .wrap_mode(egui::TextWrapMode::Extend)
                                        .min_size(vec2(40.0, 30.0));
                                    if ui.add(btn).clicked()
                                        && let Some(r) = &mut self.ruler
                                    {
                                        r.angle += deg.to_radians();
                                    }
                                }
                            }
                            if icons::button(ui, Icon::Ruler, self.ruler.is_some(), "Ruler")
                                .clicked()
                            {
                                self.toggle_ruler();
                            }
                            if icons::button(ui, Icon::ZoomIn, false, "Zoom in").clicked() {
                                self.zoom_at(c.center(), self.zoom * 1.25);
                            }
                            let label = egui::Button::new(
                                RichText::new(format!(
                                    "{:.0}%",
                                    self.zoom / self.fit_zoom() * 100.0
                                ))
                                .size(12.0),
                            )
                            .frame(false);
                            if ui.add(label).on_hover_text("Fit width").clicked() {
                                let page = self.current;
                                self.fit_width();
                                self.scroll_to_page(page);
                            }
                            if icons::button(ui, Icon::ZoomOut, false, "Zoom out").clicked() {
                                self.zoom_at(c.center(), self.zoom / 1.25);
                            }
                        });
                    });
            });
        let _ = (prefs, pal);
    }

    pub fn fit_zoom(&self) -> f32 {
        ((self.canvas.width() - 32.0) / self.max_w).clamp(MIN_ZOOM, 2.2)
    }

    pub fn toggle_ruler(&mut self) {
        if self.ruler.is_some() {
            self.ruler = None;
        } else {
            let c = self.screen_to_doc(self.canvas.center());
            self.ruler = Some(Ruler {
                center: c,
                angle: 0.0,
            });
        }
    }

    fn popover_ui(
        &mut self,
        ctx: &egui::Context,
        prefs: &mut Prefs,
        clipboard: &mut Vec<Item>,
        pal: &Palette,
    ) {
        if self.popover == Popover::None {
            return;
        }
        let a = self.popover_anchor;
        let (pos, pivot) = match self.popover {
            Popover::More => (a.left_bottom() + vec2(a.width(), 6.0), Align2::RIGHT_TOP),
            _ => (pos2(a.right() + 10.0, a.top() - 8.0), Align2::LEFT_TOP),
        };
        let resp = egui::Area::new(EId::unique("editor-popover"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .pivot(pivot)
            .constrain(true)
            .show(ctx, |ui| {
                Frame::popup(ui.style())
                    .corner_radius(16)
                    .inner_margin(14)
                    .fill(pal.raised)
                    .show(ui, |ui| {
                        ui.set_width(300.0);
                        match self.popover {
                            Popover::ToolSettings(t) => self.tool_panel(ui, t, prefs, pal),
                            Popover::Color => self.color_panel(ui, prefs, pal),
                            Popover::Insert => self.insert_menu(ui, clipboard),
                            Popover::More => self.more_menu(ui, prefs),
                            Popover::Template => self.template_panel(ui, prefs, pal),
                            Popover::None => {}
                        }
                    });
            })
            .response;
        // Close when tapping elsewhere (the canvas also closes it on pen-down).
        let clicked_outside = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !resp.rect.contains(p) && !a.contains(p))
        });
        if clicked_outside {
            self.popover = Popover::None;
        }
    }

    fn pen_preview(&self, ui: &mut Ui, prefs: &Prefs, pal: &Palette, hl: bool) {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 70.0), Sense::hover());
        ui.painter()
            .rect_filled(r, 12.0, if prefs.dark_ui { pal.hover } else { pal.bg });
        let points: Vec<InkPoint> = (0..=60)
            .map(|i| {
                let t = i as f32 / 60.0;
                InkPoint {
                    x: 20.0 + t * (r.width() - 40.0),
                    y: 35.0 - (t * 6.0).sin() * 14.0,
                    p: 0.25 + 0.75 * (t * std::f32::consts::PI).sin(),
                }
            })
            .collect();
        let st = if hl {
            Stroke {
                pen: PenKind::Highlighter,
                color: prefs.hl_color,
                width: mm(prefs.hl_mm),
                points,
            }
        } else {
            Stroke {
                pen: prefs.pen_kind,
                color: prefs.pen_color,
                width: mm(prefs.pen_mm),
                points,
            }
        };
        let paper = if prefs.dark_ui {
            Paper::Night
        } else {
            Paper::White
        };
        let m = samples_mesh(&ink::samples(&st), c32(ink::ink_color(&st, paper)));
        ui.painter().with_clip_rect(r).add(place(&m, r.min, 1.4));
    }

    fn size_row(ui: &mut Ui, sizes: &[f32], value: &mut f32, pal: &Palette) {
        ui.horizontal(|ui| {
            for s in sizes {
                let (r, resp) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::click());
                let selected = (*value - s).abs() < 0.01;
                if selected {
                    ui.painter().rect_stroke(
                        r.shrink(2.0),
                        10.0,
                        EStroke::new(2.0, pal.accent),
                        egui::StrokeKind::Inside,
                    );
                }
                let dot = (mm(*s) * 1.3).clamp(2.0, 14.0);
                ui.painter()
                    .circle_filled(r.center() - vec2(0.0, 8.0), dot / 2.0 + 1.0, pal.text);
                ui.painter().text(
                    r.center() + vec2(0.0, 12.0),
                    Align2::CENTER_CENTER,
                    format!("{s}mm"),
                    egui::FontId::proportional(11.0),
                    pal.muted,
                );
                if resp.clicked() {
                    *value = *s;
                }
            }
        });
    }

    fn tool_panel(&mut self, ui: &mut Ui, t: Tool, prefs: &mut Prefs, pal: &Palette) {
        match t {
            Tool::Pen => {
                ui.vertical_centered(|ui| ui.heading(RichText::new("Pen").size(18.0)));
                self.pen_preview(ui, prefs, pal, false);
                section(ui, "Pen", pal);
                ui.horizontal(|ui| {
                    for (k, name) in [
                        (PenKind::Fineliner, "Fineliner"),
                        (PenKind::Fountain, "Fountain"),
                        (PenKind::Brush, "Brush"),
                    ] {
                        if ui.selectable_label(prefs.pen_kind == k, name).clicked() {
                            prefs.pen_kind = k;
                        }
                    }
                });
                section(ui, "Size", pal);
                Self::size_row(ui, &PEN_SIZES_MM, &mut prefs.pen_mm, pal);
                ui.add(
                    egui::Slider::new(&mut prefs.pen_mm, 0.2..=6.0)
                        .suffix(" mm")
                        .step_by(0.05),
                );
                section(ui, "Colour", pal);
                let cur = prefs.pen_color;
                if let Some(c) = swatch_row(ui, &mut prefs.swatches, cur, pal, true) {
                    prefs.pen_color = c;
                }
                section(ui, "Feel", pal);
                ui.horizontal(|ui| {
                    ui.label("Smoothing");
                    ui.add(
                        egui::Slider::new(&mut prefs.stabilization, 0.0..=1.0).show_value(false),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Pressure");
                    ui.add(
                        egui::Slider::new(&mut prefs.pressure_gain, 0.5..=2.0).show_value(false),
                    );
                });
                toggle_row(ui, "Hold to draw shapes", &mut prefs.shape_hold);
            }
            Tool::Highlighter => {
                ui.vertical_centered(|ui| ui.heading(RichText::new("Highlighter").size(18.0)));
                self.pen_preview(ui, prefs, pal, true);
                section(ui, "Size", pal);
                Self::size_row(ui, &HL_SIZES_MM, &mut prefs.hl_mm, pal);
                section(ui, "Colour", pal);
                let cur = prefs.hl_color;
                if let Some(c) = swatch_row(ui, &mut prefs.hl_swatches, cur, pal, true) {
                    prefs.hl_color = c;
                }
                toggle_row(ui, "Straight lines", &mut prefs.hl_straight);
            }
            Tool::Eraser => {
                ui.vertical_centered(|ui| ui.heading(RichText::new("Eraser").size(18.0)));
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(!prefs.eraser_precise, "Whole stroke")
                        .clicked()
                    {
                        prefs.eraser_precise = false;
                    }
                    if ui
                        .selectable_label(prefs.eraser_precise, "Precise")
                        .clicked()
                    {
                        prefs.eraser_precise = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Size");
                    ui.add(egui::Slider::new(&mut prefs.eraser_size, 4.0..=60.0).show_value(false));
                });
                toggle_row(ui, "Erase highlighter only", &mut prefs.eraser_hl_only);
                ui.add_space(4.0);
                if ui
                    .button(RichText::new("Clear this page").color(pal.danger))
                    .clicked()
                {
                    self.clear_page(self.current);
                    self.popover = Popover::None;
                }
            }
            Tool::Lasso => {
                ui.vertical_centered(|ui| ui.heading(RichText::new("Lasso").size(18.0)));
                ui.label(RichText::new("Circle ink, shapes or text to move, resize, recolour, copy or delete it. Tap an item to pick it alone.").color(pal.muted));
                if ui.button("Select everything on this page").clicked() {
                    self.select_all();
                    self.popover = Popover::None;
                }
            }
            Tool::Shape => {
                ui.vertical_centered(|ui| ui.heading(RichText::new("Shapes").size(18.0)));
                ui.horizontal_wrapped(|ui| {
                    for (k, name) in [
                        (ShapeKind::Line, "Line"),
                        (ShapeKind::Arrow, "Arrow"),
                        (ShapeKind::Rect, "Rectangle"),
                        (ShapeKind::Ellipse, "Ellipse"),
                    ] {
                        if ui.selectable_label(prefs.shape_kind == k, name).clicked() {
                            prefs.shape_kind = k;
                        }
                    }
                });
                ui.add(egui::Slider::new(&mut prefs.shape_mm, 0.2..=4.0).suffix(" mm"));
                let cur = prefs.shape_color;
                if let Some(c) = swatch_row(ui, &mut prefs.swatches, cur, pal, false) {
                    prefs.shape_color = c;
                }
                toggle_row(ui, "Fill", &mut prefs.shape_fill);
                toggle_row(ui, "Dashed", &mut prefs.shape_dashed);
                ui.label(
                    RichText::new("Tip: with the pen, draw a shape and hold still to snap it.")
                        .color(pal.muted)
                        .size(13.0),
                );
            }
            Tool::Text => {
                ui.vertical_centered(|ui| ui.heading(RichText::new("Text").size(18.0)));
                ui.add(egui::Slider::new(&mut prefs.text_size, 8.0..=72.0).text("Size"));
                let cur = prefs.text_color;
                if let Some(c) = swatch_row(ui, &mut prefs.swatches, cur, pal, false) {
                    prefs.text_color = c;
                }
                ui.label(
                    RichText::new("Tap the page to add a text box. Tap existing text to edit it.")
                        .color(pal.muted)
                        .size(13.0),
                );
            }
            Tool::Laser => {
                ui.vertical_centered(|ui| ui.heading(RichText::new("Laser pointer").size(18.0)));
                ui.label(
                    RichText::new(
                        "Point at things while presenting. The trail fades and is never saved.",
                    )
                    .color(pal.muted),
                );
                for hex in ["#ff3b30", "#34c759", "#0a84ff", "#ffcc00"] {
                    let c = Rgba::from_hex(hex).unwrap_or(Rgba::BLACK);
                    if ui
                        .selectable_label(
                            prefs.laser_color == c,
                            RichText::new("●  ").color(c32(c)),
                        )
                        .clicked()
                    {
                        prefs.laser_color = c;
                    }
                }
            }
            Tool::Hand => {
                ui.vertical_centered(|ui| ui.heading(RichText::new("Hand").size(18.0)));
                ui.label(
                    RichText::new("Scroll pages with the Pencil or mouse without writing.")
                        .color(pal.muted),
                );
            }
        }
    }

    fn color_panel(&mut self, ui: &mut Ui, prefs: &mut Prefs, pal: &Palette) {
        let target: &mut Rgba = match self.tool {
            Tool::Highlighter => &mut prefs.hl_color,
            Tool::Shape => &mut prefs.shape_color,
            Tool::Text => &mut prefs.text_color,
            Tool::Laser => &mut prefs.laser_color,
            _ => &mut prefs.pen_color,
        };
        let cur = *target;
        ui.label(RichText::new("Colour").strong());
        if let Some(c) = palette_grid(ui, cur, pal) {
            *target = c;
        }
        if let Some(c) = custom_color(ui, cur) {
            *target = c;
        }
        let chosen = *target;
        ui.separator();
        ui.label(RichText::new("Favourites").color(pal.muted));
        let hl = self.tool == Tool::Highlighter;
        let favs = if hl {
            &mut prefs.hl_swatches
        } else {
            &mut prefs.swatches
        };
        if let Some(c) = swatch_row(ui, favs, chosen, pal, true) {
            match self.tool {
                Tool::Highlighter => prefs.hl_color = c,
                Tool::Shape => prefs.shape_color = c,
                Tool::Text => prefs.text_color = c,
                Tool::Laser => prefs.laser_color = c,
                _ => prefs.pen_color = c,
            }
        }
        let favs = if hl {
            &mut prefs.hl_swatches
        } else {
            &mut prefs.swatches
        };
        if !favs.contains(&chosen) && ui.button("Add to favourites").clicked() {
            favs.push(chosen);
        }
    }

    fn insert_menu(&mut self, ui: &mut Ui, clipboard: &mut Vec<Item>) {
        ui.label(RichText::new("Insert").strong());
        if ui.button("New page after this one").clicked() {
            self.insert_page(self.current);
            self.popover = Popover::None;
        }
        if ui.button("Text box").clicked() {
            self.set_tool(Tool::Text);
            let c = self.visible_center(self.current);
            let prefs_size = 16.0;
            let _ = prefs_size;
            self.popover = Popover::None;
            self.toast("Tap the page where the text should go");
            let _ = c;
        }
        if ui.button("Shapes").clicked() {
            self.set_tool(Tool::Shape);
            self.popover = Popover::None;
        }
        if ui.button("Ruler").clicked() {
            self.toggle_ruler();
            self.popover = Popover::None;
        }
        if ui
            .add_enabled(!clipboard.is_empty(), egui::Button::new("Paste"))
            .clicked()
        {
            self.paste(clipboard);
            self.popover = Popover::None;
        }
    }

    fn more_menu(&mut self, ui: &mut Ui, prefs: &mut Prefs) {
        let i = self.current;
        ui.label(RichText::new(format!("Page {}", i + 1)).strong());
        if ui.button("Template and paper…").clicked() {
            self.popover = Popover::Template;
            return;
        }
        let bm = self.doc.pages[i].bookmark.clone();
        match bm {
            None => {
                if ui.button("🔖 Bookmark page").clicked() {
                    self.bookmark_edit = Some((i, format!("Page {}", i + 1)));
                    self.popover = Popover::None;
                }
            }
            Some(name) => {
                if ui.button("Rename bookmark").clicked() {
                    self.bookmark_edit = Some((i, name));
                    self.popover = Popover::None;
                }
                if ui.button("Remove bookmark").clicked() {
                    self.set_bookmark(i, None);
                }
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Duplicate").clicked() {
                self.duplicate_page(i);
            }
            if ui
                .add_enabled(i > 0, egui::Button::new("Move up"))
                .clicked()
            {
                self.move_page(i, false);
            }
            if ui
                .add_enabled(i + 1 < self.doc.pages.len(), egui::Button::new("Move down"))
                .clicked()
            {
                self.move_page(i, true);
            }
        });
        if ui.button("Delete page").clicked() {
            self.delete_page(i);
            self.popover = Popover::None;
        }
        ui.separator();
        ui.label(RichText::new("Notebook").strong());
        if ui.button("Rename…").clicked() {
            self.rename = Some(self.title.clone());
            self.popover = Popover::None;
        }
        if ui.button("Export PDF").clicked() {
            self.events.push(EditorEvent::ExportPdf);
            self.popover = Popover::None;
        }
        if ui.button("Export page as PNG").clicked() {
            self.events.push(EditorEvent::ExportPng(i));
            self.popover = Popover::None;
        }
        ui.separator();
        ui.label(RichText::new("Settings").strong());
        toggle_row(
            ui,
            "Only Pencil writes (fingers scroll)",
            &mut prefs.pencil_only,
        );
        toggle_row(ui, "Two-finger tap to undo", &mut prefs.two_finger_undo);
        toggle_row(ui, "Hold to draw shapes", &mut prefs.shape_hold);
        toggle_row(ui, "Dark interface", &mut prefs.dark_ui);
    }

    fn template_panel(&mut self, ui: &mut Ui, prefs: &mut Prefs, pal: &Palette) {
        let cur = &self.doc.pages[self.current];
        let (mut t, mut p) = (cur.template, cur.paper);
        ui.label(RichText::new("Template").strong());
        let mut changed = textures::template_picker(ui, &mut t, p, pal);
        ui.label(RichText::new("Paper").strong());
        ui.horizontal(|ui| {
            for paper in Paper::ALL {
                let (r, resp) = ui.allocate_exact_size(vec2(56.0, 40.0), Sense::click());
                ui.painter()
                    .rect_filled(r.shrink(3.0), 8.0, c32(paper.background()));
                ui.painter().rect_stroke(
                    r.shrink(3.0),
                    8.0,
                    EStroke::new(
                        if p == paper { 2.5 } else { 1.0 },
                        if p == paper { pal.accent } else { pal.line },
                    ),
                    egui::StrokeKind::Inside,
                );
                if resp.on_hover_text(paper.name()).clicked() {
                    p = paper;
                    changed = true;
                }
            }
        });
        if changed {
            self.set_template(t, p, false);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Apply to all pages").clicked() {
                let cur = &self.doc.pages[self.current];
                let (t, p) = (cur.template, cur.paper);
                self.set_template(t, p, true);
                self.popover = Popover::None;
            }
            if ui.button("Use for new notebooks").clicked() {
                let cur = &self.doc.pages[self.current];
                prefs.new_template = cur.template;
                prefs.new_paper = cur.paper;
                self.toast("Saved as default paper");
            }
        });
        let _ = CornerRadius::ZERO;
        let _ = Color32::WHITE;
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut name) = self.rename.take() {
            let mut keep = true;
            egui::Modal::new(EId::unique("rename-nb")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.heading("Rename notebook");
                let r = ui.text_edit_singleline(&mut name);
                r.request_focus();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        keep = false;
                    }
                    if ui.button("Save").clicked()
                        || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                    {
                        let n = name.trim();
                        if !n.is_empty() {
                            self.title = n.to_string();
                            self.touch();
                        }
                        keep = false;
                    }
                });
            });
            if keep {
                self.rename = Some(name);
            }
        }
        if let Some((i, mut name)) = self.bookmark_edit.take() {
            let mut keep = true;
            egui::Modal::new(EId::unique("bookmark")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.heading("Bookmark");
                let r = ui.text_edit_singleline(&mut name);
                r.request_focus();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        keep = false;
                    }
                    if ui.button("Save").clicked()
                        || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                    {
                        let n = name.trim().to_string();
                        self.set_bookmark(
                            i,
                            Some(if n.is_empty() {
                                format!("Page {}", i + 1)
                            } else {
                                n
                            }),
                        );
                        keep = false;
                    }
                });
            });
            if keep {
                self.bookmark_edit = Some((i, name));
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context, prefs: &mut Prefs, clipboard: &mut Vec<Item>) {
        if ctx.memory(|m| m.focused().is_some()) || self.text_edit.is_some() {
            return;
        }
        let (cmd, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
        let pressed = |k: Key| ctx.input(|i| i.key_pressed(k));
        if cmd && pressed(Key::Z) {
            if shift { self.redo() } else { self.undo() }
        } else if cmd && pressed(Key::Y) {
            self.redo();
        } else if cmd && pressed(Key::C) {
            self.copy_selection(clipboard);
        } else if cmd && pressed(Key::X) {
            self.copy_selection(clipboard);
            self.delete_selection();
        } else if cmd && pressed(Key::V) {
            self.paste(clipboard);
        } else if cmd && pressed(Key::D) {
            self.duplicate_selection();
        } else if cmd && pressed(Key::A) {
            self.select_all();
        } else if !cmd {
            if pressed(Key::Delete) || pressed(Key::Backspace) {
                self.delete_selection();
            }
            if pressed(Key::Escape) {
                self.selection = None;
                self.popover = Popover::None;
                self.fullscreen = false;
            }
            for (k, t) in [
                (Key::P, Tool::Pen),
                (Key::H, Tool::Highlighter),
                (Key::E, Tool::Eraser),
                (Key::L, Tool::Lasso),
                (Key::S, Tool::Shape),
                (Key::T, Tool::Text),
            ] {
                if pressed(k) {
                    self.set_tool(t);
                }
            }
            if pressed(Key::Plus) || pressed(Key::Equals) {
                self.zoom_at(self.canvas.center(), self.zoom * 1.25);
            }
            if pressed(Key::Minus) {
                self.zoom_at(self.canvas.center(), self.zoom / 1.25);
            }
            if pressed(Key::ArrowDown) || pressed(Key::PageDown) {
                self.scroll_to_page((self.current + 1).min(self.doc.pages.len() - 1));
            }
            if pressed(Key::ArrowUp) || pressed(Key::PageUp) {
                self.scroll_to_page(self.current.saturating_sub(1));
            }
        }
        let _ = prefs;
    }
}
