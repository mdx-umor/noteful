// Bounding boxes, hit testing, lasso tests and transforms.

import type { Item, Shape, Tape } from "./model";
import { textHeight } from "./text";

export type Pt = [number, number];
export interface Rect { x: number; y: number; w: number; h: number }

export function rectFrom(pts: Iterable<Pt>): Rect {
  let x1 = Infinity, y1 = Infinity, x2 = -Infinity, y2 = -Infinity;
  for (const [x, y] of pts) {
    if (x < x1) x1 = x;
    if (y < y1) y1 = y;
    if (x > x2) x2 = x;
    if (y > y2) y2 = y;
  }
  if (x1 > x2) return { x: 0, y: 0, w: 0, h: 0 };
  return { x: x1, y: y1, w: x2 - x1, h: y2 - y1 };
}
export const expand = (r: Rect, d: number): Rect => ({ x: r.x - d, y: r.y - d, w: r.w + 2 * d, h: r.h + 2 * d });
export const union = (a: Rect, b: Rect): Rect => {
  const x = Math.min(a.x, b.x), y = Math.min(a.y, b.y);
  return { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y };
};
export const contains = (r: Rect, x: number, y: number) => x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h;
export const intersects = (a: Rect, b: Rect) => a.x <= b.x + b.w && b.x <= a.x + a.w && a.y <= b.y + b.h && b.y <= a.y + a.h;
export const center = (r: Rect): Pt => [r.x + r.w / 2, r.y + r.h / 2];
export const dist = (a: Pt, b: Pt) => Math.hypot(a[0] - b[0], a[1] - b[1]);

export function segDist(p: Pt, a: Pt, b: Pt): number {
  const dx = b[0] - a[0], dy = b[1] - a[1];
  const len2 = dx * dx + dy * dy;
  const t = len2 > 0 ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2)) : 0;
  return Math.hypot(p[0] - (a[0] + t * dx), p[1] - (a[1] + t * dy));
}

/** Outline of a shape and whether it is closed. */
export function shapeOutline(s: Shape): [Pt[], boolean] {
  if (s.points.length < 2) return [s.points.slice(), false];
  const [a, b] = s.points;
  switch (s.kind) {
    case "Line":
    case "Arrow":
      return [[a, b], false];
    case "Rect": {
      const x1 = Math.min(a[0], b[0]), x2 = Math.max(a[0], b[0]);
      const y1 = Math.min(a[1], b[1]), y2 = Math.max(a[1], b[1]);
      return [[[x1, y1], [x2, y1], [x2, y2], [x1, y2]], true];
    }
    case "Ellipse": {
      const cx = (a[0] + b[0]) / 2, cy = (a[1] + b[1]) / 2;
      const rx = Math.abs(b[0] - a[0]) / 2, ry = Math.abs(b[1] - a[1]) / 2;
      const pts: Pt[] = [];
      for (let i = 0; i < 72; i++) {
        const t = (i / 72) * Math.PI * 2;
        pts.push([cx + rx * Math.cos(t), cy + ry * Math.sin(t)]);
      }
      return [pts, true];
    }
    case "Polygon":
      return [s.points.slice(), true];
  }
}

export function tapeOutline(t: Tape): [Pt[], boolean] {
  if (t.rect && t.points.length >= 2) {
    const [a, b] = t.points;
    const x1 = Math.min(a[0], b[0]), x2 = Math.max(a[0], b[0]);
    const y1 = Math.min(a[1], b[1]), y2 = Math.max(a[1], b[1]);
    return [[[x1, y1], [x2, y1], [x2, y2], [x1, y2]], true];
  }
  return [t.points.slice(), false];
}

export function itemBBox(it: Item): Rect {
  const c = it.content;
  if ("Stroke" in c) return expand(rectFrom(c.Stroke.points.map((p) => [p.x, p.y] as Pt)), c.Stroke.width * 1.2);
  if ("Shape" in c) {
    const s = c.Shape;
    let r = expand(rectFrom(shapeOutline(s)[0]), s.width / 2);
    if (s.kind === "Arrow") r = expand(r, s.width * 4 + 6);
    return r;
  }
  if ("Text" in c) { const t = c.Text; return { x: t.x, y: t.y, w: t.w, h: textHeight(t.text, t.size, t.w) }; }
  if ("Tape" in c) { const r = rectFrom(tapeOutline(c.Tape)[0]); return c.Tape.rect ? r : expand(r, c.Tape.width / 2); }
  const im = c.Image;
  return { x: im.x, y: im.y, w: im.w, h: im.h };
}

export function unionBBox(items: Item[]): Rect | null {
  let acc: Rect | null = null;
  for (const it of items) acc = acc ? union(acc, itemBBox(it)) : itemBBox(it);
  return acc;
}

function polyHit(pts: Pt[], closed: boolean, p: Pt, r: number): boolean {
  if (pts.length === 0) return false;
  if (pts.length === 1) return dist(pts[0], p) <= r;
  const n = pts.length, segs = closed ? n : n - 1;
  for (let i = 0; i < segs; i++) if (segDist(p, pts[i], pts[(i + 1) % n]) <= r) return true;
  return false;
}

export function pointInPoly(x: number, y: number, poly: Pt[]): boolean {
  let inside = false;
  const n = poly.length;
  if (n < 3) return false;
  for (let i = 0, j = n - 1; i < n; j = i++) {
    const [xi, yi] = poly[i], [xj, yj] = poly[j];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

/** Does a circle of radius r around (x, y) touch the item? */
export function hitItem(it: Item, x: number, y: number, r: number): boolean {
  const c = it.content;
  const p: Pt = [x, y];
  if ("Stroke" in c) return polyHit(c.Stroke.points.map((q) => [q.x, q.y] as Pt), false, p, r + c.Stroke.width / 2);
  if ("Shape" in c) {
    const [pts, closed] = shapeOutline(c.Shape);
    if (c.Shape.fill && closed && pointInPoly(x, y, pts)) return true;
    return polyHit(pts, closed, p, r + c.Shape.width / 2);
  }
  if ("Tape" in c && !c.Tape.rect) return polyHit(c.Tape.points, false, p, r + c.Tape.width / 2);
  return contains(expand(itemBBox(it), r), x, y);
}

/** Lassoed when at least half of its points (or its centre) are inside. */
export function itemInLasso(it: Item, poly: Pt[]): boolean {
  const c = it.content;
  let pts: Pt[];
  if ("Stroke" in c) pts = c.Stroke.points.map((q) => [q.x, q.y]);
  else if ("Shape" in c) pts = shapeOutline(c.Shape)[0];
  else if ("Tape" in c) pts = tapeOutline(c.Tape)[0];
  else { const m = center(itemBBox(it)); return pointInPoly(m[0], m[1], poly); }
  if (!pts.length) return false;
  const step = Math.max(1, Math.floor(pts.length / 48));
  let inside = 0, total = 0;
  for (let i = 0; i < pts.length; i += step) {
    total++;
    if (pointInPoly(pts[i][0], pts[i][1], poly)) inside++;
  }
  return inside * 2 >= total;
}

/** Maps every point through f; s scales widths and text size. */
export function transformItem(it: Item, f: (x: number, y: number) => Pt, s: number): Item {
  const c = it.content;
  if ("Stroke" in c)
    return { ...it, content: { Stroke: { ...c.Stroke, width: c.Stroke.width * s, widths: c.Stroke.widths?.map((w) => w * s), points: c.Stroke.points.map((p) => { const q = f(p.x, p.y); return { x: q[0], y: q[1], p: p.p }; }) } } };
  if ("Shape" in c)
    return { ...it, content: { Shape: { ...c.Shape, width: c.Shape.width * s, points: c.Shape.points.map((p) => f(p[0], p[1])) } } };
  if ("Text" in c) {
    const q = f(c.Text.x, c.Text.y);
    return { ...it, content: { Text: { ...c.Text, x: q[0], y: q[1], w: c.Text.w * s, size: c.Text.size * s } } };
  }
  if ("Tape" in c)
    return { ...it, content: { Tape: { ...c.Tape, width: c.Tape.rect ? c.Tape.width : c.Tape.width * s, points: c.Tape.points.map((p) => f(p[0], p[1])) } } };
  const im = c.Image;
  const a = f(im.x, im.y), b = f(im.x + im.w, im.y + im.h);
  return { ...it, content: { Image: { ...im, x: Math.min(a[0], b[0]), y: Math.min(a[1], b[1]), w: Math.abs(b[0] - a[0]), h: Math.abs(b[1] - a[1]) } } };
}
export const translateItem = (it: Item, dx: number, dy: number) => transformItem(it, (x, y) => [x + dx, y + dy], 1);
export const scaleItem = (it: Item, ax: number, ay: number, s: number) =>
  transformItem(it, (x, y) => [ax + (x - ax) * s, ay + (y - ay) * s], s);

/** Ramer–Douglas–Peucker simplification. */
export function rdp(pts: Pt[], eps: number): Pt[] {
  if (pts.length < 3) return pts.slice();
  const a = pts[0], b = pts[pts.length - 1];
  let idx = 0, maxD = 0;
  for (let i = 1; i < pts.length - 1; i++) {
    const d = segDist(pts[i], a, b);
    if (d > maxD) { maxD = d; idx = i; }
  }
  if (maxD > eps) {
    const left = rdp(pts.slice(0, idx + 1), eps);
    const right = rdp(pts.slice(idx), eps);
    left.pop();
    return left.concat(right);
  }
  return [a, b];
}

/** Hold-to-snap: a hand-drawn line, triangle, rectangle or ellipse becomes a clean shape. */
export function recognize(points: Pt[]): [Shape["kind"], Pt[]] | null {
  if (points.length < 4) return null;
  const bbox = rectFrom(points);
  const diag = Math.hypot(bbox.w, bbox.h);
  if (diag < 14) return null;
  let len = 0;
  for (let i = 1; i < points.length; i++) len += dist(points[i - 1], points[i]);
  const first = points[0], last = points[points.length - 1];
  const ends = dist(first, last);
  if (ends / len > 0.9) return ["Line", [first, last]];
  if (ends > diag * 0.3) return null;
  const verts = rdp(points, diag * 0.07);
  if (verts.length > 1 && dist(verts[0], verts[verts.length - 1]) < diag * 0.2) verts.pop();
  const merged: Pt[] = [];
  for (const v of verts) if (!merged.length || dist(merged[merged.length - 1], v) > diag * 0.12) merged.push(v);
  if (merged.length > 2 && dist(merged[0], merged[merged.length - 1]) < diag * 0.12) merged.pop();
  if (merged.length === 3) return ["Polygon", merged];
  if (merged.length === 4) {
    const aligned = merged.every((v, i) => {
      const w = merged[(i + 1) % 4];
      const ang = Math.abs((Math.atan2(w[1] - v[1], w[0] - v[0]) * 180) / Math.PI) % 90;
      return !(ang >= 14 && ang <= 76);
    });
    if (aligned) return ["Rect", [[bbox.x, bbox.y], [bbox.x + bbox.w, bbox.y + bbox.h]]];
    return ["Polygon", merged];
  }
  const cx = bbox.x + bbox.w / 2, cy = bbox.y + bbox.h / 2;
  const rx = Math.max(1, bbox.w / 2), ry = Math.max(1, bbox.h / 2);
  let err = 0;
  for (const p of points) err += Math.abs(Math.hypot((p[0] - cx) / rx, (p[1] - cy) / ry) - 1);
  if (err / points.length < 0.22) return ["Ellipse", [[bbox.x, bbox.y], [bbox.x + bbox.w, bbox.y + bbox.h]]];
  return null;
}
