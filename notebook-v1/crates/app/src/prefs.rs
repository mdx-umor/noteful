//! Tool settings and app preferences, saved as JSON next to the library.

use notebook_core::library::SortBy;
use notebook_core::storage::Store;
use notebook_core::{Paper, PaperSize, PenKind, Rgba, ShapeKind, Sticker, Template};
use serde::{Deserialize, Serialize};

pub const PEN_SIZES_MM: [f32; 5] = [0.35, 0.8, 1.2, 1.9, 3.8];
pub const HL_SIZES_MM: [f32; 5] = [2.0, 3.5, 5.0, 7.0, 10.0];
pub const TAPE_SIZES_MM: [f32; 5] = [3.0, 5.0, 7.0, 10.0, 14.0];
/// Pastel tape patterns: peach, pink, lilac, blue.
pub const TAPE_COLORS: [&str; 4] = ["#f6b9a4", "#f4b6cf", "#cdb9ef", "#b2c6f2"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TapeKind {
    Line,
    Rect,
    Free,
}

/// Tools that can be hidden from the toolbar (Customize Toolbar).
pub const TOOL_NAMES: [&str; 11] = [
    "Pen",
    "Highlighter",
    "Tape",
    "Eraser",
    "Lasso",
    "Hand",
    "Laser",
    "Shapes",
    "Text",
    "Stickers",
    "Insert",
];

pub fn default_swatches() -> Vec<Rgba> {
    [
        "#1d1d1f", "#1e5bd8", "#d93025", "#188038", "#7b3fe4", "#f28b25",
    ]
    .iter()
    .filter_map(|h| Rgba::from_hex(h))
    .collect()
}

pub fn default_hl_swatches() -> Vec<Rgba> {
    ["#ffd60a", "#7ee787", "#8ad4ff", "#ff9bd2", "#ffb38a"]
        .iter()
        .filter_map(|h| Rgba::from_hex(h))
        .collect()
}

/// A complete palette offered in the colour picker.
pub const PALETTE: [&str; 24] = [
    "#1d1d1f", "#5f6368", "#9aa0a6", "#ffffff", "#7b1fa2", "#7b3fe4", "#3949ab", "#1e5bd8",
    "#039be5", "#12b5cb", "#00897b", "#188038", "#7cb342", "#c0ca33", "#f6c026", "#ffd60a",
    "#f28b25", "#e8710a", "#d93025", "#c2185b", "#ff6f91", "#a1887f", "#795548", "#3e2723",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub pen_kind: PenKind,
    pub pen_mm: f32,
    pub pen_color: Rgba,
    pub swatches: Vec<Rgba>,
    pub hl_mm: f32,
    pub hl_color: Rgba,
    pub hl_swatches: Vec<Rgba>,
    pub hl_straight: bool,
    /// Eraser radius in screen points.
    pub eraser_size: f32,
    pub eraser_precise: bool,
    pub eraser_hl_only: bool,
    pub shape_kind: ShapeKind,
    pub shape_mm: f32,
    pub shape_color: Rgba,
    pub shape_fill: bool,
    pub shape_dashed: bool,
    pub text_size: f32,
    pub text_color: Rgba,
    pub laser_color: Rgba,
    /// Fingers scroll and zoom; only the Pencil writes. Switched on when a Pencil is first seen.
    pub pencil_only: bool,
    pub pencil_seen: bool,
    /// Hold the pen still at the end of a stroke to turn it into a clean shape.
    pub shape_hold: bool,
    /// 0 = raw input, 1 = strong smoothing.
    pub stabilization: f32,
    /// Multiplier from Pencil force to ink pressure.
    pub pressure_gain: f32,
    pub dark_ui: bool,
    pub sort: SortBy,
    pub list_view: bool,
    pub new_template: Template,
    pub new_paper: Paper,
    pub auto_backup: bool,
    pub two_finger_undo: bool,
    pub tape_kind: TapeKind,
    pub tape_mm: f32,
    pub tape_color: Rgba,
    pub eraser_shapes: bool,
    pub eraser_tapes: bool,
    /// After erasing, switch back to the tool used before.
    pub eraser_return: bool,
    pub lasso_rect: bool,
    pub lasso_ink: bool,
    pub lasso_hl: bool,
    pub lasso_text: bool,
    pub lasso_shapes: bool,
    pub lasso_tapes: bool,
    pub lasso_images: bool,
    /// Dark pages with light ink, whatever the paper.
    pub night_mode: bool,
    pub show_timer: bool,
    pub keep_awake: bool,
    pub hidden_tools: Vec<String>,
    pub stickers: Vec<Sticker>,
    pub new_size: PaperSize,
    pub new_landscape: bool,
    /// Insert-shape colour.
    pub fill_color: Rgba,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            pen_kind: PenKind::Fountain,
            pen_mm: 0.8,
            pen_color: Rgba::BLACK,
            swatches: default_swatches(),
            hl_mm: 5.0,
            hl_color: Rgba::rgb(0xff, 0xd6, 0x0a),
            hl_swatches: default_hl_swatches(),
            hl_straight: false,
            eraser_size: 14.0,
            eraser_precise: false,
            eraser_hl_only: false,
            shape_kind: ShapeKind::Rect,
            shape_mm: 0.8,
            shape_color: Rgba::BLACK,
            shape_fill: false,
            shape_dashed: false,
            text_size: 16.0,
            text_color: Rgba::BLACK,
            laser_color: Rgba::rgb(255, 59, 48),
            pencil_only: false,
            pencil_seen: false,
            shape_hold: true,
            stabilization: 0.35,
            pressure_gain: 1.0,
            dark_ui: true,
            sort: SortBy::Modified,
            list_view: false,
            new_template: Template::Lined,
            new_paper: Paper::Cream,
            auto_backup: true,
            two_finger_undo: true,
            tape_kind: TapeKind::Free,
            tape_mm: 7.0,
            tape_color: Rgba::from_hex(TAPE_COLORS[0]).unwrap_or(Rgba::WHITE),
            eraser_shapes: true,
            eraser_tapes: true,
            eraser_return: false,
            lasso_rect: false,
            lasso_ink: true,
            lasso_hl: true,
            lasso_text: true,
            lasso_shapes: true,
            lasso_tapes: true,
            lasso_images: true,
            night_mode: false,
            show_timer: false,
            keep_awake: false,
            hidden_tools: Vec::new(),
            stickers: Vec::new(),
            new_size: PaperSize::A4,
            new_landscape: false,
            fill_color: Rgba::rgb(0xff, 0xd6, 0x0a),
        }
    }
}

impl Prefs {
    pub fn tool_visible(&self, name: &str) -> bool {
        !self.hidden_tools.iter().any(|t| t == name)
    }

    pub fn load(store: &Store) -> Prefs {
        store
            .read_file("prefs.json")
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, store: &Store) {
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let _ = store.write_file("prefs.json", &bytes);
        }
    }
}
