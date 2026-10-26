//! Draws the pages and everything on them, and hosts the text editor overlay.

use super::mesh::{place, place_transformed};
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

fn draw_text(p: &egui::Painter, origin: Pos2, zoom: f32, t: &TextBox, paper: Paper) {
    let lh = t.size * text::LINE_HEIGHT;
    let fs = quantize(t.size * zoom);
    let color = c32(super::mesh::display_color(t.color, paper));
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
            self.fit_canvas_w = rect.width();
        } else if (rect.width() - self.fit_canvas_w).abs() > 1.0 && rect.width() > 50.0 {
            // The window or a side panel changed size: keep fitting the width if the
            // page was fitted before.
            let old_fit = ((self.fit_canvas_w - 32.0) / self.max_w).clamp(MIN_ZOOM, 2.2);
            if (self.zoom - old_fit).abs() < 0.01 {
                let page = self.current;
                self.fit_width();
                self.scroll_to_page(page);
            }
            self.fit_canvas_w = rect.width();
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

        let night = prefs.night_mode;
        self.assets.begin_frame();
        for i in 0..self.doc.pages.len() {
            let pr = self.page_rect(i);
            if !pr.intersects(rect) {
                continue;
            }
            let page = self.doc.pages[i].clone();
            let paper = if night && !page.paper.is_dark() {
                Paper::Night
            } else {
                page.paper
            };
            painter.rect_filled(
                pr.translate(vec2(0.0, 3.0)).expand(1.5),
                3.0,
                Color32::from_black_alpha(if prefs.dark_ui { 120 } else { 40 }),
            );
            let clip = pr.intersect(rect);
            let pp = painter.with_clip_rect(clip);
            pp.rect_filled(pr, 0.0, c32(paper.background()));
            pp.add(place(self.cache.paper(&page, paper), pr.min, zoom));
            if let Some(bg) = page.background {
                match self.assets.texture(&ctx, bg) {
                    Some(t) => {
                        let tint = if night {
                            Color32::from_gray(200)
                        } else {
                            Color32::WHITE
                        };
                        pp.image(t, pr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
                    }
                    None => {
                        pp.text(
                            pr.center(),
                            Align2::CENTER_CENTER,
                            "Loading page…",
                            FontId::proportional(14.0),
                            Color32::GRAY,
                        );
                    }
                }
            }

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
            for layer in page.layers.iter().filter(|l| l.visible) {
                for it in page.items_in_layer(layer.id) {
                    if let Content::Image(im) = &it.content
                        && !exclude.contains(&it.id)
                    {
                        let r = Rect::from_min_size(
                            pr.min + vec2(im.x, im.y) * zoom,
                            vec2(im.w, im.h) * zoom,
                        );
                        self.draw_picture(&pp, &ctx, im.asset, r);
                    }
                }
                let (hl, inkm, tapes) = self.cache.layer(&page, layer.id, paper, &exclude);
                let (hl, inkm, tapes) = (
                    place(hl, pr.min, zoom),
                    place(inkm, pr.min, zoom),
                    place(tapes, pr.min, zoom),
                );
                pp.add(hl);
                pp.add(inkm);
                for it in page.items_in_layer(layer.id) {
                    if let Content::Text(t) = &it.content
                        && !exclude.contains(&it.id)
                    {
                        draw_text(&pp, pr.min, zoom, t, paper);
                    }
                }
                pp.add(tapes);
            }
            // Small badge on linked items.
            for it in page.items.iter().filter(|it| it.link.is_some() && page.visible(it)) {
                let b = geom::item_bbox(it);
                let c = pr.min + vec2(b.right(), b.y) * zoom;
                pp.circle_filled(c, 9.0, pal.accent);
                pp.text(
                    c,
                    Align2::CENTER_CENTER,
                    "🔗",
                    FontId::proportional(10.0),
                    Color32::WHITE,
                );
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
                    let moved = || match self.action {
                        Action::Move { delta, .. } => geom::translate_item(it, delta[0], delta[1]),
                        Action::Scale { anchor, factor, .. } => {
                            geom::scale_item(it, anchor[0], anchor[1], factor)
                        }
                        _ => (**it).clone(),
                    };
                    match &it.content {
                        Content::Text(_) => {
                            if let Content::Text(t) = &moved().content {
                                draw_text(&pp, pr.min, zoom, t, paper);
                            }
                        }
                        Content::Image(_) => {
                            if let Content::Image(im) = &moved().content {
                                let r = Rect::from_min_size(
                                    pr.min + vec2(im.x, im.y) * zoom,
                                    vec2(im.w, im.h) * zoom,
                                );
                                self.draw_picture(&pp, &ctx, im.asset, r);
                            }
                        }
                        _ => {
                            let m = self.cache.item(it, paper).clone();
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
                                    paper,
                                )),
                                &mut out,
                            );
                            pp.add(place(&out, pr.min, zoom));
                        } else {
                            ink::shape_mesh(sh, &mut m);
                            let mut out = egui::Mesh::default();
                            super::mesh::append(
                                &m,
                                c32(super::mesh::display_color(sh.color, paper)),
                                &mut out,
                            );
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
                        let m = super::mesh::item_mesh_with(
                            &Item::new(Content::Stroke(st)),
                            paper,
                            page.has_pictures(),
                        );
                        pp.add(place(&m, pr.min, zoom));
                    }
                }
                Action::ShapeDrag { page: ap, a, b } if *ap == i => {
                    let mut sh = Shape::new(
                        prefs.shape_kind,
                        vec![*a, *b],
                        prefs.shape_color,
                        mm(prefs.shape_mm),
                    );
                    sh.fill = prefs.shape_fill.then_some(prefs.shape_color);
                    sh.dashed = prefs.shape_dashed;
                    let m = super::mesh::item_mesh(&Item::new(Content::Shape(sh)), paper);
                    pp.add(place(&m, pr.min, zoom));
                }
                Action::TapeDraw { page: ap, pts } if *ap == i && pts.len() > 1 => {
                    let t = Tape {
                        points: match prefs.tape_kind {
                            crate::prefs::TapeKind::Free => pts.clone(),
                            _ => vec![pts[0], *pts.last().expect("len > 1")],
                        },
                        width: mm(prefs.tape_mm),
                        color: prefs.tape_color,
                        rect: prefs.tape_kind == crate::prefs::TapeKind::Rect,
                        revealed: false,
                    };
                    let m = super::mesh::item_mesh(&Item::new(Content::Tape(t)), paper);
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

    fn draw_picture(&mut self, p: &egui::Painter, ctx: &egui::Context, asset: Id, r: Rect) {
        match self.assets.texture(ctx, asset) {
            Some(t) => {
                p.image(
                    t,
                    r,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                p.rect_filled(r, 2.0, Color32::from_gray(200));
            }
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
        prefs: &mut Prefs,
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
        if !matches!(self.action, Action::Idle) || self.edit.is_some() {
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
                            if icons::button(ui, Icon::Trash, false, "Delete").clicked() {
                                self.delete_selection();
                            }
                            ui.separator();
                            let more = ui.button("More ▾");
                            egui::Popup::menu(&more).show(|ui| {
                                ui.set_min_width(190.0);
                                let has_shape = self
                                    .selected_items()
                                    .iter()
                                    .any(|i| matches!(i.content, Content::Shape(_)));
                                if ui
                                    .add_enabled(has_shape, egui::Button::new("Shape style…"))
                                    .clicked()
                                {
                                    self.popover = Popover::ShapeStyle;
                                    self.popover_anchor = more.rect;
                                }
                                let link = self
                                    .selected_items()
                                    .first()
                                    .and_then(|i| i.link.clone());
                                if ui.button("🔗  Link…").clicked() {
                                    let (url, page, web) = match &link {
                                        Some(Link::Url(u)) => (u.clone(), String::new(), true),
                                        Some(Link::Page(id)) => (
                                            String::new(),
                                            self.doc
                                                .page_index(*id)
                                                .map(|i| (i + 1).to_string())
                                                .unwrap_or_default(),
                                            false,
                                        ),
                                        None => (String::new(), String::new(), true),
                                    };
                                    self.edit = Some(EditDialog::Link { url, page, web });
                                }
                                if link.is_some() && ui.button("Follow link").clicked() {
                                    if let Some(l) = link.clone() {
                                        self.follow_link(&l);
                                    }
                                }
                                if ui.button("★  Save as sticker").clicked() {
                                    if let Some(st) = self.selection_as_sticker() {
                                        prefs.stickers.insert(0, st);
                                        prefs.stickers.truncate(40);
                                        self.toast("Saved to Stickers");
                                    }
                                }
                                ui.separator();
                                if ui.button("Bring to front").clicked() {
                                    self.bring_to_front();
                                }
                                if ui.button("Send to back").clicked() {
                                    self.send_to_back();
                                }
                                if self.doc.pages[sel.page].layers.len() > 1
                                    && ui.button("Move to layer…").clicked()
                                {
                                    self.popover = Popover::MoveToLayer;
                                    self.popover_anchor = more.rect;
                                }
                            });
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
