//! Document model: notebooks hold pages, pages hold items (ink, shapes, text).

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub type Id = u64;

/// Page size: A4 in PDF points (1 pt = 1/72 inch). All page coordinates use points.
pub const A4_W: f32 = 595.0;
pub const A4_H: f32 = 842.0;

/// Millimetres to points.
pub fn mm(v: f32) -> f32 {
    v * 72.0 / 25.4
}

pub fn now_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Unique ids: milliseconds since 1970 in the high bits, a counter in the low 16 bits.
/// Kept under 2^53 so they survive a round trip through any JSON reader.
pub fn new_id() -> Id {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let candidate = (now_ms() & 0x1F_FFFF_FFFF) << 16;
    let mut prev = LAST.load(Ordering::Relaxed);
    loop {
        let next = candidate.max(prev + 1);
        match LAST.compare_exchange_weak(prev, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(p) => prev = p,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub const BLACK: Rgba = Rgba(0x1d, 0x1d, 0x1f, 255);
    pub const WHITE: Rgba = Rgba(255, 255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Rgba(r, g, b, 255)
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let h = hex.trim_start_matches('#');
        if h.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(h, 16).ok()?;
        Some(Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255))
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// Mixes `self` over `base` with weight `t` (0 = base, 1 = self). Result is opaque.
    pub fn mix_over(self, base: Rgba, t: f32) -> Rgba {
        let m = |a: u8, b: u8| {
            (b as f32 + (a as f32 - b as f32) * t)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Rgba(m(self.0, base.0), m(self.1, base.1), m(self.2, base.2), 255)
    }

    pub fn luminance(self) -> f32 {
        (0.2126 * self.0 as f32 + 0.7152 * self.1 as f32 + 0.0722 * self.2 as f32) / 255.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum PenKind {
    /// Even line, like a gel pen or fineliner.
    Fineliner,
    /// Pressure plus nib angle: thick and thin strokes like a real fountain pen.
    Fountain,
    /// Strong pressure response with tapered ends.
    Brush,
    Highlighter,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct InkPoint {
    pub x: f32,
    pub y: f32,
    /// Pressure 0..1. Mouse input uses 0.5.
    pub p: f32,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Stroke {
    pub pen: PenKind,
    pub color: Rgba,
    /// Nominal width in points.
    pub width: f32,
    pub points: Vec<InkPoint>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ShapeKind {
    Line,
    Arrow,
    /// Two opposite corners.
    Rect,
    /// Bounding box corners.
    Ellipse,
    /// Closed polygon through all points.
    Polygon,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Shape {
    pub kind: ShapeKind,
    pub points: Vec<[f32; 2]>,
    pub color: Rgba,
    pub width: f32,
    #[serde(default)]
    pub fill: Option<Rgba>,
    #[serde(default)]
    pub dashed: bool,
    /// Opaque fill instead of a light tint.
    #[serde(default)]
    pub solid: bool,
    /// Dots instead of dashes (when `dashed`).
    #[serde(default)]
    pub dotted: bool,
}

impl Shape {
    pub fn new(kind: ShapeKind, points: Vec<[f32; 2]>, color: Rgba, width: f32) -> Self {
        Shape {
            kind,
            points,
            color,
            width,
            fill: None,
            dashed: false,
            solid: false,
            dotted: false,
        }
    }
}

/// Study tape: an opaque strip that hides what is under it until tapped.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Tape {
    /// Centre line (freehand or straight), or two opposite corners when `rect`.
    pub points: Vec<[f32; 2]>,
    pub width: f32,
    pub color: Rgba,
    #[serde(default)]
    pub rect: bool,
    #[serde(default)]
    pub revealed: bool,
}

/// A picture placed on the page. The JPEG bytes live in the asset store.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ImageBox {
    pub asset: Id,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Where tapping a linked item goes.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Link {
    Url(String),
    Page(Id),
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TextBox {
    pub x: f32,
    pub y: f32,
    /// Wrap width.
    pub w: f32,
    pub text: String,
    pub size: f32,
    pub color: Rgba,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Content {
    Stroke(Stroke),
    Shape(Shape),
    Text(TextBox),
    Tape(Tape),
    Image(ImageBox),
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: Id,
    pub content: Content,
    /// Layer the item sits on (see [`Page::layers`]).
    #[serde(default)]
    pub layer: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
}

impl Item {
    pub fn new(content: Content) -> Self {
        Item {
            id: new_id(),
            content,
            layer: 0,
            link: None,
        }
    }

    pub fn is_tape(&self) -> bool {
        matches!(self.content, Content::Tape(_))
    }

    pub fn kind(&self) -> ItemKind {
        match &self.content {
            Content::Stroke(s) if s.pen == PenKind::Highlighter => ItemKind::Highlight,
            Content::Stroke(_) => ItemKind::Ink,
            Content::Shape(_) => ItemKind::Shape,
            Content::Text(_) => ItemKind::Text,
            Content::Tape(_) => ItemKind::Tape,
            Content::Image(_) => ItemKind::Image,
        }
    }

    pub fn is_highlighter(&self) -> bool {
        matches!(&self.content, Content::Stroke(s) if s.pen == PenKind::Highlighter)
    }

    pub fn color(&self) -> Rgba {
        match &self.content {
            Content::Stroke(s) => s.color,
            Content::Shape(s) => s.color,
            Content::Text(t) => t.color,
            Content::Tape(t) => t.color,
            Content::Image(_) => Rgba::WHITE,
        }
    }

    pub fn with_color(&self, c: Rgba) -> Item {
        let mut it = self.clone();
        match &mut it.content {
            Content::Stroke(s) => s.color = c,
            Content::Shape(s) => s.color = c,
            Content::Text(t) => t.color = c,
            Content::Tape(t) => t.color = c,
            Content::Image(_) => {}
        }
        it
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ItemKind {
    Ink,
    Highlight,
    Shape,
    Text,
    Tape,
    Image,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Template {
    Blank,
    Lined,
    Narrow,
    Grid,
    Dotted,
    Cornell,
    Todo,
    Music,
    Storyboard,
    Ruled,
    MarginRight,
    Columns2,
    Columns3,
}

impl Template {
    pub const ALL: [Template; 13] = [
        Template::Blank,
        Template::Ruled,
        Template::Lined,
        Template::MarginRight,
        Template::Narrow,
        Template::Grid,
        Template::Dotted,
        Template::Cornell,
        Template::Columns2,
        Template::Columns3,
        Template::Todo,
        Template::Music,
        Template::Storyboard,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Template::Blank => "Blank",
            Template::Lined => "Margin",
            Template::Narrow => "Narrow lined",
            Template::Grid => "Grid",
            Template::Dotted => "Dotted",
            Template::Cornell => "Cornell",
            Template::Todo => "To-do",
            Template::Music => "Music",
            Template::Storyboard => "Storyboard",
            Template::Ruled => "Ruled",
            Template::MarginRight => "Margin right",
            Template::Columns2 => "2 Columns",
            Template::Columns3 => "3 Columns",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Paper {
    White,
    Cream,
    Gray,
    Night,
    Black,
    Rose,
    Pink,
    Lilac,
    Periwinkle,
    Sky,
    Mint,
    Sage,
}

impl Paper {
    pub const ALL: [Paper; 12] = [
        Paper::White,
        Paper::Cream,
        Paper::Gray,
        Paper::Night,
        Paper::Black,
        Paper::Rose,
        Paper::Pink,
        Paper::Lilac,
        Paper::Periwinkle,
        Paper::Sky,
        Paper::Mint,
        Paper::Sage,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Paper::White => "White",
            Paper::Cream => "Cream",
            Paper::Gray => "Gray",
            Paper::Night => "Night",
            Paper::Black => "Black",
            Paper::Rose => "Rose",
            Paper::Pink => "Pink",
            Paper::Lilac => "Lilac",
            Paper::Periwinkle => "Periwinkle",
            Paper::Sky => "Sky",
            Paper::Mint => "Mint",
            Paper::Sage => "Sage",
        }
    }

    pub fn background(self) -> Rgba {
        match self {
            Paper::White => Rgba::rgb(255, 255, 255),
            Paper::Cream => Rgba::rgb(0xfd, 0xf8, 0xe6),
            Paper::Gray => Rgba::rgb(0xec, 0xee, 0xf1),
            Paper::Night => Rgba::rgb(0x1f, 0x21, 0x26),
            Paper::Black => Rgba::rgb(0x0b, 0x0b, 0x0c),
            Paper::Rose => Rgba::rgb(0xfb, 0xe3, 0xe0),
            Paper::Pink => Rgba::rgb(0xfb, 0xe1, 0xee),
            Paper::Lilac => Rgba::rgb(0xec, 0xe3, 0xf8),
            Paper::Periwinkle => Rgba::rgb(0xe1, 0xe6, 0xfa),
            Paper::Sky => Rgba::rgb(0xdd, 0xef, 0xfb),
            Paper::Mint => Rgba::rgb(0xdf, 0xf5, 0xea),
            Paper::Sage => Rgba::rgb(0xe6, 0xee, 0xd9),
        }
    }

    /// Ruling colour, already blended with the background (opaque).
    pub fn line(self) -> Rgba {
        let bg = self.background();
        match self {
            Paper::Night | Paper::Black => Rgba::rgb(255, 255, 255).mix_over(bg, 0.13),
            Paper::Cream => Rgba::rgb(120, 100, 60).mix_over(bg, 0.28),
            _ => Rgba::rgb(60, 90, 150).mix_over(bg, 0.24),
        }
    }

    pub fn margin(self) -> Rgba {
        Rgba::rgb(225, 70, 70).mix_over(self.background(), 0.4)
    }

    pub fn is_dark(self) -> bool {
        matches!(self, Paper::Night | Paper::Black)
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Page {
    pub id: Id,
    pub w: f32,
    pub h: f32,
    pub template: Template,
    pub paper: Paper,
    pub items: Vec<Arc<Item>>,
    /// A bookmarked page carries a title; it also shows up in the outline.
    #[serde(default)]
    pub bookmark: Option<String>,
    /// Bottom to top. Every page has at least one layer once loaded.
    #[serde(default)]
    pub layers: Vec<Layer>,
    /// Picture drawn as the page background (imported PDF pages and photos).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Id>,
}

fn yes() -> bool {
    true
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub id: Id,
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
}

impl Layer {
    pub fn first() -> Self {
        Layer {
            id: 0,
            name: "Layer 1".into(),
            visible: true,
            locked: false,
        }
    }
}

/// Paper sizes in points, portrait.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum PaperSize {
    A4,
    Letter,
    A5,
    Legal,
    Square,
    Tablet,
}

impl PaperSize {
    pub const ALL: [PaperSize; 6] = [
        PaperSize::A4,
        PaperSize::Letter,
        PaperSize::A5,
        PaperSize::Legal,
        PaperSize::Square,
        PaperSize::Tablet,
    ];
    pub fn name(self) -> &'static str {
        match self {
            PaperSize::A4 => "A4",
            PaperSize::Letter => "Letter",
            PaperSize::A5 => "A5",
            PaperSize::Legal => "Legal",
            PaperSize::Square => "Square",
            PaperSize::Tablet => "Tablet screen",
        }
    }
    /// Width and height in points; `landscape` swaps them.
    pub fn dims(self, landscape: bool) -> (f32, f32) {
        let (w, h) = match self {
            PaperSize::A4 => (A4_W, A4_H),
            PaperSize::Letter => (612.0, 792.0),
            PaperSize::A5 => (420.0, 595.0),
            PaperSize::Legal => (612.0, 1008.0),
            PaperSize::Square => (595.0, 595.0),
            PaperSize::Tablet => (595.0, 793.0),
        };
        if landscape { (h, w) } else { (w, h) }
    }
}

impl Page {
    pub fn new(template: Template, paper: Paper) -> Self {
        Page {
            id: new_id(),
            w: A4_W,
            h: A4_H,
            template,
            paper,
            items: Vec::new(),
            bookmark: None,
            layers: vec![Layer::first()],
            background: None,
        }
    }

    pub fn sized(template: Template, paper: Paper, w: f32, h: f32) -> Self {
        Page {
            w,
            h,
            ..Page::new(template, paper)
        }
    }

    /// Copy of the page with a fresh id (items keep theirs; they are immutable).
    pub fn duplicate(&self) -> Self {
        Page {
            id: new_id(),
            ..self.clone()
        }
    }

    /// Ensures at least one layer and that every item points at an existing layer.
    pub fn normalize(&mut self) {
        if self.layers.is_empty() {
            self.layers.push(Layer::first());
        }
        let first = self.layers[0].id;
        let ids: Vec<Id> = self.layers.iter().map(|l| l.id).collect();
        for it in &mut self.items {
            if !ids.contains(&it.layer) {
                let mut c = (**it).clone();
                c.layer = first;
                *it = Arc::new(c);
            }
        }
    }

    /// Pages with a background picture or images get see-through highlighter.
    pub fn has_pictures(&self) -> bool {
        self.background.is_some()
            || self
                .items
                .iter()
                .any(|i| matches!(i.content, Content::Image(_)))
    }

    pub fn layer(&self, id: Id) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Can the item be seen and changed (its layer is visible and unlocked)?
    pub fn editable(&self, it: &Item) -> bool {
        self.layer(it.layer).is_none_or(|l| l.visible && !l.locked)
    }

    pub fn visible(&self, it: &Item) -> bool {
        self.layer(it.layer).is_none_or(|l| l.visible)
    }

    /// Items in drawing order: layer by layer, then the order they were added.
    pub fn items_in_layer(&self, layer: Id) -> impl Iterator<Item = &Arc<Item>> {
        self.items.iter().filter(move |i| i.layer == layer)
    }
}

/// An entry in a notebook's table of contents.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct OutlineEntry {
    pub id: Id,
    pub title: String,
    pub page: Id,
    #[serde(default)]
    pub level: u8,
}

/// A reusable group of items saved from the lasso.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Sticker {
    pub id: Id,
    pub items: Vec<Item>,
    #[serde(default)]
    pub used: u64,
}

/// The content of one notebook. Metadata lives in [`crate::library::NotebookMeta`].
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: Id,
    pub pages: Vec<Page>,
    #[serde(default)]
    pub outline: Vec<OutlineEntry>,
}

impl Document {
    pub fn new(id: Id, template: Template, paper: Paper) -> Self {
        Document {
            id,
            pages: vec![Page::new(template, paper)],
            outline: Vec::new(),
        }
    }

    pub fn sized(id: Id, template: Template, paper: Paper, w: f32, h: f32) -> Self {
        Document {
            id,
            pages: vec![Page::sized(template, paper, w, h)],
            outline: Vec::new(),
        }
    }

    pub fn normalize(&mut self) {
        if self.pages.is_empty() {
            self.pages.push(Page::new(Template::Blank, Paper::White));
        }
        for p in &mut self.pages {
            p.normalize();
        }
    }

    /// Every picture this notebook uses.
    pub fn asset_ids(&self) -> Vec<Id> {
        let mut v = Vec::new();
        for p in &self.pages {
            v.extend(p.background);
            for it in &p.items {
                if let Content::Image(im) = &it.content {
                    v.push(im.asset);
                }
            }
        }
        v.sort();
        v.dedup();
        v
    }

    pub fn page_index(&self, id: Id) -> Option<usize> {
        self.pages.iter().position(|p| p.id == id)
    }

    /// All typed text, used for library search.
    pub fn search_text(&self) -> String {
        let mut out = String::new();
        for p in &self.pages {
            if let Some(b) = &p.bookmark {
                out.push_str(b);
                out.push(' ');
            }
            for it in &p.items {
                if let Content::Text(t) = &it.content {
                    out.push_str(&t.text);
                    out.push(' ');
                }
            }
        }
        for o in &self.outline {
            out.push_str(&o.title);
            out.push(' ');
        }
        out.truncate(out.floor_char_boundary(20_000));
        out
    }
}
