// Document model. The JSON shape matches what the previous (Rust) version saved,
// so notebooks written by either version open in the other.

export type Id = number;
/** [r, g, b, a], 0..255 */
export type Rgba = [number, number, number, number];

export const A4_W = 595;
export const A4_H = 842;
export const mm = (v: number) => (v * 72) / 25.4;

let lastId = 0;
/** Milliseconds in the high bits, a counter in the low 16 bits; always < 2^53. */
export function newId(): Id {
  const candidate = (Date.now() % 2 ** 37) * 65536;
  lastId = Math.max(candidate, lastId + 1);
  return lastId;
}

export const now = () => Date.now();

export type PenKind = "Fineliner" | "Fountain" | "Brush" | "Highlighter";
export interface InkPoint { x: number; y: number; p: number }
export interface Stroke { pen: PenKind; color: Rgba; width: number; points: InkPoint[] }

export type ShapeKind = "Line" | "Arrow" | "Rect" | "Ellipse" | "Polygon";
export interface Shape {
  kind: ShapeKind;
  points: [number, number][];
  color: Rgba;
  width: number;
  fill?: Rgba | null;
  dashed?: boolean;
  solid?: boolean;
  dotted?: boolean;
}
export interface TextBox { x: number; y: number; w: number; text: string; size: number; color: Rgba }
export interface Tape { points: [number, number][]; width: number; color: Rgba; rect?: boolean; revealed?: boolean }
export interface ImageBox { asset: Id; x: number; y: number; w: number; h: number }

export type Content =
  | { Stroke: Stroke }
  | { Shape: Shape }
  | { Text: TextBox }
  | { Tape: Tape }
  | { Image: ImageBox };

export type Link = { Url: string } | { Page: Id };

export interface Item { id: Id; content: Content; layer?: Id; link?: Link | null }

export type ItemKind = "ink" | "highlight" | "shape" | "text" | "tape" | "image";
export function kindOf(it: Item): ItemKind {
  const c = it.content;
  if ("Stroke" in c) return c.Stroke.pen === "Highlighter" ? "highlight" : "ink";
  if ("Shape" in c) return "shape";
  if ("Text" in c) return "text";
  if ("Tape" in c) return "tape";
  return "image";
}

export function itemColor(it: Item): Rgba {
  const c = it.content;
  if ("Stroke" in c) return c.Stroke.color;
  if ("Shape" in c) return c.Shape.color;
  if ("Text" in c) return c.Text.color;
  if ("Tape" in c) return c.Tape.color;
  return [255, 255, 255, 255];
}

export function withColor(it: Item, col: Rgba): Item {
  const c = it.content;
  let content: Content = c;
  if ("Stroke" in c) content = { Stroke: { ...c.Stroke, color: col } };
  else if ("Shape" in c) content = { Shape: { ...c.Shape, color: col, fill: c.Shape.fill ? col : c.Shape.fill } };
  else if ("Text" in c) content = { Text: { ...c.Text, color: col } };
  else if ("Tape" in c) content = { Tape: { ...c.Tape, color: col } };
  return { ...it, content };
}

export function newItem(content: Content, layer: Id = 0): Item {
  return { id: newId(), content, layer, link: null };
}

export type Template =
  | "Blank" | "Lined" | "Narrow" | "Grid" | "Dotted" | "Cornell" | "Todo" | "Music"
  | "Storyboard" | "Ruled" | "MarginRight" | "Columns2" | "Columns3";

export const TEMPLATES: Template[] = [
  "Blank", "Ruled", "Lined", "MarginRight", "Narrow", "Grid", "Dotted", "Cornell",
  "Columns2", "Columns3", "Todo", "Music", "Storyboard",
];
export const TEMPLATE_NAMES: Record<Template, string> = {
  Blank: "Blank", Ruled: "Ruled", Lined: "Margin", MarginRight: "Margin right",
  Narrow: "Narrow ruled", Grid: "Grid", Dotted: "Dotted", Cornell: "Cornell",
  Columns2: "2 Columns", Columns3: "3 Columns", Todo: "To-do", Music: "Music",
  Storyboard: "Storyboard",
};

export type Paper =
  | "White" | "Cream" | "Gray" | "Night" | "Black" | "Rose" | "Pink" | "Lilac"
  | "Periwinkle" | "Sky" | "Mint" | "Sage";
export const PAPERS: Paper[] = [
  "White", "Cream", "Gray", "Night", "Black", "Rose", "Pink", "Lilac", "Periwinkle", "Sky", "Mint", "Sage",
];
const PAPER_BG: Record<Paper, Rgba> = {
  White: [255, 255, 255, 255], Cream: [0xfd, 0xf8, 0xe6, 255], Gray: [0xec, 0xee, 0xf1, 255],
  Night: [0x1f, 0x21, 0x26, 255], Black: [0x0b, 0x0b, 0x0c, 255], Rose: [0xfb, 0xe3, 0xe0, 255],
  Pink: [0xfb, 0xe1, 0xee, 255], Lilac: [0xec, 0xe3, 0xf8, 255], Periwinkle: [0xe1, 0xe6, 0xfa, 255],
  Sky: [0xdd, 0xef, 0xfb, 255], Mint: [0xdf, 0xf5, 0xea, 255], Sage: [0xe6, 0xee, 0xd9, 255],
};
export const paperBg = (p: Paper): Rgba => PAPER_BG[p] ?? PAPER_BG.White;
export const isDark = (p: Paper) => p === "Night" || p === "Black";

export function mix(a: Rgba, base: Rgba, t: number): Rgba {
  const m = (x: number, y: number) => Math.round(y + (x - y) * t);
  return [m(a[0], base[0]), m(a[1], base[1]), m(a[2], base[2]), 255];
}
export function paperLine(p: Paper): Rgba {
  const bg = paperBg(p);
  if (isDark(p)) return mix([255, 255, 255, 255], bg, 0.13);
  if (p === "Cream") return mix([120, 100, 60, 255], bg, 0.28);
  return mix([60, 90, 150, 255], bg, 0.24);
}
export const paperMargin = (p: Paper) => mix([225, 70, 70, 255], paperBg(p), 0.4);
export const luminance = (c: Rgba) => (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]) / 255;

export const css = (c: Rgba, alpha = 1) => `rgba(${c[0]},${c[1]},${c[2]},${((c[3] ?? 255) / 255) * alpha})`;
export function fromHex(h: string): Rgba {
  const v = parseInt(h.replace("#", ""), 16);
  return [(v >> 16) & 255, (v >> 8) & 255, v & 255, 255];
}
export const toHex = (c: Rgba) => "#" + [c[0], c[1], c[2]].map((v) => v.toString(16).padStart(2, "0")).join("");
export const sameColor = (a: Rgba, b: Rgba) => a[0] === b[0] && a[1] === b[1] && a[2] === b[2];

export interface Layer { id: Id; name: string; visible: boolean; locked: boolean }

export interface Page {
  id: Id;
  w: number;
  h: number;
  template: Template;
  paper: Paper;
  items: Item[];
  bookmark?: string | null;
  layers: Layer[];
  background?: Id | null;
}

export interface OutlineEntry { id: Id; title: string; page: Id; level: number }
export interface Doc { id: Id; pages: Page[]; outline: OutlineEntry[] }
export interface Sticker { id: Id; items: Item[]; used: number }

export const firstLayer = (): Layer => ({ id: 0, name: "Layer 1", visible: true, locked: false });

export function newPage(template: Template, paper: Paper, w = A4_W, h = A4_H): Page {
  return { id: newId(), w, h, template, paper, items: [], bookmark: null, layers: [firstLayer()], background: null };
}

export function newDoc(template: Template, paper: Paper, w = A4_W, h = A4_H): Doc {
  return { id: newId(), pages: [newPage(template, paper, w, h)], outline: [] };
}

/** Fills in fields older files may lack: layers, item layers, outline. */
export function normalizeDoc(d: Doc): Doc {
  const pages = (d.pages?.length ? d.pages : [newPage("Blank", "White")]).map((p) => {
    const layers = p.layers?.length
      ? p.layers.map((l) => ({ ...l, visible: l.visible !== false, locked: !!l.locked }))
      : [firstLayer()];
    const ids = new Set(layers.map((l) => l.id));
    const items = (p.items ?? []).map((it) => (ids.has(it.layer ?? 0) ? { ...it, layer: it.layer ?? 0 } : { ...it, layer: layers[0].id }));
    return { ...p, layers, items, bookmark: p.bookmark ?? null, background: p.background ?? null };
  });
  return { id: d.id, pages, outline: d.outline ?? [] };
}

export const layerOf = (p: Page, id: Id) => p.layers.find((l) => l.id === id);
export const editable = (p: Page, it: Item) => {
  const l = layerOf(p, it.layer ?? 0);
  return !l || (l.visible && !l.locked);
};
export const visible = (p: Page, it: Item) => {
  const l = layerOf(p, it.layer ?? 0);
  return !l || l.visible;
};
export const hasPictures = (p: Page) => !!p.background || p.items.some((i) => "Image" in i.content);

export function assetIds(d: Doc): Id[] {
  const s = new Set<Id>();
  for (const p of d.pages) {
    if (p.background) s.add(p.background);
    for (const it of p.items) if ("Image" in it.content) s.add(it.content.Image.asset);
  }
  return [...s];
}

export function searchText(d: Doc): string {
  const out: string[] = [];
  for (const p of d.pages) {
    if (p.bookmark) out.push(p.bookmark);
    for (const it of p.items) if ("Text" in it.content) out.push(it.content.Text.text);
  }
  for (const o of d.outline) out.push(o.title);
  return out.join(" ").slice(0, 20000);
}

export type PaperSize = "A4" | "Letter" | "A5" | "Legal" | "Square" | "Tablet";
export const PAPER_SIZES: PaperSize[] = ["A4", "Letter", "A5", "Legal", "Square", "Tablet"];
const SIZE_DIMS: Record<PaperSize, [number, number]> = {
  A4: [A4_W, A4_H], Letter: [612, 792], A5: [420, 595], Legal: [612, 1008], Square: [595, 595], Tablet: [595, 793],
};
export function sizeDims(s: PaperSize, landscape: boolean): [number, number] {
  const [w, h] = SIZE_DIMS[s] ?? SIZE_DIMS.A4;
  return landscape ? [h, w] : [w, h];
}
export const SIZE_NAMES: Record<PaperSize, string> = {
  A4: "A4", Letter: "Letter", A5: "A5", Legal: "Legal", Square: "Square", Tablet: "Tablet screen",
};

// ---------- library ----------

export interface NotebookMeta {
  id: Id;
  title: string;
  cover: Rgba;
  folder?: Id | null;
  tags: string[];
  pinned: boolean;
  trashed_at?: number | null;
  created: number;
  updated: number;
  opened: number;
  page_count: number;
  search_text: string;
  template: Template;
  paper: Paper;
}
export interface Folder { id: Id; name: string }
export interface Library { notebooks: NotebookMeta[]; folders: Folder[] }

export type Filter =
  | { k: "all" } | { k: "untagged" } | { k: "unfiled" } | { k: "trash" } | { k: "pinned" }
  | { k: "folder"; id: Id } | { k: "tag"; path: string };
export type SortBy = "Modified" | "Opened" | "Created" | "Name";

export function normalizeTag(s: string): string | null {
  const parts = s.trim().replace(/^#+/, "").split("/").map((p) => p.trim()).filter(Boolean);
  return parts.length ? parts.join("/") : null;
}
export const tagMatches = (tag: string, path: string) => tag === path || tag.startsWith(path + "/");

export function matches(m: NotebookMeta, f: Filter): boolean {
  const trashed = m.trashed_at != null;
  if (f.k === "trash") return trashed;
  if (trashed) return false;
  switch (f.k) {
    case "all": return true;
    case "untagged": return m.tags.length === 0;
    case "unfiled": return m.folder == null;
    case "pinned": return m.pinned;
    case "folder": return m.folder === f.id;
    case "tag": return m.tags.some((t) => tagMatches(t, f.path));
  }
}

export function query(lib: Library, f: Filter, search: string, sort: SortBy): NotebookMeta[] {
  const words = search.toLowerCase().split(/\s+/).filter(Boolean);
  const out = lib.notebooks.filter((m) => {
    const base = words.length ? m.trashed_at == null || f.k === "trash" : matches(m, f);
    return base && words.every((w) =>
      m.title.toLowerCase().includes(w) ||
      m.tags.some((t) => t.toLowerCase().includes(w.replace(/^#/, ""))) ||
      (m.search_text ?? "").toLowerCase().includes(w));
  });
  const key: Record<SortBy, (m: NotebookMeta) => number | string> = {
    Modified: (m) => -m.updated, Opened: (m) => -(m.opened ?? 0), Created: (m) => -m.created, Name: (m) => m.title.toLowerCase(),
  };
  out.sort((a, b) => { const x = key[sort](a), y = key[sort](b); return x < y ? -1 : x > y ? 1 : 0; });
  if (f.k !== "trash") out.sort((a, b) => Number(b.pinned) - Number(a.pinned));
  return out;
}

export interface TagNode { name: string; path: string; count: number; children: TagNode[] }
export function tagTree(lib: Library): TagNode[] {
  const paths = new Set<string>();
  for (const m of lib.notebooks) if (m.trashed_at == null) for (const t of m.tags) {
    const parts = t.split("/");
    for (let i = 1; i <= parts.length; i++) paths.add(parts.slice(0, i).join("/"));
  }
  const all = [...paths].sort();
  const build = (parent: string | null): TagNode[] =>
    all.filter((p) => parent === null ? !p.includes("/") : p.startsWith(parent + "/") && !p.slice(parent.length + 1).includes("/"))
      .map((p) => ({
        name: p.split("/").pop()!, path: p,
        count: lib.notebooks.filter((m) => matches(m, { k: "tag", path: p })).length,
        children: build(p),
      }));
  return build(null);
}
