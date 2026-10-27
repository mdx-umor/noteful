// Tool settings and app preferences (prefs.json; same names as the previous version).

import type { PaperSize, Paper, PenKind, Rgba, ShapeKind, SortBy, Sticker, Template } from "./core/model";
import { fromHex } from "./core/model";
import { readText, writeText } from "./core/storage";

export const PEN_SIZES_MM = [0.35, 0.8, 1.2, 1.9, 3.8];
export const HL_SIZES_MM = [2, 3.5, 5, 7, 10];
export const TAPE_SIZES_MM = [3, 5, 7, 10, 14];
export const TAPE_COLORS = ["#f6b9a4", "#f4b6cf", "#cdb9ef", "#b2c6f2"];
export type TapeKind = "Line" | "Rect" | "Free";
export const PALETTE = [
  "#1d1d1f", "#5f6368", "#9aa0a6", "#ffffff", "#7b1fa2", "#7b3fe4", "#3949ab", "#1e5bd8",
  "#039be5", "#12b5cb", "#00897b", "#188038", "#7cb342", "#c0ca33", "#f6c026", "#ffd60a",
  "#f28b25", "#e8710a", "#d93025", "#c2185b", "#ff6f91", "#a1887f", "#795548", "#3e2723",
];
export const TOOL_NAMES = ["Pen", "Highlighter", "Tape", "Eraser", "Lasso", "Hand", "Laser", "Shapes", "Text", "Audio", "Stickers", "Insert"] as const;

export interface Prefs {
  pen_kind: PenKind;
  pen_mm: number;
  pen_color: Rgba;
  swatches: Rgba[];
  hl_mm: number;
  hl_color: Rgba;
  hl_swatches: Rgba[];
  hl_straight: boolean;
  eraser_size: number;
  eraser_precise: boolean;
  eraser_hl_only: boolean;
  eraser_shapes: boolean;
  eraser_tapes: boolean;
  eraser_return: boolean;
  shape_kind: ShapeKind;
  shape_mm: number;
  shape_color: Rgba;
  shape_fill: boolean;
  shape_dashed: boolean;
  fill_color: Rgba;
  text_size: number;
  text_color: Rgba;
  laser_color: Rgba;
  tape_kind: TapeKind;
  tape_mm: number;
  tape_color: Rgba;
  lasso_rect: boolean;
  lasso_ink: boolean;
  lasso_hl: boolean;
  lasso_text: boolean;
  lasso_shapes: boolean;
  lasso_tapes: boolean;
  lasso_images: boolean;
  /** Fingers scroll and zoom; only the Pencil writes. */
  pencil_only: boolean;
  pencil_seen: boolean;
  shape_hold: boolean;
  stabilization: number;
  pressure_gain: number;
  dark_ui: boolean;
  night_mode: boolean;
  show_timer: boolean;
  keep_awake: boolean;
  two_finger_undo: boolean;
  hidden_tools: string[];
  stickers: Sticker[];
  sort: SortBy;
  list_view: boolean;
  new_template: Template;
  new_paper: Paper;
  new_size: PaperSize;
  new_landscape: boolean;
  last_backup: number;
  /** Recognise handwriting in the background so it can be searched. */
  ocr_index: boolean;
}

const hexes = (h: string[]) => h.map(fromHex);

export const DEFAULT_PREFS: Prefs = {
  pen_kind: "Fountain",
  pen_mm: 0.8,
  pen_color: [0x1d, 0x1d, 0x1f, 255],
  swatches: hexes(["#1d1d1f", "#1e5bd8", "#d93025", "#188038", "#7b3fe4", "#f28b25"]),
  hl_mm: 5,
  hl_color: fromHex("#ffd60a"),
  hl_swatches: hexes(["#ffd60a", "#7ee787", "#8ad4ff", "#ff9bd2", "#ffb38a"]),
  hl_straight: false,
  eraser_size: 14,
  eraser_precise: false,
  eraser_hl_only: false,
  eraser_shapes: true,
  eraser_tapes: true,
  eraser_return: false,
  shape_kind: "Rect",
  shape_mm: 0.8,
  shape_color: [0x1d, 0x1d, 0x1f, 255],
  shape_fill: false,
  shape_dashed: false,
  fill_color: fromHex("#ffd60a"),
  text_size: 16,
  text_color: [0x1d, 0x1d, 0x1f, 255],
  laser_color: [255, 59, 48, 255],
  tape_kind: "Free",
  tape_mm: 7,
  tape_color: fromHex(TAPE_COLORS[0]),
  lasso_rect: false,
  lasso_ink: true,
  lasso_hl: true,
  lasso_text: true,
  lasso_shapes: true,
  lasso_tapes: true,
  lasso_images: true,
  pencil_only: false,
  pencil_seen: false,
  shape_hold: true,
  stabilization: 0.35,
  pressure_gain: 1,
  dark_ui: true,
  night_mode: false,
  show_timer: false,
  keep_awake: false,
  two_finger_undo: true,
  hidden_tools: [],
  stickers: [],
  sort: "Modified",
  list_view: false,
  new_template: "Lined",
  new_paper: "Cream",
  new_size: "A4",
  new_landscape: false,
  last_backup: 0,
  ocr_index: true,
};

export function loadPrefs(): Prefs {
  try {
    const t = readText("prefs.json");
    if (t) return { ...DEFAULT_PREFS, ...JSON.parse(t) };
  } catch {
    /* fall back to defaults */
  }
  return { ...DEFAULT_PREFS };
}

let lastSaved = "";
export function savePrefs(p: Prefs) {
  const s = JSON.stringify(p);
  if (s !== lastSaved) {
    lastSaved = s;
    writeText("prefs.json", s);
  }
}
