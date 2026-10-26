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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SideTab {
    Pages,
    Bookmarks,
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
}

impl Editor {
    pub fn new(meta_id: Id, title: String, doc: Document) -> Self {
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

    // ---------- items ----------

    pub fn add_item(&mut self, page: usize, item: Item) {
        let before = self.snapshot();
        self.doc.pages[page].items.push(Arc::new(item));
        self.commit(before);
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
        for it in clipboard {
            let mut c = geom::translate_item(it, dx, dy);
            c.id = new_id();
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
        let ids: Vec<Id> = self.doc.pages[page].items.iter().map(|i| i.id).collect();
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
        let hit = self.doc.pages[page]
            .items
            .iter()
            .rev()
            .find(|i| matches!(i.content, Content::Text(_)) && geom::hit_item(i, at[0], at[1], 4.0))
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
                    items[idx] = Arc::new(Item {
                        id,
                        content: Content::Text(new),
                    });
                }
            }
            None => {
                if trimmed.is_empty() {
                    return;
                }
                items.push(Arc::new(Item::new(Content::Text(TextBox {
                    x: te.x,
                    y: te.y,
                    w: te.w,
                    text: trimmed,
                    size: te.size,
                    color: te.color,
                }))));
            }
        }
        self.commit(before);
    }

    pub fn set_tool(&mut self, t: Tool) {
        if self.tool != t {
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
