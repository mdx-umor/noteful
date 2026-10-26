//! Pointer handling. Apple Pencil touches carry a force value, finger touches do
//! not, which is how the Pencil writes while fingers scroll, zoom and undo.

use super::*;
use egui::{Event, PointerButton, TouchPhase};
use notebook_core::recognize::recognize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Device {
    Pencil,
    Finger,
    Mouse,
}

/// Seconds the pen must rest before a stroke snaps into a shape.
const HOLD_SECS: f64 = 0.5;

impl Editor {
    /// Processes this frame's raw input. `blocked` says whether a screen point is
    /// covered by UI (toolbars, popovers) and must not start drawing.
    pub fn handle_input(
        &mut self,
        ctx: &egui::Context,
        prefs: &mut Prefs,
        clipboard: &mut Vec<Item>,
        hovered: bool,
    ) {
        let events = ctx.input(|i| i.events.clone());
        if events.iter().any(|e| matches!(e, Event::Touch { .. })) {
            self.touch_mode = true;
        }
        for e in events {
            match e {
                Event::Touch {
                    id,
                    phase,
                    pos,
                    force,
                    ..
                } => self.on_touch(ctx, id.0, phase, pos, force, prefs, clipboard),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    ..
                } if !self.touch_mode => {
                    if pressed {
                        if self.accepts(ctx, pos) {
                            self.mouse_down = true;
                            self.down(pos, 0.5, Device::Mouse, prefs);
                        }
                    } else if self.mouse_down {
                        self.mouse_down = false;
                        self.up(pos, prefs, clipboard);
                    }
                }
                Event::PointerMoved(pos) if !self.touch_mode && self.mouse_down => {
                    self.drag(pos, 0.5, prefs)
                }
                _ => {}
            }
        }
        if !self.touch_mode && hovered {
            let (scroll, zoom, ptr) =
                ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
            if (zoom - 1.0).abs() > 1e-4 {
                if let Some(p) = ptr {
                    self.zoom_at(p, self.zoom * zoom);
                }
            } else if scroll != Vec2::ZERO {
                self.offset += scroll;
                self.clamp_offset();
            }
        }
        self.tick(prefs);
    }

    /// Can a stroke start here? Only on the canvas and not under any floating UI.
    fn accepts(&self, ctx: &egui::Context, pos: Pos2) -> bool {
        self.canvas.contains(pos) && ctx.layer_id_at(pos).is_none_or(|l| l == self.layer)
    }

    #[allow(clippy::too_many_arguments)]
    fn on_touch(
        &mut self,
        ctx: &egui::Context,
        id: u64,
        phase: TouchPhase,
        pos: Pos2,
        force: Option<f32>,
        prefs: &mut Prefs,
        clipboard: &mut Vec<Item>,
    ) {
        // On iPad only the Pencil reports force. In Safari every touch carries a force
        // value, but fingers report 0. Once a Pencil stroke has started, every event of
        // that touch belongs to the Pencil: Safari reports force 0 as the Pencil lifts,
        // which must not be mistaken for a finger (the stroke would never be saved).
        let has_force = if cfg!(target_arch = "wasm32") {
            force.is_some_and(|f| f > 0.0)
        } else {
            force.is_some()
        };
        let own_pencil_touch = self.pen_touch == Some(id) && !self.fingers.map.contains_key(&id);
        let is_pencil = own_pencil_touch || (has_force && phase == TouchPhase::Start);
        if is_pencil {
            if !prefs.pencil_seen {
                prefs.pencil_seen = true;
                prefs.pencil_only = true;
                self.toast(
                    "Apple Pencil connected: write with the Pencil, scroll and zoom with fingers",
                );
            }
            // A zero reading (the moment of lifting) keeps the last real pressure.
            let p = if force.is_some_and(|f| f > 0.0) {
                pressure(force, prefs)
            } else {
                self.last_pressure()
            };
            match phase {
                TouchPhase::Start => {
                    if self.pen_touch.is_none() && self.accepts(ctx, pos) {
                        self.pen_touch = Some(id);
                        self.down(pos, p, Device::Pencil, prefs);
                    }
                }
                TouchPhase::Move => {
                    if self.pen_touch == Some(id) {
                        self.drag(pos, p, prefs);
                    }
                }
                TouchPhase::End => {
                    if self.pen_touch == Some(id) {
                        self.pen_touch = None;
                        self.up(pos, prefs, clipboard);
                    }
                }
                TouchPhase::Cancel => {
                    if self.pen_touch == Some(id) {
                        self.pen_touch = None;
                        self.up(pos, prefs, clipboard);
                    }
                }
            }
            return;
        }

        // ---------- fingers ----------
        // While the Pencil is down, other touches are the writing hand resting on the
        // screen: ignore them so they cannot scroll or cancel the stroke.
        let pencil_down = self
            .pen_touch
            .is_some_and(|p| !self.fingers.map.contains_key(&p));
        if pencil_down && !self.fingers.map.contains_key(&id) {
            return;
        }
        match phase {
            TouchPhase::Start => {
                if !self.accepts(ctx, pos) && self.fingers.map.is_empty() {
                    return;
                }
                if self.fingers.map.is_empty() {
                    self.fingers.started = self.now;
                    self.fingers.max_count = 0;
                    self.fingers.moved = 0.0;
                }
                self.fingers.map.insert(id, (pos, pos));
                self.fingers.max_count = self.fingers.max_count.max(self.fingers.map.len());
                let n = self.fingers.map.len();
                if n == 1 {
                    let finger_draws = !prefs.pencil_only && self.pen_touch.is_none();
                    let doc = self.screen_to_doc(pos);
                    let on_selection = self.selection_hit(pos).is_some();
                    let on_ruler = self.ruler.is_some_and(|r| r.contains(doc));
                    if on_selection {
                        self.pen_touch = Some(id);
                        self.down(pos, 0.5, Device::Finger, prefs);
                    } else if on_ruler {
                        self.action = Action::RulerMove { last: pos };
                    } else if finger_draws {
                        self.pen_touch = Some(id);
                        self.down(pos, 0.5, Device::Finger, prefs);
                    } else {
                        self.action = Action::Pan { last: pos };
                    }
                } else if n == 2 {
                    // A second finger turns a fresh finger stroke into a pinch.
                    if let Some(pid) = self.pen_touch
                        && self.fingers.map.contains_key(&pid)
                    {
                        self.pen_touch = None;
                        self.cancel_action();
                    }
                    let (a, b) = self.two_fingers();
                    let center = pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                    if self
                        .ruler
                        .is_some_and(|r| r.contains(self.screen_to_doc(center)))
                    {
                        let ang = (b.y - a.y).atan2(b.x - a.x);
                        self.fingers.ruler_angle = Some((ang, self.ruler.map_or(0.0, |r| r.angle)));
                        self.action = Action::RulerMove { last: center };
                    } else {
                        self.fingers.pinch = Some((a.distance(b).max(1.0), self.zoom, center));
                        self.action = Action::Idle;
                    }
                }
            }
            TouchPhase::Move => {
                let Some(entry) = self.fingers.map.get_mut(&id) else {
                    return;
                };
                let prev = entry.1;
                entry.1 = pos;
                self.fingers.moved += prev.distance(pos);
                if self.pen_touch == Some(id) {
                    self.drag(pos, 0.5, prefs);
                    return;
                }
                if self.fingers.map.len() >= 2 {
                    let (a, b) = self.two_fingers();
                    let center = pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                    if let Some((a0, r0)) = self.fingers.ruler_angle {
                        if let (Some(r), Action::RulerMove { last }) =
                            (&mut self.ruler, &mut self.action)
                        {
                            let ang = (b.y - a.y).atan2(b.x - a.x);
                            r.angle = r0 + (ang - a0);
                            let d = (center - *last) / self.zoom;
                            r.center[0] += d.x;
                            r.center[1] += d.y;
                            *last = center;
                        }
                    } else if let Some((d0, z0, last)) = self.fingers.pinch {
                        let d = a.distance(b).max(1.0);
                        self.offset += center - last;
                        self.zoom_at(center, z0 * d / d0);
                        self.fingers.pinch = Some((d0, z0, center));
                    }
                } else {
                    match &mut self.action {
                        Action::Pan { last } => {
                            let d = pos - *last;
                            *last = pos;
                            self.offset += d;
                            self.clamp_offset();
                        }
                        Action::RulerMove { last } => {
                            let d = (pos - *last) / self.zoom;
                            *last = pos;
                            if let Some(r) = &mut self.ruler {
                                r.center[0] += d.x;
                                r.center[1] += d.y;
                            }
                        }
                        _ => {}
                    }
                }
            }
            TouchPhase::End | TouchPhase::Cancel => {
                let Some((start, _)) = self.fingers.map.remove(&id) else {
                    return;
                };
                if self.pen_touch == Some(id) {
                    self.pen_touch = None;
                    self.up(pos, prefs, clipboard);
                }
                if self.fingers.map.len() < 2 {
                    self.fingers.pinch = None;
                    self.fingers.ruler_angle = None;
                    if let Some((_, (_, cur))) = self.fingers.map.iter().next() {
                        self.action = Action::Pan { last: *cur };
                    }
                }
                if self.fingers.map.is_empty() {
                    if matches!(self.action, Action::Pan { .. } | Action::RulerMove { .. }) {
                        self.action = Action::Idle;
                    }
                    let quick = self.now - self.fingers.started < 0.3 && self.fingers.moved < 24.0;
                    if quick && phase == TouchPhase::End && prefs.two_finger_undo {
                        match self.fingers.max_count {
                            2 => {
                                self.undo();
                                self.toast("Undo");
                            }
                            3 => {
                                self.redo();
                                self.toast("Redo");
                            }
                            _ => {}
                        }
                    }
                    if quick && self.fingers.max_count == 1 && start.distance(pos) < 10.0 {
                        self.finger_tap(pos, prefs);
                    }
                }
            }
        }
    }

    /// Pressure of the last point of the stroke being drawn.
    fn last_pressure(&self) -> f32 {
        match &self.action {
            Action::Ink { pts, .. } => pts.last().map_or(0.5, |p| p.p),
            _ => 0.5,
        }
    }

    fn two_fingers(&self) -> (Pos2, Pos2) {
        let mut it = self.fingers.map.values().map(|v| v.1);
        let a = it.next().unwrap_or_default();
        (a, it.next().unwrap_or(a))
    }

    /// A finger tap edits text under it, otherwise clears the selection.
    fn finger_tap(&mut self, pos: Pos2, prefs: &Prefs) {
        let Some(page) = self.page_at(pos) else {
            return;
        };
        let p = self.screen_to_page(page, pos);
        let on_text = self.doc.pages[page]
            .items
            .iter()
            .any(|i| matches!(i.content, Content::Text(_)) && geom::hit_item(i, p[0], p[1], 4.0));
        if self.popover != Popover::None {
            self.popover = Popover::None;
            return;
        }
        if self.tool != Tool::Text && self.tap_item(pos) {
            return;
        }
        if on_text || self.tool == Tool::Text {
            self.start_text(page, p, prefs);
        } else if self.selection.is_some() {
            self.selection = None;
        }
    }

    /// Is the point on the selection? Returns true for the resize handle.
    pub fn selection_hit(&self, pos: Pos2) -> Option<bool> {
        let sel = self.selection.as_ref()?;
        let b = self.selection_bbox()?;
        let corner = self.page_to_screen(sel.page, [b.right(), b.bottom()]);
        if corner.distance(pos) < 26.0 {
            return Some(true);
        }
        let r = Rect::from_min_max(self.page_to_screen(sel.page, [b.x, b.y]), corner).expand(10.0);
        r.contains(pos).then_some(false)
    }

    pub fn cancel_action(&mut self) {
        if let Action::Erase {
            before: Some(b), ..
        } = std::mem::replace(&mut self.action, Action::Idle)
        {
            // Keep what was erased, but make it undoable.
            self.commit(b);
        }
        self.action = Action::Idle;
    }

    // ---------- tool actions ----------

    pub fn down(&mut self, pos: Pos2, p: f32, device: Device, prefs: &Prefs) {
        self.tap_start = Some((pos, self.now));
        // The first tap outside an open panel only closes it.
        if self.popover != Popover::None {
            self.popover = Popover::None;
            return;
        }
        if self.text_edit.is_some() {
            self.finish_text();
            if self.tool == Tool::Text {
                return;
            }
        }
        // Dragging an existing selection works with any tool.
        if let Some(handle) = self.selection_hit(pos) {
            let sel = self.selection.as_ref().expect("hit implies selection");
            let page = sel.page;
            let pt = self.screen_to_page(page, pos);
            if handle {
                let b = self.selection_bbox().expect("selection");
                let anchor = [b.x, b.y];
                let start = geom::dist(anchor, pt).max(1.0);
                self.action = Action::Scale {
                    page,
                    anchor,
                    start,
                    factor: 1.0,
                };
            } else {
                self.action = Action::Move {
                    page,
                    start: pt,
                    delta: [0.0, 0.0],
                };
            }
            return;
        }
        if self.tool == Tool::Hand || (device == Device::Mouse && self.tool == Tool::Hand) {
            self.action = Action::Pan { last: pos };
            return;
        }
        if self.ruler.is_some_and(|r| {
            r.contains(self.screen_to_doc(pos))
                && r.offset(self.screen_to_doc(pos)).abs() < RULER_HALF - 10.0 / self.zoom
        }) && self.tool != Tool::Pen
            && self.tool != Tool::Highlighter
        {
            self.action = Action::RulerMove { last: pos };
            return;
        }
        let Some(page) = self.page_at(pos) else {
            self.selection = None;
            self.action = if device == Device::Mouse {
                Action::Pan { last: pos }
            } else {
                Action::Idle
            };
            return;
        };
        let pt = self.screen_to_page(page, pos);
        let writes = matches!(
            self.tool,
            Tool::Pen | Tool::Highlighter | Tool::Tape | Tool::Shape | Tool::Text
        );
        if writes && !self.can_write(page) {
            // Taps still reveal tape and follow links on locked layers.
            if !self.tap_item(pos) {
                self.toast("The active layer is locked or hidden. Pick another in Layers.");
            }
            self.action = Action::Idle;
            return;
        }
        match self.tool {
            Tool::Tape => {
                self.selection = None;
                self.action = Action::TapeDraw {
                    page,
                    pts: vec![pt],
                };
            }
            Tool::Pen | Tool::Highlighter => {
                self.selection = None;
                let doc = self.screen_to_doc(pos);
                let ruler_edge = self.ruler.and_then(|r| {
                    let off = r.offset(doc);
                    let near = 28.0 / self.zoom;
                    if r.along(doc).abs() > RULER_LEN / 2.0 {
                        None
                    } else if (off - RULER_HALF).abs() < near {
                        Some(1)
                    } else if (off + RULER_HALF).abs() < near {
                        Some(0)
                    } else {
                        None
                    }
                });
                self.action = Action::Ink {
                    page,
                    pts: vec![InkPoint {
                        x: pt[0],
                        y: pt[1],
                        p,
                    }],
                    smooth: pt,
                    last_screen: pos,
                    still_since: self.now,
                    snapped: None,
                    ruler_edge,
                };
            }
            Tool::Eraser => {
                self.selection = None;
                self.action = Action::Erase {
                    page,
                    before: None,
                    last: pt,
                };
                self.erase_at(page, pt, prefs);
            }
            Tool::Lasso => {
                self.selection = None;
                self.action = Action::Lasso {
                    page,
                    pts: vec![pt],
                };
            }
            Tool::Shape => {
                self.selection = None;
                self.action = Action::ShapeDrag { page, a: pt, b: pt };
            }
            Tool::Text => self.start_text(page, pt, prefs),
            Tool::Laser => {
                self.laser.retain(|(_, t)| *t >= 0.0);
                self.laser.push((self.screen_to_doc(pos), -1.0));
                self.action = Action::Laser;
            }
            Tool::Hand => {}
        }
    }

    pub fn drag(&mut self, pos: Pos2, p: f32, prefs: &Prefs) {
        let zoom = self.zoom;
        match &mut self.action {
            Action::Ink {
                page,
                pts,
                smooth,
                last_screen,
                still_since,
                snapped,
                ruler_edge,
            } => {
                let page = *page;
                let raw = {
                    let d = (pos - self.canvas.min - self.offset) / zoom;
                    let o = [(self.max_w - self.doc.pages[page].w) / 2.0, self.tops[page]];
                    [d.x - o[0], d.y - o[1]]
                };
                if last_screen.distance(pos) > 2.5 {
                    *still_since = self.now;
                    *last_screen = pos;
                }
                if let Some(sh) = snapped {
                    if sh.kind == ShapeKind::Line && sh.points.len() == 2 {
                        sh.points[1] = raw;
                    }
                    return;
                }
                // Adaptive smoothing: strong when slow (steady lines), light when fast (no lag).
                let stab = prefs.stabilization.clamp(0.0, 1.0);
                let a = 1.0 - stab * 0.75;
                let speed = (raw[0] - smooth[0]).hypot(raw[1] - smooth[1]) * zoom;
                let a = a + (1.0 - a) * (speed / (speed + 14.0));
                let s = [
                    smooth[0] + (raw[0] - smooth[0]) * a,
                    smooth[1] + (raw[1] - smooth[1]) * a,
                ];
                *smooth = s;
                let last = pts.last().copied().expect("stroke has a point");
                if (s[0] - last.x).hypot(s[1] - last.y) * zoom > 0.6 {
                    pts.push(InkPoint {
                        x: s[0],
                        y: s[1],
                        p,
                    });
                } else if let Some(l) = pts.last_mut() {
                    l.p = l.p.max(p);
                }
                let _ = ruler_edge;
            }
            Action::Erase { page, last, .. } => {
                let (page, from) = (*page, *last);
                let to = self.screen_to_page(page, pos);
                let r = prefs.eraser_size / zoom;
                let steps = ((geom::dist(from, to) / (r * 0.5)).ceil() as usize).clamp(1, 64);
                for k in 1..=steps {
                    let t = k as f32 / steps as f32;
                    self.erase_at(
                        page,
                        [
                            from[0] + (to[0] - from[0]) * t,
                            from[1] + (to[1] - from[1]) * t,
                        ],
                        prefs,
                    );
                }
                if let Action::Erase { last, .. } = &mut self.action {
                    *last = to;
                }
            }
            Action::Lasso { page, pts } => {
                let page = *page;
                let pt = {
                    let d = (pos - self.canvas.min - self.offset) / zoom;
                    let o = [(self.max_w - self.doc.pages[page].w) / 2.0, self.tops[page]];
                    [d.x - o[0], d.y - o[1]]
                };
                if pts.last().is_none_or(|l| geom::dist(*l, pt) * zoom > 2.0) {
                    pts.push(pt);
                }
            }
            Action::TapeDraw { page, pts } => {
                let page = *page;
                let pt = {
                    let d = (pos - self.canvas.min - self.offset) / zoom;
                    let o = [(self.max_w - self.doc.pages[page].w) / 2.0, self.tops[page]];
                    [d.x - o[0], d.y - o[1]]
                };
                match prefs.tape_kind {
                    crate::prefs::TapeKind::Free => {
                        if pts.last().is_none_or(|l| geom::dist(*l, pt) * zoom > 2.0) {
                            pts.push(pt);
                        }
                    }
                    _ => {
                        pts.truncate(1);
                        pts.push(pt);
                    }
                }
            }
            Action::Move { page, start, .. } => {
                let (page, s) = (*page, *start);
                let pt = self.screen_to_page(page, pos);
                if let Action::Move { delta, .. } = &mut self.action {
                    *delta = [pt[0] - s[0], pt[1] - s[1]];
                }
            }
            Action::Scale {
                page,
                anchor,
                start,
                ..
            } => {
                let (page, anchor, start) = (*page, *anchor, *start);
                let pt = self.screen_to_page(page, pos);
                let f = (geom::dist(anchor, pt) / start).clamp(0.1, 20.0);
                if let Action::Scale { factor, .. } = &mut self.action {
                    *factor = f;
                }
            }
            Action::ShapeDrag { page, a, .. } => {
                let (page, a) = (*page, *a);
                let mut pt = self.screen_to_page(page, pos);
                if prefs.shape_kind == ShapeKind::Line || prefs.shape_kind == ShapeKind::Arrow {
                    // Snap near-horizontal/vertical/diagonal lines.
                    let ang = (pt[1] - a[1]).atan2(pt[0] - a[0]);
                    let step = std::f32::consts::FRAC_PI_4;
                    let snapped = (ang / step).round() * step;
                    if (ang - snapped).abs() < 0.06 {
                        let l = geom::dist(a, pt);
                        pt = [a[0] + l * snapped.cos(), a[1] + l * snapped.sin()];
                    }
                }
                if let Action::ShapeDrag { b, .. } = &mut self.action {
                    *b = pt;
                }
            }
            Action::Pan { last } => {
                let d = pos - *last;
                *last = pos;
                self.offset += d;
                self.clamp_offset();
            }
            Action::Laser => {
                let d = self.screen_to_doc(pos);
                self.laser.push((d, -1.0));
            }
            Action::RulerMove { last } => {
                let d = (pos - *last) / zoom;
                *last = pos;
                if let Some(r) = &mut self.ruler {
                    r.center[0] += d.x;
                    r.center[1] += d.y;
                }
            }
            Action::Idle => {}
        }
    }

    pub fn up(&mut self, pos: Pos2, prefs: &Prefs, _clipboard: &mut Vec<Item>) {
        let action = std::mem::replace(&mut self.action, Action::Idle);
        let tap = self
            .tap_start
            .take()
            .is_some_and(|(p0, t0)| p0.distance(pos) < 8.0 && self.now - t0 < 0.4);
        match action {
            Action::Ink { .. } if tap && self.tap_item_with(pos, false) => {}
            Action::Ink {
                page,
                mut pts,
                snapped,
                ruler_edge,
                ..
            } => {
                let end = self.screen_to_page(page, pos);
                if pts
                    .last()
                    .is_some_and(|l| (l.x - end[0]).hypot(l.y - end[1]) * self.zoom > 0.6)
                    && snapped.is_none()
                {
                    let p = pts.last().map_or(0.5, |l| l.p);
                    pts.push(InkPoint {
                        x: end[0],
                        y: end[1],
                        p,
                    });
                }
                let hl = self.tool == Tool::Highlighter;
                if let Some(sh) = snapped {
                    if hl {
                        let pp = sh.points.clone();
                        let pts = vec![
                            InkPoint {
                                x: pp[0][0],
                                y: pp[0][1],
                                p: 0.5,
                            },
                            InkPoint {
                                x: pp[1][0],
                                y: pp[1][1],
                                p: 0.5,
                            },
                        ];
                        self.add_item(
                            page,
                            Item::new(Content::Stroke(Stroke {
                                pen: PenKind::Highlighter,
                                color: prefs.hl_color,
                                width: mm(prefs.hl_mm),
                                points: pts,
                            })),
                        );
                    } else {
                        self.add_item(page, Item::new(Content::Shape(sh)));
                    }
                    return;
                }
                if let (Some(edge), Some(r)) = (ruler_edge, self.ruler) {
                    pts = self.project_on_ruler(
                        page,
                        &pts,
                        r,
                        edge,
                        if hl {
                            mm(prefs.hl_mm)
                        } else {
                            mm(prefs.pen_mm)
                        },
                    );
                } else if hl && prefs.hl_straight && pts.len() > 1 {
                    let (a, b) = (pts[0], *pts.last().expect("len > 1"));
                    pts = vec![
                        a,
                        InkPoint {
                            y: a.y + (b.y - a.y),
                            ..b
                        },
                    ];
                }
                let (pen, color, width) = if hl {
                    (PenKind::Highlighter, prefs.hl_color, mm(prefs.hl_mm))
                } else {
                    (prefs.pen_kind, prefs.pen_color, mm(prefs.pen_mm))
                };
                self.add_item(
                    page,
                    Item::new(Content::Stroke(Stroke {
                        pen,
                        color,
                        width,
                        points: pts,
                    })),
                );
            }
            Action::Erase { before, .. } => {
                if let Some(b) = before {
                    self.commit(b);
                }
                if prefs.eraser_return && self.prev_tool != Tool::Eraser {
                    self.set_tool(self.prev_tool);
                }
            }
            Action::TapeDraw { page, pts } => {
                if tap || pts.len() < 2 {
                    self.tap_item(pos);
                    return;
                }
                let rect = prefs.tape_kind == crate::prefs::TapeKind::Rect;
                let pts = if prefs.tape_kind == crate::prefs::TapeKind::Free {
                    notebook_core::recognize::rdp(&pts, 0.8)
                } else {
                    vec![pts[0], *pts.last().expect("len >= 2")]
                };
                if geom::dist(pts[0], *pts.last().expect("non-empty")) * self.zoom < 6.0
                    && pts.len() < 3
                {
                    return;
                }
                self.add_item(
                    page,
                    Item::new(Content::Tape(Tape {
                        points: pts,
                        width: mm(prefs.tape_mm),
                        color: prefs.tape_color,
                        rect,
                        revealed: false,
                    })),
                );
            }
            Action::Lasso { page, mut pts } => {
                let len: f32 =
                    pts.windows(2).map(|w| geom::dist(w[0], w[1])).sum::<f32>() * self.zoom;
                if prefs.lasso_rect && pts.len() > 1 {
                    let (a, b) = (pts[0], *pts.last().expect("len > 1"));
                    pts = vec![a, [b[0], a[1]], b, [a[0], b[1]]];
                }
                let pg = &self.doc.pages[page];
                let wanted = |i: &Item| {
                    pg.editable(i)
                        && match i.kind() {
                            ItemKind::Ink => prefs.lasso_ink,
                            ItemKind::Highlight => prefs.lasso_hl,
                            ItemKind::Text => prefs.lasso_text,
                            ItemKind::Shape => prefs.lasso_shapes,
                            ItemKind::Tape => prefs.lasso_tapes,
                            ItemKind::Image => prefs.lasso_images,
                        }
                };
                let ids: Vec<Id> = if len < 12.0 {
                    let p = pts[0];
                    pg.items
                        .iter()
                        .rev()
                        .find(|i| wanted(i) && geom::hit_item(i, p[0], p[1], 8.0 / self.zoom))
                        .map(|i| vec![i.id])
                        .unwrap_or_default()
                } else {
                    pg.items
                        .iter()
                        .filter(|i| wanted(i) && geom::item_in_lasso(i, &pts))
                        .map(|i| i.id)
                        .collect()
                };
                self.selection = (!ids.is_empty()).then_some(Selection { page, ids });
            }
            Action::Move { page, delta, .. } => {
                if delta[0].abs() + delta[1].abs() > 0.01 {
                    let _ = page;
                    self.map_selection(|it| geom::translate_item(it, delta[0], delta[1]));
                }
            }
            Action::Scale { anchor, factor, .. } => {
                if (factor - 1.0).abs() > 0.001 {
                    self.map_selection(|it| geom::scale_item(it, anchor[0], anchor[1], factor));
                }
            }
            Action::ShapeDrag { page, a, b } => {
                if geom::dist(a, b) * self.zoom > 4.0 {
                    let mut sh = Shape::new(
                        prefs.shape_kind,
                        vec![a, b],
                        prefs.shape_color,
                        mm(prefs.shape_mm),
                    );
                    sh.fill = prefs.shape_fill.then_some(prefs.shape_color);
                    sh.dashed = prefs.shape_dashed;
                    self.add_item(page, Item::new(Content::Shape(sh)));
                }
            }
            Action::Laser => {
                for l in &mut self.laser {
                    if l.1 < 0.0 {
                        l.1 = self.now;
                    }
                }
            }
            Action::Pan { .. } => {
                if tap {
                    self.tap_item(pos);
                }
            }
            Action::RulerMove { .. } | Action::Idle => {}
        }
    }

    /// Per-frame work: hold-to-snap timing and laser fade.
    fn tick(&mut self, prefs: &Prefs) {
        let now = self.now;
        let tool = self.tool;
        let zoom = self.zoom;
        if let Action::Ink {
            pts,
            still_since,
            snapped,
            ruler_edge,
            ..
        } = &mut self.action
            && prefs.shape_hold
            && snapped.is_none()
            && ruler_edge.is_none()
            && pts.len() > 6
            && now - *still_since > HOLD_SECS
        {
            let xy: Vec<[f32; 2]> = pts.iter().map(|p| [p.x, p.y]).collect();
            if let Some((kind, points)) = recognize(&xy)
                && (tool == Tool::Pen || kind == ShapeKind::Line)
            {
                *snapped = Some(Shape::new(kind, points, prefs.pen_color, mm(prefs.pen_mm)));
                let _ = zoom;
            }
            *still_since = now + 1e9; // try once per pause
        }
        self.laser.retain(|(_, t)| *t < 0.0 || now - *t < 1.0);
    }

    fn erase_at(&mut self, page: usize, p: [f32; 2], prefs: &Prefs) {
        let r = prefs.eraser_size / self.zoom;
        let pg = &self.doc.pages[page];
        let items = &pg.items;
        let hits: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                let eligible = pg.editable(it)
                    && match &it.content {
                        Content::Stroke(s) => {
                            !prefs.eraser_hl_only || s.pen == PenKind::Highlighter
                        }
                        Content::Shape(_) => !prefs.eraser_hl_only && prefs.eraser_shapes,
                        Content::Tape(_) => !prefs.eraser_hl_only && prefs.eraser_tapes,
                        Content::Text(_) | Content::Image(_) => false,
                    };
                eligible && geom::hit_item(it, p[0], p[1], r)
            })
            .map(|(i, _)| i)
            .collect();
        if hits.is_empty() {
            return;
        }
        if let Action::Erase { before, .. } = &mut self.action
            && before.is_none()
        {
            *before = Some(self.doc.pages.clone());
        }
        let page_ref = &mut self.doc.pages[page];
        let mut out: Vec<Arc<Item>> = Vec::with_capacity(page_ref.items.len());
        for (i, it) in page_ref.items.iter().enumerate() {
            if !hits.contains(&i) {
                out.push(it.clone());
                continue;
            }
            if prefs.eraser_precise
                && let Content::Stroke(s) = &it.content
            {
                for part in split_stroke(s, p, r) {
                    let mut piece = Item::new(Content::Stroke(part));
                    piece.layer = it.layer;
                    out.push(Arc::new(piece));
                }
            }
        }
        page_ref.items = out;
        self.dirty = true;
        self.last_edit = self.now;
    }

    fn project_on_ruler(
        &self,
        page: usize,
        pts: &[InkPoint],
        r: Ruler,
        edge: usize,
        width: f32,
    ) -> Vec<InkPoint> {
        let o = self.page_origin_doc(page);
        let side = if edge == 1 { 1.0 } else { -1.0 };
        let (d, n) = (r.dir(), r.normal());
        let off = side * (RULER_HALF + width / 2.0 + 0.5);
        let proj = |p: &InkPoint| {
            let doc = [p.x + o[0], p.y + o[1]];
            let t = r.along(doc);
            InkPoint {
                x: r.center[0] + d[0] * t + n[0] * off - o[0],
                y: r.center[1] + d[1] * t + n[1] * off - o[1],
                p: p.p,
            }
        };
        let (first, last) = (pts.first(), pts.last());
        match (first, last) {
            (Some(a), Some(b)) => vec![proj(a), proj(b)],
            _ => Vec::new(),
        }
    }
}

/// Pencil force → ink pressure. Typical writing force sits around a quarter of the
/// Pencil's maximum, so the raw value is boosted before the pen curve is applied.
fn pressure(force: Option<f32>, prefs: &Prefs) -> f32 {
    match force {
        Some(f) => {
            // Safari already reports Pencil force as 0..1; native iPadOS reports it raw.
            let boost = if cfg!(target_arch = "wasm32") {
                1.3
            } else {
                2.2
            };
            (f * boost * prefs.pressure_gain).clamp(0.03, 1.0)
        }
        None => 0.5,
    }
}

/// Removes the part of a stroke inside the eraser circle, keeping the rest as separate strokes.
fn split_stroke(s: &Stroke, c: [f32; 2], r: f32) -> Vec<Stroke> {
    let reach = r + s.width / 2.0;
    let mut parts = Vec::new();
    let mut cur: Vec<InkPoint> = Vec::new();
    for p in &s.points {
        if (p.x - c[0]).hypot(p.y - c[1]) <= reach {
            if cur.len() > 1 {
                parts.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        } else {
            cur.push(*p);
        }
    }
    if cur.len() > 1 {
        parts.push(cur);
    }
    parts
        .into_iter()
        .map(|points| Stroke {
            points,
            ..s.clone()
        })
        .collect()
}
