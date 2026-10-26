//! The notebook editor: state, document operations and tool logic.
//! Rendering lives in `canvas.rs`, input in `input.rs`, chrome in `chrome.rs`.

mod canvas;
mod chrome;
mod input;
mod mesh;
mod sidebar;

use crate::prefs::Prefs;
use egui::{Pos2, Rect, Vec2, pos2, vec2};
use notebook_core::geom::{self, Rect as PRect};
use notebook_core::history::History;
use notebook_core::*;
use std::collections::HashMap;
use std::sync::Arc;

pub use mesh::RenderCache;

/// Space between pages, in page units.
pub const GAP: f32 = 28.0;
pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Pen,
    Highlighter,
    Tape,
    Eraser,
    Lasso,
    Hand,
    Laser,
    Shape,
    Text,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Popover {
    None,
    ToolSettings(Tool),
    Color,
    Insert,
    More,
    Template,
    Shapes,
    Stickers,
    ShapeStyle,
    MoveToLayer,
    Customize,
    GoTo,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SideTab {
    Pages,
    Bookmarks,
    Outline,
    Layers,
}

/// What a picked file is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportTarget {
    /// A picture placed on the current page.
    Image,
    /// PDF pages or a photo added as new pages after the current one.
    Pages,
}

/// Text being edited in a small dialog.
#[derive(Clone, Debug)]
pub enum EditDialog {
    Link {
        url: String,
        page: String,
        web: bool,
    },
    LayerName(usize, Id, String),
    OutlineTitle(Id, String),
    Find(String),
}

/// Study timer (stopwatch).
#[derive(Clone, Copy, Debug, Default)]
pub struct Timer {
    pub since: Option<f64>,
    pub acc: f64,
}

impl Timer {
    pub fn seconds(&self, now: f64) -> f64 {
        self.acc + self.since.map_or(0.0, |s| now - s)
    }
}

#[derive(Clone, Debug)]
pub struct Selection {
    pub page: usize,
    pub ids: Vec<Id>,
}

#[derive(Clone, Debug)]
pub enum Action {
    Idle,
    Ink {
        page: usize,
        pts: Vec<InkPoint>,
        smooth: [f32; 2],
        last_screen: Pos2,
        still_since: f64,
        snapped: Option<Shape>,
        /// Index of the ruler edge the stroke is locked to.
        ruler_edge: Option<usize>,
    },
    Erase {
        page: usize,
        before: Option<Vec<Page>>,
        last: [f32; 2],
    },
    Lasso {
        page: usize,
        pts: Vec<[f32; 2]>,
    },
    TapeDraw {
        page: usize,
        pts: Vec<[f32; 2]>,
    },
    Move {
        page: usize,
        start: [f32; 2],
        delta: [f32; 2],
    },
    Scale {
        page: usize,
        anchor: [f32; 2],
        start: f32,
        factor: f32,
    },
    ShapeDrag {
        page: usize,
        a: [f32; 2],
        b: [f32; 2],
    },
    Pan {
        last: Pos2,
    },
    Laser,
    RulerMove {
        last: Pos2,
    },
}

#[derive(Clone, Debug)]
pub struct TextEditState {
    pub page: usize,
    pub id: Option<Id>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub text: String,
    pub size: f32,
    pub color: Rgba,
    pub focused_once: bool,
}

/// A straight-edge that strokes snap to. Position in document units.
#[derive(Clone, Copy, Debug)]
pub struct Ruler {
    pub center: [f32; 2],
    pub angle: f32,
}

pub const RULER_HALF: f32 = 34.0;
pub const RULER_LEN: f32 = 1400.0;

impl Ruler {
    pub fn dir(&self) -> [f32; 2] {
        [self.angle.cos(), self.angle.sin()]
    }
    pub fn normal(&self) -> [f32; 2] {
        [-self.angle.sin(), self.angle.cos()]
    }
    /// Signed distance of a document point from the ruler's centre line.
    pub fn offset(&self, p: [f32; 2]) -> f32 {
        let n = self.normal();
        (p[0] - self.center[0]) * n[0] + (p[1] - self.center[1]) * n[1]
    }
    pub fn along(&self, p: [f32; 2]) -> f32 {
        let d = self.dir();
        (p[0] - self.center[0]) * d[0] + (p[1] - self.center[1]) * d[1]
    }
    pub fn contains(&self, p: [f32; 2]) -> bool {
        self.offset(p).abs() <= RULER_HALF && self.along(p).abs() <= RULER_LEN / 2.0
    }
}

#[derive(Default, Debug)]
pub struct Fingers {
    pub map: HashMap<u64, (Pos2, Pos2)>, // id → (start, current)
    pub started: f64,
    pub max_count: usize,
    pub moved: f32,
    pub pinch: Option<(f32, f32, Pos2)>, // (start distance, start zoom, last centre)
    pub ruler_angle: Option<(f32, f32)>, // (start finger angle, start ruler angle)
}

pub enum EditorEvent {
    Close,
    ExportPdf,
    ExportPng(usize),
    Toast(String),
}

pub struct Editor {
    pub meta_id: Id,
    pub title: String,
    pub doc: Document,
    pub history: History,
    pub tool: Tool,
    pub zoom: f32,
    /// Screen offset of the document origin from the canvas' top-left.
    pub offset: Vec2,
    pub fitted: bool,
    pub action: Action,
    pub selection: Option<Selection>,
    pub text_edit: Option<TextEditState>,
    pub laser: Vec<([f32; 2], f64)>,
    pub ruler: Option<Ruler>,
    pub dirty: bool,
    pub last_edit: f64,
    pub popover: Popover,
    pub popover_anchor: Rect,
    pub sidebar: bool,
    pub side_tab: SideTab,
    pub overview: bool,
    pub fullscreen: bool,
    pub fingers: Fingers,
    pub pen_touch: Option<u64>,
    pub mouse_down: bool,
    pub touch_mode: bool,
    pub cache: RenderCache,
    pub canvas: Rect,
    pub layer: egui::LayerId,
    pub tops: Vec<f32>,
    pub max_w: f32,
    pub current: usize,
    pub scroll_to: Option<usize>,
    pub events: Vec<EditorEvent>,
    pub now: f64,
    pub rename: Option<String>,
    pub bookmark_edit: Option<(usize, String)>,
    pub thumbs: crate::textures::PageThumbs,
    pub assets: crate::assets::Assets,
    /// Layer new items go to, per page id.
    pub active_layer: HashMap<Id, Id>,
    /// Pages to return to after following links.
    pub back_stack: Vec<usize>,
    pub edit: Option<EditDialog>,
    pub pending_import: Option<ImportTarget>,
    pub timer: Timer,
    pub prev_tool: Tool,
    /// Where and when the current touch started (for taps on links and tapes).
    pub tap_start: Option<(Pos2, f64)>,
    pub goto_text: String,
    /// Canvas width when the zoom was last fitted to the page width.
    pub fit_canvas_w: f32,
}

impl Editor {
    pub fn new(meta_id: Id, title: String, mut doc: Document, assets: crate::assets::Assets) -> Self {
        doc.normalize();
        Editor {
            meta_id,
            title,
            doc,
            history: History::default(),
            tool: Tool::Pen,
            zoom: 1.0,
            offset: Vec2::ZERO,
            fitted: false,
            action: Action::Idle,
            selection: None,
            text_edit: None,
            laser: Vec::new(),
            ruler: None,
            dirty: false,
            last_edit: 0.0,
            popover: Popover::None,
            popover_anchor: Rect::NOTHING,
            sidebar: false,
            side_tab: SideTab::Pages,
            overview: false,
            fullscreen: false,
            fingers: Fingers::default(),
            pen_touch: None,
            mouse_down: false,
            touch_mode: false,
            cache: RenderCache::default(),
            canvas: Rect::NOTHING,
            layer: egui::LayerId::background(),
            tops: Vec::new(),
            max_w: A4_W,
            current: 0,
            scroll_to: None,
            events: Vec::new(),
            now: 0.0,
            rename: None,
            bookmark_edit: None,
            thumbs: Default::default(),
            assets,
            active_layer: HashMap::new(),
            back_stack: Vec::new(),
            edit: None,
            pending_import: None,
            timer: Timer::default(),
            prev_tool: Tool::Pen,
            tap_start: None,
            goto_text: String::new(),
            fit_canvas_w: 0.0,
        }
    }

    // ---------- layout & coordinates ----------

    pub fn layout(&mut self) {
        self.tops.clear();
        let mut y = 0.0;
        self.max_w = self.doc.pages.iter().map(|p| p.w).fold(0.0, f32::max);
        for p in &self.doc.pages {
            self.tops.push(y);
            y += p.h + GAP;
        }
    }

    pub fn doc_height(&self) -> f32 {
        self.tops
            .last()
            .map(|t| t + self.doc.pages.last().map_or(0.0, |p| p.h))
            .unwrap_or(0.0)
    }

    pub fn page_origin_doc(&self, i: usize) -> [f32; 2] {
        let p = &self.doc.pages[i];
        [
            (self.max_w - p.w) / 2.0,
            self.tops.get(i).copied().unwrap_or(0.0),
        ]
    }

    pub fn doc_to_screen(&self, d: [f32; 2]) -> Pos2 {
        self.canvas.min + self.offset + vec2(d[0], d[1]) * self.zoom
    }

    pub fn screen_to_doc(&self, p: Pos2) -> [f32; 2] {
        let v = (p - self.canvas.min - self.offset) / self.zoom;
        [v.x, v.y]
    }

    pub fn page_to_screen(&self, i: usize, p: [f32; 2]) -> Pos2 {
        let o = self.page_origin_doc(i);
        self.doc_to_screen([o[0] + p[0], o[1] + p[1]])
    }

    pub fn screen_to_page(&self, i: usize, p: Pos2) -> [f32; 2] {
        let d = self.screen_to_doc(p);
        let o = self.page_origin_doc(i);
        [d[0] - o[0], d[1] - o[1]]
    }

    pub fn page_rect(&self, i: usize) -> Rect {
        let p = &self.doc.pages[i];
        Rect::from_min_max(
            self.page_to_screen(i, [0.0, 0.0]),
            self.page_to_screen(i, [p.w, p.h]),
        )
    }

    /// The page under a screen point (with a small margin around each page).
    pub fn page_at(&self, pos: Pos2) -> Option<usize> {
        let m = GAP / 2.0 * self.zoom;
        (0..self.doc.pages.len()).find(|&i| self.page_rect(i).expand(m).contains(pos))
    }

    pub fn fit_width(&mut self) {
        let avail = self.canvas.width() - 32.0;
        self.zoom = (avail / self.max_w).clamp(MIN_ZOOM, 2.2);
        self.offset.x = (self.canvas.width() - self.max_w * self.zoom) / 2.0;
    }

    pub fn zoom_at(&mut self, screen: Pos2, new_zoom: f32) {
        let z = new_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let d = self.screen_to_doc(screen);
        self.zoom = z;
        self.offset = (screen - self.canvas.min) - vec2(d[0], d[1]) * z;
        self.clamp_offset();
    }

    pub fn clamp_offset(&mut self) {
        let cw = self.max_w * self.zoom;
        let ch = self.doc_height() * self.zoom;
        let (vw, vh) = (self.canvas.width(), self.canvas.height());
        if cw + 32.0 <= vw {
            self.offset.x = (vw - cw) / 2.0;
        } else {
            self.offset.x = self.offset.x.clamp(vw - cw - 16.0, 16.0);
        }
        let top = 20.0;
        let bottom = vh - ch - vh * 0.45;
        self.offset.y = self.offset.y.clamp(bottom.min(top), top);
    }

    pub fn scroll_to_page(&mut self, i: usize) {
        if let Some(t) = self.tops.get(i) {
            self.offset.y = 20.0 - t * self.zoom;
            self.clamp_offset();
        }
    }

    /// The page nearest the middle of the screen.
    pub fn update_current(&mut self) {
        let mid = self.screen_to_doc(self.canvas.center())[1];
        let mut best = 0;
        for (i, t) in self.tops.iter().enumerate() {
            if *t <= mid {
                best = i;
            }
        }
        self.current = best.min(self.doc.pages.len().saturating_sub(1));
    }

    // ---------- history ----------

    pub fn snapshot(&self) -> Vec<Page> {
        self.doc.pages.clone()
    }

    pub fn commit(&mut self, before: Vec<Page>) {
        self.history.push(before);
        self.touch();
    }

    pub fn touch(&mut self) {
        self.dirty = true;
        self.last_edit = self.now;
    }

    pub fn undo(&mut self) {
        self.finish_text();
        if let Some(p) = self.history.undo(self.snapshot()) {
            self.doc.pages = p;
            self.after_structure_change();
        }
    }

    pub fn redo(&mut self) {
        self.finish_text();
        if let Some(p) = self.history.redo(self.snapshot()) {
            self.doc.pages = p;
            self.after_structure_change();
        }
    }

    fn after_structure_change(&mut self) {
        self.selection = None;
        self.action = Action::Idle;
        self.touch();
        self.layout();
        self.current = self.current.min(self.doc.pages.len() - 1);
    }

    // ---------- layers ----------

    /// The layer new items on page `i` go to (the top layer unless another was chosen).
    pub fn active_layer_id(&self, i: usize) -> Id {
        let page = &self.doc.pages[i];
        self.active_layer
            .get(&page.id)
            .copied()
            .filter(|id| page.layer(*id).is_some())
            .or_else(|| page.layers.last().map(|l| l.id))
            .unwrap_or(0)
    }

    /// Can new items be added to page `i` (active layer visible and unlocked)?
    pub fn can_write(&self, i: usize) -> bool {
        let page = &self.doc.pages[i];
        page.layer(self.active_layer_id(i))
            .is_none_or(|l| l.visible && !l.locked)
    }

    pub fn add_layer(&mut self, i: usize) {
        let before = self.snapshot();
        let page = &mut self.doc.pages[i];
        let n = page.layers.len() + 1;
        let id = new_id();
        page.layers.push(Layer {
            id,
            name: format!("Layer {n}"),
            visible: true,
            locked: false,
        });
        let pid = page.id;
        self.commit(before);
        self.active_layer.insert(pid, id);
    }

    pub fn update_layer(&mut self, i: usize, id: Id, f: impl FnOnce(&mut Layer)) {
        let before = self.snapshot();
        if let Some(l) = self.doc.pages[i].layers.iter_mut().find(|l| l.id == id) {
            f(l);
        }
        self.commit(before);
        self.selection = None;
    }

    pub fn delete_layer(&mut self, i: usize, id: Id) {
        if self.doc.pages[i].layers.len() <= 1 {
            self.toast("A page needs at least one layer");
            return;
        }
        let before = self.snapshot();
        let page = &mut self.doc.pages[i];
        page.layers.retain(|l| l.id != id);
        page.items.retain(|it| it.layer != id);
        self.commit(before);
        self.selection = None;
    }

    pub fn duplicate_layer(&mut self, i: usize, id: Id) {
        let before = self.snapshot();
        let page = &mut self.doc.pages[i];
        let Some(pos) = page.layers.iter().position(|l| l.id == id) else {
            return;
        };
        let nid = new_id();
        let mut l = page.layers[pos].clone();
        l.id = nid;
        l.name = format!("{} copy", l.name);
        page.layers.insert(pos + 1, l);
        let copies: Vec<Arc<Item>> = page
            .items
            .iter()
            .filter(|it| it.layer == id)
            .map(|it| {
                let mut c = (**it).clone();
                c.id = new_id();
                c.layer = nid;
                Arc::new(c)
            })
            .collect();
        page.items.extend(copies);
        self.commit(before);
    }

    /// Moves layer `id` one step up (towards the top) or down.
    pub fn move_layer(&mut self, i: usize, id: Id, up: bool) {
        let page = &self.doc.pages[i];
        let Some(pos) = page.layers.iter().position(|l| l.id == id) else {
            return;
        };
        let to = if up { pos + 1 } else { pos.wrapping_sub(1) };
        if to >= page.layers.len() {
            return;
        }
        let before = self.snapshot();
        self.doc.pages[i].layers.swap(pos, to);
        self.commit(before);
    }

    /// Moves everything on layer `id` into the layer below it and removes `id`.
    pub fn merge_down(&mut self, i: usize, id: Id) {
        let page = &self.doc.pages[i];
        let Some(pos) = page.layers.iter().position(|l| l.id == id) else {
            return;
        };
        if pos == 0 {
            return;
        }
        let below = page.layers[pos - 1].id;
        let before = self.snapshot();
        let page = &mut self.doc.pages[i];
        for it in page.items.iter_mut() {
            if it.layer == id {
                let mut c = (**it).clone();
                c.layer = below;
                *it = Arc::new(c);
            }
        }
        page.layers.remove(pos);
        self.commit(before);
    }

    pub fn move_selection_to_layer(&mut self, layer: Id) {
        self.map_selection(|it| {
            let mut c = it.clone();
            c.layer = layer;
            c
        });
    }

    // ---------- items ----------

    pub fn add_item(&mut self, page: usize, mut item: Item) {
        item.layer = self.active_layer_id(page);
        let before = self.snapshot();
        self.doc.pages[page].items.push(Arc::new(item));
        self.commit(before);
    }

    /// Adds several items as one undo step and selects them.
    pub fn add_items(&mut self, page: usize, items: Vec<Item>) {
        let layer = self.active_layer_id(page);
        let before = self.snapshot();
        let mut ids = Vec::new();
        for mut it in items {
            it.layer = layer;
            it.id = new_id();
            ids.push(it.id);
            self.doc.pages[page].items.push(Arc::new(it));
        }
        self.commit(before);
        self.selection = Some(Selection { page, ids });
        self.tool = Tool::Lasso;
    }

    /// Top-most visible item of a kind-filter under a page point.
    pub fn item_at(&self, page: usize, p: [f32; 2], r: f32, f: impl Fn(&Item) -> bool) -> Option<Arc<Item>> {
        let pg = &self.doc.pages[page];
        pg.items
            .iter()
            .rev()
            .find(|i| pg.visible(i) && f(i) && geom::hit_item(i, p[0], p[1], r))
            .cloned()
    }

    /// Flips a tape between covering and revealed. Not an undo step.
    pub fn toggle_tape(&mut self, page: usize, id: Id) {
        let items = &mut self.doc.pages[page].items;
        if let Some(it) = items.iter_mut().find(|i| i.id == id) {
            let mut c = (**it).clone();
            if let Content::Tape(t) = &mut c.content {
                t.revealed = !t.revealed;
            }
            *it = Arc::new(c);
            self.touch();
        }
    }

    /// Covers (or reveals) every tape on the current page.
    pub fn set_all_tapes(&mut self, revealed: bool) {
        let page = &mut self.doc.pages[self.current];
        let mut n = 0;
        for it in page.items.iter_mut() {
            if let Content::Tape(t) = &it.content
                && t.revealed != revealed
            {
                let mut c = (**it).clone();
                if let Content::Tape(t) = &mut c.content {
                    t.revealed = revealed;
                }
                *it = Arc::new(c);
                n += 1;
            }
        }
        if n > 0 {
            self.touch();
        } else {
            self.toast("No tape on this page");
        }
    }

    /// Follows a link: web pages open in a new tab, page links jump inside the notebook.
    pub fn follow_link(&mut self, link: &Link) {
        match link {
            Link::Url(u) => crate::platform::open_url(u),
            Link::Page(id) => {
                if let Some(i) = self.doc.page_index(*id) {
                    self.back_stack.push(self.current);
                    if self.back_stack.len() > 20 {
                        self.back_stack.remove(0);
                    }
                    self.scroll_to = Some(i);
                } else {
                    self.toast("That page no longer exists");
                }
            }
        }
    }

    pub fn go_back(&mut self) {
        if let Some(i) = self.back_stack.pop() {
            self.scroll_to = Some(i.min(self.doc.pages.len() - 1));
        }
    }

    /// A tap (finger, hand tool, or a tiny pen dot) on tape or a link.
    /// Returns true when it did something. Pen dots only toggle tape.
    pub fn tap_item(&mut self, pos: Pos2) -> bool {
        self.tap_item_with(pos, true)
    }

    pub fn tap_item_with(&mut self, pos: Pos2, links: bool) -> bool {
        let Some(page) = self.page_at(pos) else {
            return false;
        };
        let p = self.screen_to_page(page, pos);
        let r = 6.0 / self.zoom;
        if let Some(t) = self.item_at(page, p, r, |i| i.is_tape()) {
            self.toggle_tape(page, t.id);
            return true;
        }
        if links
            && let Some(it) = self.item_at(page, p, r, |i| i.link.is_some())
            && let Some(l) = it.link.clone()
        {
            self.follow_link(&l);
            return true;
        }
        false
    }

    pub fn set_selection_link(&mut self, link: Option<Link>) {
        self.map_selection(|it| {
            let mut c = it.clone();
            c.link = link.clone();
            c
        });
    }

    pub fn send_to_back(&mut self) {
        let Some(sel) = self.selection.clone() else {
            return;
        };
        let before = self.snapshot();
        let items = &mut self.doc.pages[sel.page].items;
        let (sel_items, rest): (Vec<_>, Vec<_>) =
            items.drain(..).partition(|i| sel.ids.contains(&i.id));
        items.extend(sel_items);
        items.extend(rest);
        self.commit(before);
    }

    /// Saves the selection as a sticker (positioned at the origin).
    pub fn selection_as_sticker(&self) -> Option<Sticker> {
        let items = self.selected_items();
        let b = geom::union_bbox(items.iter().map(|a| a.as_ref()))?;
        let items = items
            .iter()
            .map(|it| {
                let mut c = geom::translate_item(it, -b.x, -b.y);
                c.layer = 0;
                c
            })
            .collect();
        Some(Sticker {
            id: new_id(),
            items,
            used: now_ms(),
        })
    }

    /// Drops a sticker in the middle of the visible part of the current page.
    pub fn place_sticker(&mut self, st: &Sticker) {
        let page = self.current;
        if !self.can_write(page) {
            self.toast("The active layer is locked or hidden");
            return;
        }
        let Some(b) = geom::union_bbox(st.items.iter()) else {
            return;
        };
        let c = self.visible_center(page);
        let (dx, dy) = (c[0] - b.center()[0], c[1] - b.center()[1]);
        let items = st
            .items
            .iter()
            .map(|it| geom::translate_item(it, dx, dy))
            .collect();
        self.add_items(page, items);
    }

    // ---------- outline ----------

    pub fn add_outline(&mut self) {
        let page = self.doc.pages[self.current].id;
        let title = self.doc.pages[self.current]
            .bookmark
            .clone()
            .unwrap_or_else(|| format!("Page {}", self.current + 1));
        let entry = OutlineEntry {
            id: new_id(),
            title: title.clone(),
            page,
            level: 0,
        };
        // Keep the outline in page order.
        let order = |e: &OutlineEntry| self.doc.page_index(e.page).unwrap_or(usize::MAX);
        let at = self
            .doc
            .outline
            .iter()
            .position(|e| order(e) > self.current)
            .unwrap_or(self.doc.outline.len());
        self.doc.outline.insert(at, entry.clone());
        self.touch();
        self.edit = Some(EditDialog::OutlineTitle(entry.id, title));
    }

    // ---------- pictures ----------

    /// Handles a file the user picked.
    pub fn import(&mut self, target: ImportTarget, file: crate::platform::Imported) {
        match file.kind.as_str() {
            "cancel" => return,
            "error" => {
                self.toast(format!("Import failed: {}", file.name));
                return;
            }
            _ => {}
        }
        let mut stored = Vec::new();
        for (w, h, bytes) in &file.pages {
            match self.assets.add_jpeg(bytes) {
                Ok(id) => stored.push((*w, *h, id)),
                Err(e) => {
                    self.toast(format!("Could not save the picture: {e}"));
                    return;
                }
            }
        }
        let page = self.current;
        match (target, file.kind.as_str()) {
            (ImportTarget::Image, "image") => {
                let Some(&(w, h, asset)) = stored.first() else {
                    return;
                };
                if !self.can_write(page) {
                    self.toast("The active layer is locked or hidden");
                    return;
                }
                let pw = self.doc.pages[page].w;
                let s = (pw * 0.6 / w).min(1.0).min(pw * 0.6 / h);
                let (iw, ih) = (w * s, h * s);
                let c = self.visible_center(page);
                let item = Item::new(Content::Image(ImageBox {
                    asset,
                    x: c[0] - iw / 2.0,
                    y: c[1] - ih / 2.0,
                    w: iw,
                    h: ih,
                }));
                self.add_items(page, vec![item]);
            }
            _ => {
                // New pages after the current one, each showing one picture.
                let before = self.snapshot();
                let base_w = self.doc.pages[page].w;
                for (k, (w, h, asset)) in stored.iter().enumerate() {
                    let (pw, ph) = if file.kind == "pdf" {
                        (*w, *h)
                    } else {
                        (base_w, base_w * h / w)
                    };
                    let mut p = Page::sized(Template::Blank, Paper::White, pw, ph);
                    p.background = Some(*asset);
                    self.doc.pages.insert(page + 1 + k, p);
                }
                self.commit(before);
                self.layout();
                self.scroll_to = Some(page + 1);
                self.toast(format!("Added {} page(s)", stored.len()));
            }
        }
    }

    pub fn selected_items(&self) -> Vec<Arc<Item>> {
        let Some(sel) = &self.selection else {
            return Vec::new();
        };
        let Some(page) = self.doc.pages.get(sel.page) else {
            return Vec::new();
        };
        page.items
            .iter()
            .filter(|i| sel.ids.contains(&i.id))
            .cloned()
            .collect()
    }

    pub fn selection_bbox(&self) -> Option<PRect> {
        let items = self.selected_items();
        geom::union_bbox(items.iter().map(|a| a.as_ref()))
    }

    /// Replaces each selected item with `f(item)`, keeping stacking order.
    pub fn map_selection(&mut self, f: impl Fn(&Item) -> Item) {
        let Some(sel) = self.selection.clone() else {
            return;
        };
        let before = self.snapshot();
        let page = &mut self.doc.pages[sel.page];
        for it in page.items.iter_mut() {
            if sel.ids.contains(&it.id) {
                *it = Arc::new(f(it));
            }
        }
        self.commit(before);
    }

    pub fn delete_selection(&mut self) {
        let Some(sel) = self.selection.take() else {
            return;
        };
        let before = self.snapshot();
        self.doc.pages[sel.page]
            .items
            .retain(|i| !sel.ids.contains(&i.id));
        self.commit(before);
    }

    pub fn copy_selection(&self, clipboard: &mut Vec<Item>) {
        *clipboard = self
            .selected_items()
            .iter()
            .map(|a| (**a).clone())
            .collect();
    }

    pub fn duplicate_selection(&mut self) {
        let items = self.selected_items();
        let Some(sel) = &self.selection else { return };
        let page = sel.page;
        let before = self.snapshot();
        let mut ids = Vec::new();
        for it in items {
            let mut c = geom::translate_item(&it, 16.0, 16.0);
            c.id = new_id();
            ids.push(c.id);
            self.doc.pages[page].items.push(Arc::new(c));
        }
        self.commit(before);
        self.selection = Some(Selection { page, ids });
    }

    /// Pastes items centred on the visible part of the current page.
    pub fn paste(&mut self, clipboard: &[Item]) {
        if clipboard.is_empty() {
            return;
        }
        let page = self.current;
        let Some(bbox) = geom::union_bbox(clipboard.iter()) else {
            return;
        };
        let vis = self.visible_center(page);
        let (dx, dy) = (vis[0] - bbox.center()[0], vis[1] - bbox.center()[1]);
        let before = self.snapshot();
        let mut ids = Vec::new();
        let layer = self.active_layer_id(page);
        for it in clipboard {
            let mut c = geom::translate_item(it, dx, dy);
            c.id = new_id();
            c.layer = layer;
            ids.push(c.id);
            self.doc.pages[page].items.push(Arc::new(c));
        }
        self.commit(before);
        self.selection = Some(Selection { page, ids });
        self.tool = Tool::Lasso;
    }

    pub fn bring_to_front(&mut self) {
        let Some(sel) = self.selection.clone() else {
            return;
        };
        let before = self.snapshot();
        let items = &mut self.doc.pages[sel.page].items;
        let (sel_items, rest): (Vec<_>, Vec<_>) =
            items.drain(..).partition(|i| sel.ids.contains(&i.id));
        items.extend(rest);
        items.extend(sel_items);
        self.commit(before);
    }

    pub fn select_all(&mut self) {
        let page = self.current;
        let pg = &self.doc.pages[page];
        let ids: Vec<Id> = pg.items.iter().filter(|i| pg.editable(i)).map(|i| i.id).collect();
        if !ids.is_empty() {
            self.selection = Some(Selection { page, ids });
            self.tool = Tool::Lasso;
        }
    }

    /// Centre of the part of page `i` currently on screen, in page units.
    pub fn visible_center(&self, i: usize) -> [f32; 2] {
        let r = self.page_rect(i).intersect(self.canvas);
        let c = if r.is_positive() {
            r.center()
        } else {
            self.page_rect(i).center()
        };
        let p = self.screen_to_page(i, c);
        let page = &self.doc.pages[i];
        [
            p[0].clamp(40.0, page.w - 40.0),
            p[1].clamp(40.0, page.h - 40.0),
        ]
    }

    // ---------- pages ----------

    pub fn insert_page(&mut self, after: usize) {
        let before = self.snapshot();
        let src = &self.doc.pages[after.min(self.doc.pages.len() - 1)];
        let mut p = Page::new(src.template, src.paper);
        p.w = src.w;
        p.h = src.h;
        self.doc.pages.insert(after + 1, p);
        self.commit(before);
        self.layout();
        self.scroll_to = Some(after + 1);
    }

    pub fn duplicate_page(&mut self, i: usize) {
        let before = self.snapshot();
        let p = self.doc.pages[i].duplicate();
        self.doc.pages.insert(i + 1, p);
        self.commit(before);
        self.layout();
    }

    pub fn delete_page(&mut self, i: usize) {
        let before = self.snapshot();
        if self.doc.pages.len() == 1 {
            let p = &self.doc.pages[0];
            self.doc.pages[0] = Page::new(p.template, p.paper);
        } else {
            self.doc.pages.remove(i);
        }
        self.commit(before);
        self.after_structure_change();
    }

    pub fn move_page(&mut self, i: usize, down: bool) {
        let j = if down { i + 1 } else { i.wrapping_sub(1) };
        if j >= self.doc.pages.len() {
            return;
        }
        let before = self.snapshot();
        self.doc.pages.swap(i, j);
        self.commit(before);
        self.layout();
        self.current = j;
        self.scroll_to = Some(j);
    }

    pub fn clear_page(&mut self, i: usize) {
        let before = self.snapshot();
        self.doc.pages[i].items.clear();
        self.commit(before);
        self.selection = None;
    }

    /// Changes the size of the current page (or all pages). Content stays where it is.
    pub fn set_page_size(&mut self, w: f32, h: f32, all: bool) {
        let before = self.snapshot();
        for (i, p) in self.doc.pages.iter_mut().enumerate() {
            if (all || i == self.current) && p.background.is_none() {
                p.w = w;
                p.h = h;
            }
        }
        self.commit(before);
        self.layout();
        self.fitted = false;
    }

    /// Pages whose typed text, bookmark or outline title contains `q`.
    pub fn find(&self, q: &str) -> Vec<(usize, String)> {
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (i, p) in self.doc.pages.iter().enumerate() {
            let mut texts: Vec<String> = p.bookmark.iter().cloned().collect();
            for it in &p.items {
                if let Content::Text(t) = &it.content {
                    texts.push(t.text.clone());
                }
            }
            for o in &self.doc.outline {
                if o.page == p.id {
                    texts.push(o.title.clone());
                }
            }
            for t in texts {
                if let Some(pos) = t.to_lowercase().find(&q) {
                    let start = t.floor_char_boundary(pos.saturating_sub(24));
                    let end = t.floor_char_boundary((pos + q.len() + 40).min(t.len()));
                    out.push((i, t[start..end].replace('\n', " ")));
                }
            }
        }
        out
    }

    pub fn set_template(&mut self, t: Template, paper: Paper, all: bool) {
        let before = self.snapshot();
        for (i, p) in self.doc.pages.iter_mut().enumerate() {
            if all || i == self.current {
                p.template = t;
                p.paper = paper;
            }
        }
        self.commit(before);
    }

    pub fn set_bookmark(&mut self, i: usize, name: Option<String>) {
        let before = self.snapshot();
        self.doc.pages[i].bookmark = name;
        self.commit(before);
    }

    // ---------- text ----------

    pub fn start_text(&mut self, page: usize, at: [f32; 2], prefs: &Prefs) {
        self.finish_text();
        // Tap on existing text edits it.
        let pg = &self.doc.pages[page];
        let hit = pg
            .items
            .iter()
            .rev()
            .find(|i| {
                pg.editable(i)
                    && matches!(i.content, Content::Text(_))
                    && geom::hit_item(i, at[0], at[1], 4.0)
            })
            .cloned();
        if let Some(it) = hit
            && let Content::Text(t) = &it.content
        {
            self.text_edit = Some(TextEditState {
                page,
                id: Some(it.id),
                x: t.x,
                y: t.y,
                w: t.w,
                text: t.text.clone(),
                size: t.size,
                color: t.color,
                focused_once: false,
            });
            return;
        }
        let pw = self.doc.pages[page].w;
        let w = (pw - at[0] - 28.0).clamp(80.0, 380.0);
        let size = prefs.text_size;
        let y = at[1] - size * text::LINE_HEIGHT / 2.0;
        self.text_edit = Some(TextEditState {
            page,
            id: None,
            x: at[0],
            y,
            w,
            text: String::new(),
            size,
            color: prefs.text_color,
            focused_once: false,
        });
    }

    pub fn finish_text(&mut self) {
        let Some(te) = self.text_edit.take() else {
            return;
        };
        if te.page >= self.doc.pages.len() {
            return;
        }
        let trimmed = te.text.trim_end().to_string();
        let before = self.snapshot();
        let items = &mut self.doc.pages[te.page].items;
        match te.id {
            Some(id) => {
                let Some(idx) = items.iter().position(|i| i.id == id) else {
                    return;
                };
                if trimmed.is_empty() {
                    items.remove(idx);
                } else {
                    let new = TextBox {
                        x: te.x,
                        y: te.y,
                        w: te.w,
                        text: trimmed,
                        size: te.size,
                        color: te.color,
                    };
                    if items[idx].content == Content::Text(new.clone()) {
                        return;
                    }
                    let mut it = (*items[idx]).clone();
                    it.content = Content::Text(new);
                    items[idx] = Arc::new(it);
                }
            }
            None => {
                if trimmed.is_empty() {
                    return;
                }
                let layer = self
                    .active_layer
                    .get(&self.doc.pages[te.page].id)
                    .copied()
                    .filter(|id| self.doc.pages[te.page].layer(*id).is_some())
                    .or_else(|| self.doc.pages[te.page].layers.last().map(|l| l.id))
                    .unwrap_or(0);
                let items = &mut self.doc.pages[te.page].items;
                let mut item = Item::new(Content::Text(TextBox {
                    x: te.x,
                    y: te.y,
                    w: te.w,
                    text: trimmed,
                    size: te.size,
                    color: te.color,
                }));
                item.layer = layer;
                items.push(Arc::new(item));
            }
        }
        self.commit(before);
    }

    pub fn set_tool(&mut self, t: Tool) {
        if self.tool != t {
            if t == Tool::Eraser {
                self.prev_tool = self.tool;
            }
            self.finish_text();
            if t != Tool::Lasso {
                self.selection = None;
            }
        }
        self.tool = t;
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        self.events.push(EditorEvent::Toast(s.into()));
    }

    pub fn search_text(&self) -> String {
        self.doc.search_text()
    }

    pub fn first_page(&self) -> &Page {
        &self.doc.pages[0]
    }

    pub fn page_count_label(&self) -> String {
        format!("{} / {}", self.current + 1, self.doc.pages.len())
    }

    #[allow(dead_code)]
    pub fn pos(&self, x: f32, y: f32) -> Pos2 {
        pos2(x, y)
    }
}
