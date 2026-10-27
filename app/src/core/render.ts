// Draws pages with Canvas 2D: paper, ruling, pictures, layers, ink, text and tape.
// Used for the live pages, thumbnails and PNG export.

import type { Item, Page, Paper, Rgba, Stroke, Template, TextBox, Tape } from "./model";
import { css, isDark, kindOf, mm, paperBg, paperLine, paperMargin } from "./model";
import { displayColor, highlightTint, inkColor, ribbonPath, samples, shapePolylines } from "./ink";
import { shapeOutline, tapeOutline, type Pt } from "./geom";
import { FONT_FAMILY, LINE_HEIGHT, wrap } from "./text";

export type Prim =
  | { t: "line"; a: Pt; b: Pt; w: number; margin?: boolean }
  | { t: "dot"; c: Pt; r: number }
  | { t: "rect"; x: number; y: number; w: number; h: number; lw: number };

export function templatePrims(t: Template, w: number, h: number, weight = 1): Prim[] {
  const lw = 0.6 * weight;
  const out: Prim[] = [];
  const hline = (y: number, x1: number, x2: number, ww = lw) => out.push({ t: "line", a: [x1, y], b: [x2, y], w: ww });
  const vline = (x: number, y1: number, y2: number, ww = lw) => out.push({ t: "line", a: [x, y1], b: [x, y2], w: ww });
  switch (t) {
    case "Blank": break;
    case "Lined": case "Narrow": {
      const gap = t === "Lined" ? mm(8.7) : mm(7.1);
      for (let y = 72; y < h - 24; y += gap) hline(y, 0, w);
      out.push({ t: "line", a: [57, 0], b: [57, h], w: lw, margin: true });
      break;
    }
    case "Ruled": case "MarginRight": {
      for (let y = 72; y < h - 24; y += mm(8.7)) hline(y, 0, w);
      if (t === "MarginRight") out.push({ t: "line", a: [w - 57, 0], b: [w - 57, h], w: lw, margin: true });
      break;
    }
    case "Columns2": case "Columns3": {
      const n = t === "Columns2" ? 2 : 3, mx = 30, gx = 22;
      const cw = (w - mx * 2 - gx * (n - 1)) / n;
      for (let y = 72; y < h - 24; y += mm(8)) for (let c = 0; c < n; c++) { const x = mx + c * (cw + gx); hline(y, x, x + cw); }
      break;
    }
    case "Grid": {
      const g = mm(5);
      for (let y = g; y < h; y += g) hline(y, 0, w, lw * 0.8);
      for (let x = g; x < w; x += g) vline(x, 0, h, lw * 0.8);
      break;
    }
    case "Dotted": {
      const g = mm(5);
      for (let y = g; y < h; y += g) for (let x = g; x < w; x += g) out.push({ t: "dot", c: [x, y], r: 0.95 * weight });
      break;
    }
    case "Cornell": {
      for (let y = 100; y < h - 170; y += mm(8)) hline(y, 150, w);
      hline(72, 0, w, lw * 2);
      vline(150, 72, h - 165, lw * 2);
      hline(h - 165, 0, w, lw * 2);
      break;
    }
    case "Todo": {
      for (let y = 84; y < h - 30; y += 30) {
        hline(y, 44, w - 38);
        out.push({ t: "rect", x: 48, y: y - 20, w: 13, h: 13, lw: lw * 1.4 });
      }
      break;
    }
    case "Music": {
      for (let top = 68; top + 30 < h - 30; top += 66) for (let i = 0; i < 5; i++) hline(top + i * 6.5, 38, w - 38);
      break;
    }
    case "Storyboard": {
      const mx = 38, gx = 22, top = 54, cw = (w - mx * 2 - gx) / 2, ch = 158, gy = 90;
      for (let r = 0; r < 3; r++) for (let c = 0; c < 2; c++) {
        const x = mx + c * (cw + gx), y = top + r * (ch + gy);
        out.push({ t: "rect", x, y, w: cw, h: ch, lw: lw * 1.4 });
        hline(y + ch + 26, x, x + cw);
        hline(y + ch + 52, x, x + cw);
      }
      break;
    }
  }
  return out;
}

export function drawPaper(ctx: CanvasRenderingContext2D, template: Template, paper: Paper, w: number, h: number, weight = 1) {
  ctx.fillStyle = css(paperBg(paper));
  ctx.fillRect(0, 0, w, h);
  const line = css(paperLine(paper)), margin = css(paperMargin(paper));
  ctx.lineCap = "round";
  for (const p of templatePrims(template, w, h, weight)) {
    if (p.t === "line") {
      ctx.strokeStyle = p.margin ? margin : line;
      ctx.lineWidth = p.w;
      ctx.beginPath();
      ctx.moveTo(p.a[0], p.a[1]);
      ctx.lineTo(p.b[0], p.b[1]);
      ctx.stroke();
    } else if (p.t === "dot") {
      ctx.fillStyle = line;
      ctx.beginPath();
      ctx.arc(p.c[0], p.c[1], p.r, 0, Math.PI * 2);
      ctx.fill();
    } else {
      ctx.strokeStyle = line;
      ctx.lineWidth = p.lw;
      ctx.strokeRect(p.x, p.y, p.w, p.h);
    }
  }
}

export function drawText(ctx: CanvasRenderingContext2D, t: TextBox, paper: Paper) {
  const lh = t.size * LINE_HEIGHT;
  ctx.fillStyle = css(displayColor(t.color, paper));
  ctx.font = `${t.size}px ${FONT_FAMILY}`;
  ctx.textBaseline = "middle";
  wrap(t.text, t.size, t.w).forEach((line, i) => {
    if (line) ctx.fillText(line, t.x, t.y + i * lh + lh / 2);
  });
}

function polylinePath(pts: Pt[]): Path2D {
  const p = new Path2D();
  if (!pts.length) return p;
  p.moveTo(pts[0][0], pts[0][1]);
  if (pts.length === 1) p.lineTo(pts[0][0] + 0.01, pts[0][1]);
  for (const q of pts.slice(1)) p.lineTo(q[0], q[1]);
  return p;
}

function drawTape(ctx: CanvasRenderingContext2D, t: Tape) {
  const [pts, closed] = tapeOutline(t);
  if (t.revealed) {
    const c = t.color;
    ctx.strokeStyle = `rgba(${(c[0] * 3) >> 2},${(c[1] * 3) >> 2},${(c[2] * 3) >> 2},0.85)`;
    ctx.lineWidth = 0.8;
    ctx.setLineDash([4, 3]);
    if (closed) ctx.strokeRect(pts[0][0], pts[0][1], pts[2][0] - pts[0][0], pts[2][1] - pts[0][1]);
    else {
      // Outline both sides of the strip.
      const n = pts.length, h = t.width / 2;
      for (const side of [-1, 1]) {
        const off = pts.map((p, i): Pt => {
          const a = pts[Math.max(0, i - 1)], b = pts[Math.min(n - 1, i + 1)];
          const l = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1e-6;
          return [p[0] - ((b[1] - a[1]) / l) * h * side, p[1] + ((b[0] - a[0]) / l) * h * side];
        });
        ctx.stroke(polylinePath(off));
      }
    }
    ctx.setLineDash([]);
    return;
  }
  ctx.save();
  ctx.strokeStyle = "rgba(255,255,255,0.4)";
  if (closed) {
    // Rectangle tape: fill, then diagonal stripes clipped to it.
    const x = pts[0][0], y = pts[0][1], w = pts[2][0] - x, h = pts[2][1] - y;
    ctx.fillStyle = css(t.color);
    ctx.fillRect(x, y, w, h);
    ctx.beginPath();
    ctx.rect(x, y, w, h);
    ctx.clip();
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    for (let sx = x - h; sx < x + w; sx += 6) {
      ctx.moveTo(sx, y + h);
      ctx.lineTo(sx + h, y);
    }
    ctx.stroke();
  } else {
    ctx.strokeStyle = css(t.color);
    ctx.lineWidth = t.width;
    ctx.lineCap = "butt";
    ctx.lineJoin = "round";
    ctx.stroke(polylinePath(pts));
    // Slanted stripes across the strip, like washi tape.
    ctx.strokeStyle = "rgba(255,255,255,0.4)";
    ctx.lineWidth = 1.2;
    ctx.lineCap = "round";
    ctx.beginPath();
    const hw = (t.width / 2) * 0.85;
    let acc = 0;
    for (let i = 1; i < pts.length; i++) {
      const a = pts[i - 1], b = pts[i];
      const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
      if (len < 1e-3) continue;
      const dx = (b[0] - a[0]) / len, dy = (b[1] - a[1]) / len;
      for (let d = (6 - (acc % 6)) % 6; d < len; d += 6) {
        const cx = a[0] + dx * d, cy = a[1] + dy * d;
        ctx.moveTo(cx - dy * hw - dx * hw * 0.5, cy + dx * hw - dy * hw * 0.5);
        ctx.lineTo(cx + dy * hw + dx * hw * 0.5, cy - dx * hw + dy * hw * 0.5);
      }
      acc += len;
    }
    ctx.stroke();
  }
  ctx.restore();
}

export interface DrawOptions {
  paper: Paper;
  /** Picture lookup (decoded); undefined while loading. */
  image?: (asset: number) => CanvasImageSource | undefined;
  exclude?: Set<number>;
}

// Tessellated strokes, cached per (immutable) stroke object.
const pathCache = new WeakMap<object, Path2D>();
function strokePath(s: Stroke, caps: boolean): Path2D {
  const key = caps ? s : s.points;
  let p = pathCache.get(key);
  if (!p) {
    p = new Path2D();
    ribbonPath(p, samples(s), caps);
    pathCache.set(key, p);
  }
  return p;
}

export function drawItem(ctx: CanvasRenderingContext2D, it: Item, o: DrawOptions) {
  const c = it.content;
  const paper = o.paper;
  if ("Stroke" in c) {
    const s = c.Stroke;
    if (s.pen === "Highlighter" && !isDark(paper)) {
      // Real multiply blending: writing and pictures show through, like a marker.
      const path = strokePath(s, false);
      ctx.save();
      ctx.globalCompositeOperation = "multiply";
      ctx.fillStyle = css(highlightTint(s.color));
      ctx.fill(path);
      ctx.restore();
      return;
    }
    const path = strokePath(s, true);
    ctx.fillStyle = css(s.pen === "Highlighter" ? inkColor(s, paper) : displayColor(s.color, paper));
    ctx.fill(path);
  } else if ("Shape" in c) {
    const sh = c.Shape;
    const [pts, closed] = shapeOutline(sh);
    if (sh.fill && closed && pts.length > 2) {
      ctx.fillStyle = css(sh.fill, sh.solid ? 1 : 0.25);
      ctx.fill(polylinePath(pts));
    }
    ctx.strokeStyle = css(displayColor(sh.color, paper));
    ctx.lineWidth = sh.width;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    for (const line of shapePolylines(sh)) ctx.stroke(polylinePath(line));
  } else if ("Text" in c) {
    drawText(ctx, c.Text, paper);
  } else if ("Tape" in c) {
    drawTape(ctx, c.Tape);
  } else {
    const im = c.Image;
    const src = o.image?.(im.asset);
    if (src) ctx.drawImage(src, im.x, im.y, im.w, im.h);
    else {
      ctx.fillStyle = "#c8ccd4";
      ctx.fillRect(im.x, im.y, im.w, im.h);
    }
  }
}

const ORDER = { image: 0, highlight: 1, ink: 2, shape: 2, text: 2, tape: 3 } as const;

/** Draws a whole page in page units (caller sets the transform). */
export function drawPage(ctx: CanvasRenderingContext2D, page: Page, o: DrawOptions, paperWeight = 1) {
  drawPaper(ctx, page.template, o.paper, page.w, page.h, paperWeight);
  if (page.background) {
    const src = o.image?.(page.background);
    if (src) ctx.drawImage(src, 0, 0, page.w, page.h);
  }
  drawItems(ctx, page, o);
}

export function drawItems(ctx: CanvasRenderingContext2D, page: Page, o: DrawOptions) {
  const first = page.layers[0]?.id ?? 0;
  const known = new Set(page.layers.map((l) => l.id));
  for (const layer of page.layers) {
    if (!layer.visible) continue;
    const items = page.items.filter((it) => {
      const lid = known.has(it.layer ?? 0) ? it.layer ?? 0 : first;
      return lid === layer.id && !o.exclude?.has(it.id);
    });
    for (let pass = 0; pass < 4; pass++)
      for (const it of items) if (ORDER[kindOf(it)] === pass) drawItem(ctx, it, o);
  }
}

/** Small preview of a blank template (bolder lines so they show when tiny). */
export function templatePreview(t: Template, paper: Paper, widthPx: number): HTMLCanvasElement {
  const c = document.createElement("canvas");
  const s = widthPx / 595;
  c.width = widthPx;
  c.height = Math.round(842 * s);
  const ctx = c.getContext("2d")!;
  ctx.scale(s, s);
  drawPaper(ctx, t, paper, 595, 842, Math.min(4, Math.max(1.5, 1 / s)));
  return c;
}

export const rgbaCss = (c: Rgba) => css(c);
