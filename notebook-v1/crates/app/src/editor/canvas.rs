//! Draws the pages and everything on them, and hosts the text editor overlay.

use super::mesh::{place, place_transformed, samples_mesh};
use super::*;
use crate::icons::{self, Icon};
use crate::theme::{self, c32};
use egui::{
    Align2, Color32, FontId, Id as EId, Order, Sense, Shape as EShape, Stroke as EStroke,
    StrokeKind,
};
use notebook_core::ink;

/// Glyphs are rasterised per pixel size, so zoomed text uses a limited set of sizes
/// (12 steps per doubling) to keep the font atlas small during pinch-zoom.
fn quantize(size: f32) -> f32 {
    let steps = 12.0;
    2f32.powf((size.max(1.0).log2() * steps).round() / steps)
}

fn draw_text(p: &egui::Painter, origin: Pos2, zoom: f32, t: &TextBox) {
    let lh = t.size * text::LINE_HEIGHT;
    let fs = quantize(t.size * zoom);
    let color = c32(t.color);
    for (i, line) in text::wrap(&t.text, t.size, t.w).iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let pos = origin + vec2(t.x, t.y + i as f32 * lh + (lh - t.size) / 2.0) * zoom;
        p.text(pos, Align2::LEFT_TOP, line, FontId::proportional(fs), color);
    }
}

impl Editor {
    pub fn canvas_ui(&mut self, ui: &mut egui::Ui, prefs: &mut Prefs, clipboard: &mut Vec<Item>) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        self.canvas = rect;
        self.layer = ui.layer_id();
        self.now = ui.input(|i| i.time);
        self.layout();
        if !self.fitted && rect.width() > 50.0 {
            self.fit_width();
            self.offset.y = 20.0;
            self.fitted = true;
        }
        if let Some(i) = self.scroll_to.take()
            && i < self.doc.pages.len()
        {
            self.scroll_to_page(i);
        }
        self.cache.begin_frame();
        let ctx = ui.ctx().clone();
        self.handle_input(&ctx, prefs, clipboard, resp.hovered() || resp.dragged());
        self.clamp_offset();
        self.update_current();

        let pal = theme::palette(prefs.dark_ui);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, pal.desk);

        let zoom = self.zoom;
        let moving: Option<(usize, Vec<Id>)> = match (&self.action, &self.selection) {
            (Action::Move { page, .. } | Action::Scale { page, .. }, Some(sel)) => {
                Some((*page, sel.ids.clone()))
            }
            _ => None,
        };

        for i in 0..self.doc.pages.len() {
            let pr = self.page_rect(i);
            if !pr.intersects(rect) {
                continue;
            }
            let page = self.doc.pages[i].clone();
            painter.rect_filled(
                pr.translate(vec2(0.0, 3.0)).expand(1.5),
                3.0,
                Color32::from_black_alpha(if prefs.dark_ui { 120 } else { 40 }),
            );
            let clip = pr.intersect(rect);
            let pp = painter.with_clip_rect(clip);
            pp.rect_filled(pr, 0.0, c32(page.paper.background()));
            pp.add(place(self.cache.paper(&page), pr.min, zoom));

            let mut exclude: Vec<Id> = Vec::new();
            if let Some((mp, ids)) = &moving
                && *mp == i
            {
                exclude.extend(ids.iter().copied());
            }
            if let Some(te) = &self.text_edit
                && te.page == i
                && let Some(id) = te.id
            {
                exclude.push(id);
            }
            let (hl, inkm) = self.cache.page(&page, &exclude);
            let (hl, inkm) = (place(hl, pr.min, zoom), place(inkm, pr.min, zoom));
            pp.add(hl);
            pp.add(inkm);
            for it in &page.items {
                if let Content::Text(t) = &it.content
                    && !exclude.contains(&it.id)
                {
                    draw_text(&pp, pr.min, zoom, t);
                }
            }

            // Selection being moved or scaled: draw it transformed.
            if let Some((mp, ids)) = &moving
                && *mp == i
            {
                let f: Box<dyn Fn(Pos2) -> Pos2> = match self.action {
                    Action::Move { delta, .. } => {
                        Box::new(move |p: Pos2| p + vec2(delta[0], delta[1]))
                    }
                    Action::Scale { anchor, factor, .. } => {
                        let a = pos2(anchor[0], anchor[1]);
                        Box::new(move |p: Pos2| a + (p - a) * factor)
                    }
                    _ => Box::new(|p| p),
                };
                for it in page.items.iter().filter(|it| ids.contains(&it.id)) {
                    match &it.content {
                        Content::Text(_) => {
                            let moved = match self.action {
                                Action::Move { delta, .. } => {
                                    geom::translate_item(it, delta[0], delta[1])
                                }
                                Action::Scale { anchor, factor, .. } => {
                                    geom::scale_item(it, anchor[0], anchor[1], factor)
                                }
                                _ => (**it).clone(),
                            };
                            if let Content::Text(t) = &moved.content {
                                draw_text(&pp, pr.min, zoom, t);
                            }
                        }
                        _ => {
                            let m = self.cache.item(it, page.paper).clone();
                            pp.add(place_transformed(&m, pr.min, zoom, &f));
                        }
                    }
                }
            }

            // Live previews.
            match &self.action {
                Action::Ink {
                    page: ap,
                    pts,
                    snapped,
                    ruler_edge,
                    ..
                } if *ap == i => {
                    let hl = self.tool == Tool::Highlighter;
                    if let Some(sh) = snapped {
                        let mut m = ink::Mesh::default();
                        if hl {
                            let w = mm(prefs.hl_mm);
                            ink::ink_mesh(
                                &ink::polyline_samples(&sh.points[..2.min(sh.points.len())], w),
                                &mut m,
                            );
                            let mut out = egui::Mesh::default();
                            super::mesh::append(
                                &m,
                                c32(ink::ink_color(
                                    &Stroke {
                                        pen: PenKind::Highlighter,
                                        color: prefs.hl_color,
                                        width: w,
                                        points: vec![],
                                    },
                                    page.paper,
                                )),
                                &mut out,
                            );
                            pp.add(place(&out, pr.min, zoom));
                        } else {
                            ink::shape_mesh(sh, &mut m);
                            let mut out = egui::Mesh::default();
                            super::mesh::append(&m, c32(sh.color), &mut out);
                            pp.add(place(&out, pr.min, zoom));
                        }
                    } else {
                        let (pen, color, width) = if hl {
                            (PenKind::Highlighter, prefs.hl_color, mm(prefs.hl_mm))
                        } else {
                            (prefs.pen_kind, prefs.pen_color, mm(prefs.pen_mm))
                        };
                        let mut st = Stroke {
                            pen,
                            color,
                            width,
                            points: pts.clone(),
                        };
                        if let (Some(edge), Some(r)) = (ruler_edge, self.ruler) {
                            st.points = self.project_preview(i, &st.points, r, *edge, width);
                        }
                        let m =
                            samples_mesh(&ink::samples(&st), c32(ink::ink_color(&st, page.paper)));
                        pp.add(place(&m, pr.min, zoom));
                    }
                }
                Action::ShapeDrag { page: ap, a, b } if *ap == i => {
                    let sh = Shape {
                        kind: prefs.shape_kind,
                        points: vec![*a, *b],
                        color: prefs.shape_color,
                        width: mm(prefs.shape_mm),
                        fill: prefs.shape_fill.then_some(prefs.shape_color),
                        dashed: prefs.shape_dashed,
                    };
                    let m = super::mesh::item_mesh(&Item::new(Content::Shape(sh)), page.paper);
                    pp.add(place(&m, pr.min, zoom));
                }
                Action::Lasso { page: ap, pts } if *ap == i && pts.len() > 1 => {
                    let screen: Vec<Pos2> = pts
                        .iter()
                        .map(|p| pr.min + vec2(p[0], p[1]) * zoom)
                        .collect();
                    let mut closed = screen.clone();
                    closed.push(screen[0]);
                    pp.extend(EShape::dashed_line(
                        &closed,
                        EStroke::new(1.6, pal.accent),
                        6.0,
                        4.0,
                    ));
                }
                _ => {}
            }
        }

        self.paint_selection(&painter, &ctx, prefs, clipboard, &pal);
        self.paint_ruler(&painter);
        self.paint_laser(&painter, prefs);
        if let Action::Erase { .. } = self.action
            && let Some(p) = ctx.input(|i| i.pointer.latest_pos())
        {
            painter.circle_stroke(p, prefs.eraser_size, EStroke::new(1.5, pal.muted));
        }
        self.text_editor_ui(&ctx, prefs, &pal);

        let busy = !matches!(self.action, Action::Idle)
            || !self.laser.is_empty()
            || !self.fingers.map.is_empty();
        if busy {
            ctx.request_repaint();
        }
    }

    fn project_preview(
        &self,
        page: usize,
        pts: &[InkPoint],
        r: Ruler,
        edge: usize,
        width: f32,
    ) -> Vec<InkPoint> {
        if pts.is_empty() {
            return Vec::new();
        }
        let o = self.page_origin_doc(page);
        let side = if edge == 1 { 1.0 } else { -1.0 };
        let (d, n) = (r.dir(), r.normal());
        let off = side * (RULER_HALF + width / 2.0 + 0.5);
        let proj = |p: &InkPoint| {
            let t = r.along([p.x + o[0], p.y + o[1]]);
            InkPoint {
                x: r.center[0] + d[0] * t + n[0] * off - o[0],
                y: r.center[1] + d[1] * t + n[1] * off - o[1],
                p: p.p,
            }
        };
        vec![proj(&pts[0]), proj(pts.last().expect("non-empty"))]
    }

    fn paint_selection(
        &mut self,
        painter: &egui::Painter,
        ctx: &egui::Context,
        prefs: &Prefs,
        clipboard: &mut Vec<Item>,
        pal: &theme::Palette,
    ) {
        let Some(sel) = self.selection.clone() else {
            return;
        };
        if sel.page >= self.doc.pages.len() {
            self.selection = None;
            return;
        }
        let Some(mut b) = self.selection_bbox() else {
            self.selection = None;
            return;
        };
        match self.action {
            Action::Move { delta, .. } => {
                b.x += delta[0];
                b.y += delta[1];
            }
            Action::Scale { factor, .. } => {
                b.w *= factor;
                b.h *= factor;
            }
            _ => {}
        }
        let r = Rect::from_min_max(
            self.page_to_screen(sel.page, [b.x, b.y]),
            self.page_to_screen(sel.page, [b.right(), b.bottom()]),
        )
        .expand(6.0);
        let corners = [
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
            r.left_top(),
        ];
        painter.extend(EShape::dashed_line(
            &corners,
            EStroke::new(1.5, pal.accent),
            7.0,
            5.0,
        ));
        painter.circle(
            r.right_bottom(),
            9.0,
            Color32::WHITE,
            EStroke::new(2.0, pal.accent),
        );
        if !matches!(self.action, Action::Idle) {
            return;
        }

        // Floating action bar above the selection.
        let pos = pos2(r.center().x, (r.top() - 12.0).max(self.canvas.top() + 60.0));
        egui::Area::new(EId::unique("selection-menu"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .pivot(Align2::CENTER_BOTTOM)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .corner_radius(14)
                    .inner_margin(6)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for c in prefs.swatches.iter().take(6) {
                                let (cr, resp) =
                                    ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                                ui.painter().circle_filled(cr.center(), 10.0, c32(*c));
                                ui.painter().circle_stroke(
                                    cr.center(),
                                    10.5,
                                    EStroke::new(1.0, pal.line),
                                );
                                if resp.clicked() {
                                    let c = *c;
                                    self.map_selection(|it| it.with_color(c));
                                }
                            }
                            ui.separator();
                            if icons::button(ui, Icon::Copy, false, "Copy").clicked() {
                                self.copy_selection(clipboard);
                                self.toast("Copied");
                            }
                            if icons::button(ui, Icon::Cut, false, "Cut").clicked() {
                                self.copy_selection(clipboard);
                                self.delete_selection();
                            }
                            if icons::button(ui, Icon::Duplicate, false, "Duplicate").clicked() {
                                self.duplicate_selection();
                            }
                            if ui.button("Front").on_hover_text("Bring to front").clicked() {
                                self.bring_to_front();
                            }
                            if icons::button(ui, Icon::Trash, false, "Delete").clicked() {
                                self.delete_selection();
                            }
                        });
                    });
            });
    }

    fn paint_ruler(&self, painter: &egui::Painter) {
        let Some(r) = self.ruler else { return };
        let (d, n) = (r.dir(), r.normal());
        let at = |t: f32, o: f32| {
            self.doc_to_screen([
                r.center[0] + d[0] * t + n[0] * o,
                r.center[1] + d[1] * t + n[1] * o,
            ])
        };
        let half = RULER_LEN / 2.0;
        let poly = vec![
            at(-half, -RULER_HALF),
            at(half, -RULER_HALF),
            at(half, RULER_HALF),
            at(-half, RULER_HALF),
        ];
        painter.add(EShape::convex_polygon(
            poly,
            Color32::from_rgba_unmultiplied(235, 235, 240, 175),
            EStroke::new(1.0, Color32::from_gray(120)),
        ));
        let mm5 = mm(5.0);
        let n_ticks = (RULER_LEN / mm5) as i32;
        for k in -n_ticks / 2..=n_ticks / 2 {
            let t = k as f32 * mm5;
            let len = if k % 2 == 0 { 14.0 } else { 8.0 };
            for side in [-1.0f32, 1.0] {
                painter.line_segment(
                    [at(t, side * RULER_HALF), at(t, side * (RULER_HALF - len))],
                    EStroke::new(1.0, Color32::from_gray(70)),
                );
            }
            if k % 2 == 0 && k != 0 && self.zoom > 0.6 {
                painter.text(
                    at(t, 0.0),
                    Align2::CENTER_CENTER,
                    format!("{}", k / 2),
                    FontId::proportional(10.0),
                    Color32::from_gray(60),
                );
            }
        }
        let deg = (r.angle.to_degrees().rem_euclid(360.0)).round() as i32 % 180;
        painter.text(
            at(0.0, 0.0),
            Align2::CENTER_CENTER,
            format!("{deg}°"),
            FontId::proportional(15.0),
            Color32::from_gray(30),
        );
    }

    fn paint_laser(&self, painter: &egui::Painter, prefs: &Prefs) {
        if self.laser.len() < 2 {
            return;
        }
        let c = c32(prefs.laser_color);
        for w in self.laser.windows(2) {
            let (a, b) = (w[0], w[1]);
            if geom::dist(a.0, b.0) > 200.0 {
                continue;
            }
            let fade = if b.1 < 0.0 {
                1.0
            } else {
                (1.0 - (self.now - b.1) as f32).clamp(0.0, 1.0)
            };
            let (pa, pb) = (self.doc_to_screen(a.0), self.doc_to_screen(b.0));
            painter.line_segment([pa, pb], EStroke::new(10.0, c.gamma_multiply(0.25 * fade)));
            painter.line_segment([pa, pb], EStroke::new(3.5, c.gamma_multiply(fade)));
        }
    }

    fn text_editor_ui(&mut self, ctx: &egui::Context, prefs: &mut Prefs, pal: &theme::Palette) {
        let Some(page) = self.text_edit.as_ref().map(|t| t.page) else {
            return;
        };
        if page >= self.doc.pages.len() {
            self.text_edit = None;
            return;
        }
        let o = self.page_origin_doc(page);
        let zoom = self.zoom;
        let base = self.canvas.min + self.offset;
        let Some(te) = &mut self.text_edit else {
            return;
        };
        let screen = base + vec2(o[0] + te.x, o[1] + te.y) * zoom;
        let mut done = false;
        egui::Area::new(EId::unique("text-editor"))
            .order(Order::Foreground)
            .fixed_pos(screen - vec2(4.0, 48.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::popup(ui.style())
                        .corner_radius(12)
                        .inner_margin(4)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.button("A−").clicked() {
                                    te.size = (te.size - 2.0).max(8.0);
                                    prefs.text_size = te.size;
                                }
                                ui.label(format!("{:.0}", te.size));
                                if ui.button("A+").clicked() {
                                    te.size = (te.size + 2.0).min(96.0);
                                    prefs.text_size = te.size;
                                }
                                for c in prefs.swatches.iter().take(5) {
                                    let (cr, resp) =
                                        ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                                    ui.painter().circle_filled(cr.center(), 9.0, c32(*c));
                                    if te.color == *c {
                                        ui.painter().circle_stroke(
                                            cr.center(),
                                            11.0,
                                            EStroke::new(2.0, pal.accent),
                                        );
                                    }
                                    if resp.clicked() {
                                        te.color = *c;
                                        prefs.text_color = *c;
                                    }
                                }
                                if ui.button("Done").clicked() {
                                    done = true;
                                }
                            });
                        });
                });
                ui.add_space(6.0);
                let fs = te.size * zoom;
                let frame = egui::Frame::new()
                    .stroke(EStroke::new(1.0, pal.accent))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::same(3));
                frame.show(ui, |ui| {
                    let edit = egui::TextEdit::multiline(&mut te.text)
                        .font(FontId::proportional(fs))
                        .text_color(c32(te.color))
                        .desired_width(te.w * zoom)
                        .desired_rows(1)
                        .frame(egui::Frame::NONE)
                        .background_color(Color32::TRANSPARENT)
                        .hint_text("Type here");
                    let resp = ui.add(edit);
                    if !te.focused_once {
                        resp.request_focus();
                        te.focused_once = true;
                    }
                    if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        done = true;
                    }
                });
            });
        let _ = StrokeKind::Middle;
        if done {
            self.finish_text();
        }
    }
}
