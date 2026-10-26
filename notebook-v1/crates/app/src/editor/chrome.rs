//! Toolbars, tool panels, menus and keyboard shortcuts around the canvas.

use super::mesh::{place, samples_mesh};
use super::*;
use crate::icons::{self, Icon};
use crate::prefs::{HL_SIZES_MM, PALETTE, PEN_SIZES_MM, TAPE_COLORS, TAPE_SIZES_MM, TOOL_NAMES, TapeKind};
use notebook_core::presets::{self, Preset};
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
                    .default_size(250.0)
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
                    (Tool::Pen, Icon::Pen, "Pen (F)", "Pen"),
                    (Tool::Highlighter, Icon::Highlighter, "Highlighter (G)", "Highlighter"),
                    (Tool::Tape, Icon::Tape, "Tape: hide notes to test yourself", "Tape"),
                    (Tool::Eraser, Icon::Eraser, "Eraser (E)", "Eraser"),
                    (Tool::Lasso, Icon::Lasso, "Lasso (A)", "Lasso"),
                    (Tool::Hand, Icon::Hand, "Hand (H): scroll, follow links", "Hand"),
                    (Tool::Laser, Icon::Laser, "Laser pointer", "Laser"),
                    (Tool::Shape, Icon::Shapes, "Shapes (S)", "Shapes"),
                    (Tool::Text, Icon::Text, "Text (T)", "Text"),
                ];
                for (t, ic, tip, name) in tools {
                    if !prefs.tool_visible(name) && self.tool != t {
                        continue;
                    }
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
                if prefs.tool_visible("Stickers") {
                    let st = icons::button(
                        ui,
                        Icon::Sticker,
                        self.popover == Popover::Stickers,
                        "Stickers",
                    );
                    if st.clicked() {
                        self.toggle_popover(Popover::Stickers, st.rect);
                    }
                }
                if prefs.tool_visible("Insert") {
                    let ins = icons::button(
                        ui,
                        Icon::PlusCircle,
                        self.popover == Popover::Insert,
                        "Insert",
                    );
                    if ins.clicked() {
                        self.toggle_popover(Popover::Insert, ins.rect);
                    }
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
                    Tool::Tape => prefs.tape_color,
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
                    if self.tool == Tool::Tape {
                        self.toggle_popover(Popover::ToolSettings(Tool::Tape), r);
                    } else {
                        self.toggle_popover(Popover::Color, r);
                    }
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
        // Bottom-left: page number (tap = go to page), back chip after following a link.
        egui::Area::new(EId::unique("page-chip"))
            .order(Order::Foreground)
            .pivot(Align2::LEFT_BOTTOM)
            .fixed_pos(c.left_bottom() + vec2(14.0, -14.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(&back) = self.back_stack.last() {
                        let b = egui::Button::new(
                            RichText::new(format!("↶ Page {}", back + 1)).size(13.0),
                        )
                        .corner_radius(14);
                        if ui.add(b).on_hover_text("Back to where you were").clicked() {
                            self.go_back();
                        }
                    }
                    let chip = egui::Button::new(
                        RichText::new(format!(
                            "{} of {}",
                            self.current + 1,
                            self.doc.pages.len()
                        ))
                        .size(13.0),
                    )
                    .corner_radius(14);
                    let r = ui.add(chip).on_hover_text("Go to page");
                    if r.clicked() {
                        self.goto_text = (self.current + 1).to_string();
                        self.toggle_popover(Popover::GoTo, r.rect);
                    }
                });
            });
        if prefs.show_timer {
            egui::Area::new(EId::unique("study-timer"))
                .order(Order::Foreground)
                .pivot(Align2::RIGHT_TOP)
                .fixed_pos(c.right_top() + vec2(-14.0, 14.0))
                .show(ctx, |ui| {
                    Frame::popup(ui.style())
                        .corner_radius(14)
                        .inner_margin(Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let t = self.timer.seconds(self.now) as u64;
                                ui.label(
                                    RichText::new(format!(
                                        "⏱ {:02}:{:02}:{:02}",
                                        t / 3600,
                                        t / 60 % 60,
                                        t % 60
                                    ))
                                    .monospace()
                                    .size(15.0),
                                );
                                let running = self.timer.since.is_some();
                                if ui.button(if running { "Pause" } else { "Start" }).clicked() {
                                    if let Some(s) = self.timer.since.take() {
                                        self.timer.acc += self.now - s;
                                    } else {
                                        self.timer.since = Some(self.now);
                                    }
                                }
                                if ui.button("Reset").clicked() {
                                    self.timer = Timer::default();
                                }
                                if running {
                                    ui.ctx().request_repaint_after(
                                        std::time::Duration::from_millis(250),
                                    );
                                }
                            });
                        });
                });
        }
        let _ = pal;
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
            Popover::More | Popover::Customize | Popover::Template => {
                (a.left_bottom() + vec2(a.width(), 6.0), Align2::RIGHT_TOP)
            }
            Popover::GoTo => (a.left_top() - vec2(0.0, 8.0), Align2::LEFT_BOTTOM),
            Popover::ShapeStyle | Popover::MoveToLayer => {
                (a.left_bottom() + vec2(0.0, 6.0), Align2::LEFT_TOP)
            }
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
                        ui.set_width(if self.popover == Popover::GoTo { 200.0 } else { 300.0 });
                        let max_h = (ctx.content_rect().height() - 80.0).max(200.0);
                        egui::ScrollArea::vertical()
                            .max_height(max_h)
                            .min_scrolled_height(max_h)
                            .show(ui, |ui| match self.popover {
                                Popover::ToolSettings(t) => self.tool_panel(ui, t, prefs, pal),
                                Popover::Color => self.color_panel(ui, prefs, pal),
                                Popover::Insert => self.insert_menu(ui, prefs, clipboard),
                                Popover::More => self.more_menu(ui, prefs),
                                Popover::Template => self.template_panel(ui, prefs, pal),
                                Popover::Shapes => self.shapes_panel(ui, prefs, pal),
                                Popover::Stickers => self.stickers_panel(ui, prefs, pal),
                                Popover::ShapeStyle => self.shape_style_panel(ui, prefs, pal),
                                Popover::MoveToLayer => self.move_to_layer_panel(ui),
                                Popover::Customize => self.customize_panel(ui, prefs),
                                Popover::GoTo => self.goto_panel(ui),
                                Popover::None => {}
                            });
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
                toggle_row(ui, "Erase shapes", &mut prefs.eraser_shapes);
                toggle_row(ui, "Erase tape", &mut prefs.eraser_tapes);
                toggle_row(ui, "Return to previous tool", &mut prefs.eraser_return);
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
                section(ui, "Shape", pal);
                ui.horizontal(|ui| {
                    if ui.selectable_label(!prefs.lasso_rect, "Freeform").clicked() {
                        prefs.lasso_rect = false;
                    }
                    if ui.selectable_label(prefs.lasso_rect, "Rectangle").clicked() {
                        prefs.lasso_rect = true;
                    }
                });
                section(ui, "Object selection", pal);
                toggle_row(ui, "Handwriting", &mut prefs.lasso_ink);
                toggle_row(ui, "Highlights", &mut prefs.lasso_hl);
                toggle_row(ui, "Text boxes", &mut prefs.lasso_text);
                toggle_row(ui, "Images", &mut prefs.lasso_images);
                toggle_row(ui, "Shapes", &mut prefs.lasso_shapes);
                toggle_row(ui, "Tapes", &mut prefs.lasso_tapes);
                if ui.button("Select everything on this page").clicked() {
                    self.select_all();
                    self.popover = Popover::None;
                }
            }
            Tool::Tape => {
                ui.vertical_centered(|ui| ui.heading(RichText::new("Tape").size(18.0)));
                section(ui, "Tape type", pal);
                ui.horizontal(|ui| {
                    for (k, name) in [
                        (TapeKind::Line, "Straight"),
                        (TapeKind::Rect, "Rectangle"),
                        (TapeKind::Free, "Freehand"),
                    ] {
                        if ui.selectable_label(prefs.tape_kind == k, name).clicked() {
                            prefs.tape_kind = k;
                        }
                    }
                });
                if prefs.tape_kind != TapeKind::Rect {
                    section(ui, "Size", pal);
                    Self::size_row(ui, &TAPE_SIZES_MM, &mut prefs.tape_mm, pal);
                }
                section(ui, "Pattern", pal);
                ui.horizontal(|ui| {
                    for hex in TAPE_COLORS {
                        let c = Rgba::from_hex(hex).unwrap_or(Rgba::WHITE);
                        let (r, resp) = ui.allocate_exact_size(vec2(62.0, 30.0), Sense::click());
                        ui.painter().rect_filled(r.shrink(3.0), 12.0, c32(c));
                        for k in 0..6 {
                            let x = r.left() + 8.0 + k as f32 * 9.0;
                            ui.painter().line_segment(
                                [pos2(x, r.bottom() - 6.0), pos2(x + 6.0, r.top() + 6.0)],
                                EStroke::new(1.5, Color32::WHITE.gamma_multiply(0.5)),
                            );
                        }
                        if prefs.tape_color == c {
                            ui.painter().rect_stroke(
                                r,
                                14.0,
                                EStroke::new(2.0, pal.accent),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if resp.clicked() {
                            prefs.tape_color = c;
                        }
                    }
                });
                ui.add_space(6.0);
                Frame::new()
                    .fill(pal.hover)
                    .corner_radius(10)
                    .inner_margin(8)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(
                                "💡 Cover your notes with tape, then test yourself. Tap a tape to reveal it.",
                            )
                            .color(pal.muted)
                            .size(13.0),
                        );
                    });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("Cover all").clicked() {
                        self.set_all_tapes(false);
                    }
                    if ui.button("Reveal all").clicked() {
                        self.set_all_tapes(true);
                    }
                });
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
                    RichText::new(
                        "Scroll pages with the Pencil or mouse without writing. Tap links to follow them and tape to reveal it.",
                    )
                    .color(pal.muted),
                );
            }
        }
    }

    fn color_panel(&mut self, ui: &mut Ui, prefs: &mut Prefs, pal: &Palette) {
        let target: &mut Rgba = match self.tool {
            Tool::Highlighter => &mut prefs.hl_color,
            Tool::Tape => &mut prefs.tape_color,
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
                Tool::Tape => prefs.tape_color = c,
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

    fn insert_menu(&mut self, ui: &mut Ui, prefs: &mut Prefs, clipboard: &mut Vec<Item>) {
        ui.label(RichText::new("Insert").strong());
        let full = ui.available_width();
        let wide = |t: &str| egui::Button::new(t).min_size(vec2(full, 30.0));
        if ui.add(wide("📄  Page after this one")).clicked() {
            self.insert_page(self.current);
            self.popover = Popover::None;
        }
        if ui.add(wide("🖼  Image…")).clicked() {
            if crate::platform::pick_file(crate::platform::ACCEPT_IMAGE) {
                self.pending_import = Some(ImportTarget::Image);
            } else {
                self.toast("Importing pictures works in the web version");
            }
            self.popover = Popover::None;
        }
        if ui.add(wide("📥  Import PDF or photo as pages…")).clicked() {
            if crate::platform::pick_file(crate::platform::ACCEPT_ANY) {
                self.pending_import = Some(ImportTarget::Pages);
            } else {
                self.toast("Importing files works in the web version");
            }
            self.popover = Popover::None;
        }
        if ui.add(wide("T  Text box")).clicked() {
            self.set_tool(Tool::Text);
            self.popover = Popover::None;
            self.toast("Tap the page where the text should go");
        }
        if ui.add(wide("◆  Shape…")).clicked() {
            self.popover = Popover::Shapes;
        }
        if ui.add(wide("★  Sticker…")).clicked() {
            self.popover = Popover::Stickers;
        }
        if ui.add(wide("📏  Ruler")).clicked() {
            self.toggle_ruler();
            self.popover = Popover::None;
        }
        if ui
            .add_enabled(!clipboard.is_empty(), wide("📋  Paste"))
            .clicked()
        {
            self.paste(clipboard);
            self.popover = Popover::None;
        }
        let _ = prefs;
    }

    fn shapes_panel(&mut self, ui: &mut Ui, prefs: &mut Prefs, pal: &Palette) {
        ui.vertical_centered(|ui| ui.heading(RichText::new("Insert Shape").size(18.0)));
        egui::Grid::new("shape-grid")
            .spacing(vec2(6.0, 6.0))
            .show(ui, |ui| {
                for (i, p) in Preset::ALL.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(vec2(88.0, 70.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 10.0, pal.hover);
                    }
                    let item = presets::preset_item(*p, 0.0, 0.0, 48.0, prefs.fill_color);
                    let m = super::mesh::item_mesh(&item, Paper::White);
                    ui.painter().add(place(&m, r.center(), 1.0));
                    if resp.on_hover_text(p.name()).clicked() {
                        let page = self.current;
                        if !self.can_write(page) {
                            self.toast("The active layer is locked or hidden");
                        } else {
                            let c = self.visible_center(page);
                            let it = presets::preset_item(*p, c[0], c[1], 120.0, prefs.fill_color);
                            self.add_items(page, vec![it]);
                        }
                        self.popover = Popover::None;
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        section(ui, "Colour", pal);
        let cur = prefs.fill_color;
        if let Some(c) = palette_grid(ui, cur, pal) {
            prefs.fill_color = c;
        }
    }

    fn stickers_panel(&mut self, ui: &mut Ui, prefs: &mut Prefs, pal: &Palette) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Stickers").size(18.0));
        });
        if prefs.stickers.is_empty() {
            ui.label(
                RichText::new(
                    "No stickers yet. Use the Lasso to select something, then choose More ▾ › Save as sticker.",
                )
                .color(pal.muted),
            );
        }
        let mut place_it = None;
        let mut remove = None;
        egui::Grid::new("sticker-grid")
            .spacing(vec2(6.0, 6.0))
            .show(ui, |ui| {
                for (i, st) in prefs.stickers.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(vec2(88.0, 80.0), Sense::click());
                    ui.painter().rect_filled(r, 10.0, if resp.hovered() { pal.hover } else { pal.bg });
                    if let Some(b) = geom::union_bbox(st.items.iter()) {
                        let k = (70.0 / b.w.max(b.h).max(1.0)).min(1.5);
                        let origin = r.center() - vec2(b.center()[0], b.center()[1]) * k;
                        for it in &st.items {
                            if let Content::Text(t) = &it.content {
                                let pos = origin + vec2(t.x, t.y) * k;
                                ui.painter().text(
                                    pos,
                                    Align2::LEFT_TOP,
                                    &t.text,
                                    egui::FontId::proportional((t.size * k).max(6.0)),
                                    c32(t.color),
                                );
                            } else {
                                let m = super::mesh::item_mesh(it, Paper::White);
                                ui.painter().add(place(&m, origin, k));
                            }
                        }
                    }
                    if resp.clicked() {
                        place_it = Some(i);
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Delete sticker").clicked() {
                            remove = Some(i);
                            ui.close();
                        }
                    });
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        if let Some(i) = remove {
            prefs.stickers.remove(i);
        }
        if let Some(i) = place_it {
            let mut st = prefs.stickers.remove(i);
            st.used = now_ms();
            self.place_sticker(&st);
            prefs.stickers.insert(0, st);
            self.popover = Popover::None;
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Install samples").clicked() {
                let mut samples = presets::sample_stickers();
                samples.append(&mut prefs.stickers);
                prefs.stickers = samples;
                prefs.stickers.truncate(40);
            }
            ui.label(RichText::new("Right-click / long-press to delete").color(pal.muted).size(12.0));
        });
    }

    fn shape_style_panel(&mut self, ui: &mut Ui, prefs: &mut Prefs, pal: &Palette) {
        ui.vertical_centered(|ui| ui.heading(RichText::new("Shape Style").size(18.0)));
        let first = self.selected_items().into_iter().find_map(|i| match &i.content {
            Content::Shape(s) => Some(s.clone()),
            _ => None,
        });
        let Some(cur) = first else {
            ui.label("Select a shape first.");
            return;
        };
        let apply = |ed: &mut Editor, f: &dyn Fn(&mut Shape)| {
            ed.map_selection(|it| {
                let mut c = it.clone();
                if let Content::Shape(s) = &mut c.content {
                    f(s);
                }
                c
            });
        };
        section(ui, "Colour", pal);
        if let Some(c) = palette_grid(ui, cur.color, pal) {
            apply(self, &|s: &mut Shape| {
                s.color = c;
                if s.fill.is_some() {
                    s.fill = Some(c);
                }
            });
            prefs.fill_color = c;
        }
        section(ui, "Fill", pal);
        ui.horizontal(|ui| {
            let mode = if cur.fill.is_none() { 0 } else if cur.solid { 2 } else { 1 };
            for (k, name) in ["Outline", "Tinted", "Filled"].iter().enumerate() {
                if ui.selectable_label(mode == k, *name).clicked() {
                    apply(self, &|s: &mut Shape| {
                        s.fill = (k > 0).then_some(s.color);
                        s.solid = k == 2;
                    });
                }
            }
        });
        section(ui, "Border", pal);
        ui.horizontal(|ui| {
            let mode = if !cur.dashed { 0 } else if cur.dotted { 2 } else { 1 };
            for (k, name) in ["Solid", "Dashed", "Dotted"].iter().enumerate() {
                if ui.selectable_label(mode == k, *name).clicked() {
                    apply(self, &|s: &mut Shape| {
                        s.dashed = k > 0;
                        s.dotted = k == 2;
                    });
                }
            }
        });
        ui.horizontal(|ui| {
            for (w, name) in [(0.6, "Thin"), (1.5, "Medium"), (3.0, "Thick"), (5.0, "Heavy")] {
                if ui
                    .selectable_label((cur.width - w).abs() < 0.05, name)
                    .clicked()
                {
                    apply(self, &|s: &mut Shape| s.width = w);
                }
            }
        });
    }

    fn move_to_layer_panel(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Move to layer").strong());
        let Some(sel) = self.selection.clone() else {
            ui.label("Nothing selected.");
            return;
        };
        let layers = self.doc.pages[sel.page].layers.clone();
        for l in layers.iter().rev() {
            let label = if l.locked {
                format!("{} 🔒", l.name)
            } else {
                l.name.clone()
            };
            if ui
                .add_enabled(!l.locked, egui::Button::new(label).min_size(vec2(260.0, 28.0)))
                .clicked()
            {
                self.move_selection_to_layer(l.id);
                self.popover = Popover::None;
            }
        }
    }

    fn customize_panel(&mut self, ui: &mut Ui, prefs: &mut Prefs) {
        ui.label(RichText::new("Customize toolbar").strong());
        for name in TOOL_NAMES {
            let mut on = prefs.tool_visible(name);
            if toggle_row(ui, name, &mut on) {
                prefs.hidden_tools.retain(|t| t != name);
                if !on {
                    prefs.hidden_tools.push(name.to_string());
                }
            }
        }
        if ui.button("Reset").clicked() {
            prefs.hidden_tools.clear();
        }
    }

    fn goto_panel(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Go to page").strong());
        let n = self.doc.pages.len();
        let mut go = ui.input(|i| i.key_pressed(Key::Enter));
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.goto_text).desired_width(70.0));
            r.request_focus();
            ui.label(format!("of {n}"));
            if ui.button("Go").clicked() {
                go = true;
            }
        });
        ui.horizontal(|ui| {
            if ui.button("First").clicked() {
                self.goto_text = "1".into();
                go = true;
            }
            if ui.button("Last").clicked() {
                self.goto_text = n.to_string();
                go = true;
            }
        });
        if go {
            if let Ok(k) = self.goto_text.trim().parse::<usize>() {
                self.scroll_to = Some(k.clamp(1, n) - 1);
            }
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
        if ui.button("Add to outline").clicked() {
            self.add_outline();
            self.popover = Popover::None;
        }
        if ui.button("Clear page…").clicked() {
            self.clear_page(i);
            self.toast("Page cleared (undo to restore)");
            self.popover = Popover::None;
        }
        ui.separator();
        ui.label(RichText::new("Notebook").strong());
        ui.horizontal(|ui| {
            if ui.button("🔍 Find…").clicked() {
                self.edit = Some(EditDialog::Find(String::new()));
                self.popover = Popover::None;
            }
            if ui.button("Go to page…").clicked() {
                self.goto_text = (self.current + 1).to_string();
                self.popover = Popover::GoTo;
                self.popover_anchor = Rect::from_center_size(
                    self.canvas.left_bottom() + vec2(60.0, -30.0),
                    vec2(10.0, 10.0),
                );
            }
        });
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
        ui.label(RichText::new("View").strong());
        toggle_row(ui, "Night mode (dark pages)", &mut prefs.night_mode);
        toggle_row(ui, "Study timer", &mut prefs.show_timer);
        if toggle_row(ui, "Keep screen awake", &mut prefs.keep_awake) {
            crate::platform::keep_awake(prefs.keep_awake);
        }
        toggle_row(ui, "Dark interface", &mut prefs.dark_ui);
        ui.separator();
        ui.label(RichText::new("Settings").strong());
        toggle_row(
            ui,
            "Only Pencil writes (fingers scroll)",
            &mut prefs.pencil_only,
        );
        toggle_row(ui, "Two-finger tap to undo", &mut prefs.two_finger_undo);
        toggle_row(ui, "Hold to draw shapes", &mut prefs.shape_hold);
        if ui.button("Customize toolbar…").clicked() {
            self.popover = Popover::Customize;
        }
    }

    fn template_panel(&mut self, ui: &mut Ui, prefs: &mut Prefs, pal: &Palette) {
        let cur = &self.doc.pages[self.current];
        let (mut t, mut p) = (cur.template, cur.paper);
        ui.label(RichText::new("Template").strong());
        let mut changed = textures::template_picker(ui, &mut t, p, pal);
        ui.label(RichText::new("Background colour").strong());
        ui.horizontal_wrapped(|ui| {
            for paper in Paper::ALL {
                let (r, resp) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
                ui.painter()
                    .circle_filled(r.center(), 15.0, c32(paper.background()));
                ui.painter().circle_stroke(
                    r.center(),
                    if p == paper { 17.5 } else { 15.0 },
                    EStroke::new(
                        if p == paper { 2.5 } else { 1.0 },
                        if p == paper { pal.accent } else { pal.line },
                    ),
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
        ui.add_space(4.0);
        ui.label(RichText::new("Paper size").strong());
        let (cw, ch) = {
            let c = &self.doc.pages[self.current];
            (c.w, c.h)
        };
        let mut picked = None;
        ui.horizontal_wrapped(|ui| {
            for size in PaperSize::ALL {
                let (w, h) = size.dims(cw > ch);
                let on = (w - cw).abs() < 1.0 && (h - ch).abs() < 1.0;
                if ui.selectable_label(on, size.name()).clicked() {
                    picked = Some((w, h));
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.selectable_label(cw <= ch, "Portrait").clicked() && cw > ch {
                picked = Some((ch, cw));
            }
            if ui.selectable_label(cw > ch, "Landscape").clicked() && cw <= ch {
                picked = Some((ch, cw));
            }
        });
        let mut all = ui.ctx().data(|d| d.get_temp::<bool>(EId::unique("size-all")).unwrap_or(false));
        ui.checkbox(&mut all, "Apply size to all pages");
        ui.ctx().data_mut(|d| d.insert_temp(EId::unique("size-all"), all));
        if let Some((w, h)) = picked {
            self.set_page_size(w, h, all);
        }
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
        if let Some(edit) = self.edit.take() {
            self.edit_dialog(ctx, edit);
        }
    }

    fn edit_dialog(&mut self, ctx: &egui::Context, mut edit: EditDialog) {
        let mut keep = true;
        // Read Enter before any text field gets a chance to swallow it.
        let enter = ctx.input(|i| i.key_pressed(Key::Enter));
        egui::Modal::new(EId::unique("edit-dialog")).show(ctx, |ui| {
            ui.set_width(360.0);
            match &mut edit {
                EditDialog::Link { url, page, web } => {
                    ui.heading("Link Settings");
                    ui.horizontal(|ui| {
                        if ui.selectable_label(*web, "Web page").clicked() {
                            *web = true;
                        }
                        if ui.selectable_label(!*web, "Page in this notebook").clicked() {
                            *web = false;
                        }
                    });
                    if *web {
                        let r = ui.add(
                            egui::TextEdit::singleline(url)
                                .hint_text("https://example.com")
                                .desired_width(f32::INFINITY),
                        );
                        r.request_focus();
                    } else {
                        ui.horizontal(|ui| {
                            ui.label("Page number");
                            let r = ui.add(egui::TextEdit::singleline(page).desired_width(60.0));
                            r.request_focus();
                            ui.label(format!("of {}", self.doc.pages.len()));
                        });
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if ui
                            .button(RichText::new("Remove link").color(Color32::from_rgb(255, 69, 58)))
                            .clicked()
                        {
                            self.set_selection_link(None);
                            keep = false;
                        }
                        if ui.button("Save").clicked() || enter {
                            let link = if *web {
                                let u = url.trim();
                                (!u.is_empty()).then(|| {
                                    if u.contains("://") || u.starts_with("mailto:") {
                                        u.to_string()
                                    } else {
                                        format!("https://{u}")
                                    }
                                })
                                .map(Link::Url)
                            } else {
                                page.trim()
                                    .parse::<usize>()
                                    .ok()
                                    .filter(|k| *k >= 1 && *k <= self.doc.pages.len())
                                    .map(|k| Link::Page(self.doc.pages[k - 1].id))
                            };
                            match link {
                                Some(l) => {
                                    self.set_selection_link(Some(l));
                                    self.toast("Link added. Tap it with a finger or the Hand tool.");
                                    keep = false;
                                }
                                None => self.toast("Enter a web address or a valid page number"),
                            }
                        }
                    });
                }
                EditDialog::LayerName(page, id, name) => {
                    ui.heading("Rename layer");
                    let r = ui.text_edit_singleline(name);
                    r.request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if ui.button("Save").clicked() || enter {
                            let n = name.trim().to_string();
                            if !n.is_empty() && *page < self.doc.pages.len() {
                                self.update_layer(*page, *id, |l| l.name = n);
                            }
                            keep = false;
                        }
                    });
                }
                EditDialog::OutlineTitle(id, title) => {
                    ui.heading("Outline entry");
                    let r = ui.text_edit_singleline(title);
                    r.request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if ui.button("Save").clicked() || enter {
                            let t = title.trim().to_string();
                            if !t.is_empty()
                                && let Some(e) = self.doc.outline.iter_mut().find(|e| e.id == *id)
                            {
                                e.title = t;
                                self.touch();
                            }
                            keep = false;
                        }
                    });
                }
                EditDialog::Find(q) => {
                    ui.horizontal(|ui| {
                        ui.heading("Find in notebook");
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Done").clicked() {
                                keep = false;
                            }
                        });
                    });
                    let r = ui.add(
                        egui::TextEdit::singleline(q)
                            .hint_text("Typed text, bookmarks, outline")
                            .desired_width(f32::INFINITY),
                    );
                    r.request_focus();
                    let hits = self.find(q);
                    if !q.trim().is_empty() {
                        ui.label(
                            RichText::new(format!(
                                "{} match{} found",
                                hits.len(),
                                if hits.len() == 1 { "" } else { "es" }
                            ))
                            .weak(),
                        );
                    }
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for (page, snippet) in hits {
                            let b = egui::Button::new(format!("p.{}   {snippet}", page + 1))
                                .frame(false)
                                .wrap();
                            if ui.add(b).clicked() {
                                self.scroll_to = Some(page);
                                keep = false;
                            }
                        }
                    });
                    if ui.input(|i| i.key_pressed(Key::Escape)) {
                        keep = false;
                    }
                }
            }
        });
        if keep {
            self.edit = Some(edit);
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context, prefs: &mut Prefs, clipboard: &mut Vec<Item>) {
        if ctx.memory(|m| m.focused().is_some()) || self.text_edit.is_some() || self.edit.is_some()
        {
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
        } else if cmd && pressed(Key::F) {
            self.edit = Some(EditDialog::Find(String::new()));
        } else if cmd && pressed(Key::G) {
            self.goto_text = (self.current + 1).to_string();
            self.popover = Popover::GoTo;
            self.popover_anchor =
                Rect::from_center_size(self.canvas.left_bottom() + vec2(60.0, -30.0), vec2(10.0, 10.0));
        } else if cmd && pressed(Key::OpenBracket) {
            self.go_back();
        } else if cmd && pressed(Key::Num0) {
            let page = self.current;
            self.fit_width();
            self.scroll_to_page(page);
        } else if !cmd {
            if pressed(Key::Delete) || pressed(Key::Backspace) {
                self.delete_selection();
            }
            if pressed(Key::Escape) {
                self.selection = None;
                self.popover = Popover::None;
                self.fullscreen = false;
            }
            // Noteful-style keys (F G E A H T) plus the older P L S.
            for (k, t) in [
                (Key::F, Tool::Pen),
                (Key::P, Tool::Pen),
                (Key::G, Tool::Highlighter),
                (Key::E, Tool::Eraser),
                (Key::A, Tool::Lasso),
                (Key::L, Tool::Lasso),
                (Key::H, Tool::Hand),
                (Key::S, Tool::Shape),
                (Key::T, Tool::Text),
                (Key::K, Tool::Tape),
            ] {
                if pressed(k) {
                    self.set_tool(t);
                }
            }
            // 1–9: pen favourites (colours).
            let nums = [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
                Key::Num9,
            ];
            for (n, k) in nums.iter().enumerate() {
                if pressed(*k)
                    && let Some(c) = prefs.swatches.get(n).copied()
                {
                    prefs.pen_color = c;
                    self.set_tool(Tool::Pen);
                }
            }
            if pressed(Key::OpenBracket) || pressed(Key::CloseBracket) {
                // Previous / next layer on the current page.
                let page = self.current;
                let layers: Vec<Id> = self.doc.pages[page].layers.iter().map(|l| l.id).collect();
                let cur = self.active_layer_id(page);
                if let Some(pos) = layers.iter().position(|l| *l == cur) {
                    let to = if pressed(Key::CloseBracket) {
                        (pos + 1).min(layers.len() - 1)
                    } else {
                        pos.saturating_sub(1)
                    };
                    let pid = self.doc.pages[page].id;
                    self.active_layer.insert(pid, layers[to]);
                    let name = self.doc.pages[page].layers[to].name.clone();
                    self.toast(format!("Drawing on {name}"));
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
