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
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: Id,
    pub content: Content,
}

impl Item {
    pub fn new(content: Content) -> Self {
        Item {
            id: new_id(),
            content,
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
        }
    }

    pub fn with_color(&self, c: Rgba) -> Item {
        let mut it = self.clone();
        match &mut it.content {
            Content::Stroke(s) => s.color = c,
            Content::Shape(s) => s.color = c,
            Content::Text(t) => t.color = c,
        }
        it
    }
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
}

impl Template {
    pub const ALL: [Template; 9] = [
        Template::Blank,
        Template::Lined,
        Template::Narrow,
        Template::Grid,
        Template::Dotted,
        Template::Cornell,
        Template::Todo,
        Template::Music,
        Template::Storyboard,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Template::Blank => "Blank",
            Template::Lined => "Lined",
            Template::Narrow => "Narrow lined",
            Template::Grid => "Grid",
            Template::Dotted => "Dotted",
            Template::Cornell => "Cornell",
            Template::Todo => "To-do",
            Template::Music => "Music",
            Template::Storyboard => "Storyboard",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Paper {
    White,
    Cream,
    Gray,
    Night,
}

impl Paper {
    pub const ALL: [Paper; 4] = [Paper::White, Paper::Cream, Paper::Gray, Paper::Night];

    pub fn name(self) -> &'static str {
        match self {
            Paper::White => "White",
            Paper::Cream => "Cream",
            Paper::Gray => "Gray",
            Paper::Night => "Night",
        }
    }

    pub fn background(self) -> Rgba {
        match self {
            Paper::White => Rgba::rgb(255, 255, 255),
            Paper::Cream => Rgba::rgb(0xfd, 0xf8, 0xe6),
            Paper::Gray => Rgba::rgb(0xec, 0xee, 0xf1),
            Paper::Night => Rgba::rgb(0x1f, 0x21, 0x26),
        }
    }

    /// Ruling colour, already blended with the background (opaque).
    pub fn line(self) -> Rgba {
        let bg = self.background();
        match self {
            Paper::Night => Rgba::rgb(255, 255, 255).mix_over(bg, 0.13),
            Paper::Cream => Rgba::rgb(120, 100, 60).mix_over(bg, 0.28),
            _ => Rgba::rgb(60, 90, 150).mix_over(bg, 0.24),
        }
    }

    pub fn margin(self) -> Rgba {
        Rgba::rgb(225, 70, 70).mix_over(self.background(), 0.4)
    }

    pub fn is_dark(self) -> bool {
        self == Paper::Night
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
        }
    }

    /// Copy of the page with a fresh id (items keep theirs; they are immutable).
    pub fn duplicate(&self) -> Self {
        Page {
            id: new_id(),
            ..self.clone()
        }
    }
}

/// The content of one notebook. Metadata lives in [`crate::library::NotebookMeta`].
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: Id,
    pub pages: Vec<Page>,
}

impl Document {
    pub fn new(id: Id, template: Template, paper: Paper) -> Self {
        Document {
            id,
            pages: vec![Page::new(template, paper)],
        }
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
        out.truncate(out.floor_char_boundary(20_000));
        out
    }
}
