// Ink geometry: raw pencil points become a smooth centre line with a width at every
// sample. The width model gives each pen its feel (ported from the previous version).

import type { Paper, Rgba, Shape, Stroke } from "./model";
import { isDark, mix, paperBg } from "./model";
import { shapeOutline, type Pt } from "./geom";

export interface Sample { x: number; y: number; w: number }

const PRESSURE_GAMMA = 0.75;
const NIB_ANGLE = 0.7;
const smoothstep = (t: number) => { t = Math.max(0, Math.min(1, t)); return t * t * (3 - 2 * t); };

function widthAt(s: Stroke, pressure: number, dir: number, taper: number): number {
  const p = Math.pow(Math.max(0, Math.min(1, pressure)), PRESSURE_GAMMA);
  const w = s.width;
  let v: number;
  switch (s.pen) {
    case "Fineliner": v = w * (0.82 + 0.36 * p); break;
    case "Fountain": {
      const nib = 0.42 + 0.58 * Math.abs(Math.sin(dir - NIB_ANGLE));
      v = w * (0.35 + 1.15 * p) * nib * (0.35 + 0.65 * taper);
      break;
    }
    case "Brush": v = w * (0.15 + 1.75 * p) * (0.12 + 0.88 * taper); break;
    default: v = w;
  }
  return Math.max(0.25, v);
}

/** Evenly spaced samples along a Catmull-Rom curve through the stroke's points. */
export function samples(s: Stroke): Sample[] {
  const pts: [number, number, number][] = [];
  for (const p of s.points) {
    const l = pts[pts.length - 1];
    if (l && Math.hypot(p.x - l[0], p.y - l[1]) < 0.25) continue;
    pts.push([p.x, p.y, p.p]);
  }
  const orig = s.points[s.points.length - 1];
  if (pts.length && orig) { const last = pts[pts.length - 1]; last[2] = Math.max(last[2], Math.min(orig.p, last[2] + 0.2)); }
  if (!pts.length) return [];
  if (pts.length === 1) return [{ x: pts[0][0], y: pts[0][1], w: widthAt(s, pts[0][2], 0, 1) }];

  const n = pts.length;
  const curve: [number, number, number][] = [];
  for (let i = 0; i < n - 1; i++) {
    const p0 = pts[Math.max(0, i - 1)], p1 = pts[i], p2 = pts[i + 1], p3 = pts[Math.min(n - 1, i + 2)];
    const len = Math.hypot(p2[0] - p1[0], p2[1] - p1[1]);
    const steps = Math.max(1, Math.min(16, Math.ceil(len / 1.5)));
    for (let k = 0; k < steps; k++) {
      const t = k / steps, t2 = t * t, t3 = t2 * t;
      const cr = (a: number, b: number, c: number, d: number) =>
        0.5 * (2 * b + (-a + c) * t + (2 * a - 5 * b + 4 * c - d) * t2 + (-a + 3 * b - 3 * c + d) * t3);
      curve.push([cr(p0[0], p1[0], p2[0], p3[0]), cr(p0[1], p1[1], p2[1], p3[1]), p1[2] + (p2[2] - p1[2]) * t]);
    }
  }
  curve.push(pts[n - 1]);

  const m = curve.length;
  const raw = curve.map((c) => c[2]);
  const pressure = raw.map((_, i) => {
    const a = Math.max(0, i - 3), b = Math.min(m - 1, i + 3);
    let sum = 0;
    for (let j = a; j <= b; j++) sum += raw[j];
    return sum / (b - a + 1);
  });
  const pmin = Math.min(...raw), pmax = Math.max(...raw);
  const hasPressure = pmax - pmin > 0.04;
  const arc = new Array<number>(m).fill(0);
  for (let i = 1; i < m; i++) arc[i] = arc[i - 1] + Math.hypot(curve[i][0] - curve[i - 1][0], curve[i][1] - curve[i - 1][1]);
  const total = Math.max(0.001, arc[m - 1]);
  const tapering = s.pen === "Brush" || (s.pen === "Fountain" && !hasPressure);
  const out: Sample[] = curve.map((c, i) => {
    const a = curve[Math.max(0, i - 1)], b = curve[Math.min(m - 1, i + 1)];
    const dir = Math.atan2(b[1] - a[1], b[0] - a[0]);
    const taperLen = Math.max(0.001, Math.min(total * 0.22, s.width * 7));
    const taper = smoothstep(Math.min(arc[i], total - arc[i]) / taperLen);
    return { x: c[0], y: c[1], w: widthAt(s, pressure[i], dir, tapering ? taper : 1) };
  });
  if ((s.pen === "Fountain" || s.pen === "Brush") && out.length > 2) {
    const ws = out.map((q) => q.w);
    for (let i = 1; i < out.length - 1; i++) out[i].w = (ws[i - 1] + 2 * ws[i] + ws[i + 1]) / 4;
  }
  return out;
}

/** Highlighter ink is pre-blended with the paper so it can be painted opaque under writing. */
export function inkColor(s: Stroke, paper: Paper): Rgba {
  if (s.pen === "Highlighter") return mix(s.color, paperBg(paper), isDark(paper) ? 0.55 : 0.42);
  return s.color;
}

/** Ink colour on screen: dark ink turns light on dark paper. */
export function displayColor(c: Rgba, paper: Paper): Rgba {
  if (isDark(paper) && (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]) / 255 < 0.3)
    return [255 - (c[0] >> 1), 255 - (c[1] >> 1), 255 - (c[2] >> 1), c[3]];
  return c;
}

/** Adds a variable-width ribbon with round ends to a path (filled with nonzero). */
export function ribbonPath(path: Path2D, ss: Sample[], roundCaps = true) {
  const n = ss.length;
  if (n === 0) return;
  if (n === 1) {
    path.moveTo(ss[0].x + ss[0].w / 2, ss[0].y);
    path.arc(ss[0].x, ss[0].y, ss[0].w / 2, 0, Math.PI * 2);
    return;
  }
  const left: Pt[] = [], right: Pt[] = [];
  for (let i = 0; i < n; i++) {
    const a = ss[Math.max(0, i - 1)], b = ss[Math.min(n - 1, i + 1)];
    let tx = b.x - a.x, ty = b.y - a.y;
    const l = Math.hypot(tx, ty) || 1e-6;
    tx /= l; ty /= l;
    const h = ss[i].w / 2;
    left.push([ss[i].x - ty * h, ss[i].y + tx * h]);
    right.push([ss[i].x + ty * h, ss[i].y - tx * h]);
  }
  path.moveTo(left[0][0], left[0][1]);
  for (let i = 1; i < n; i++) path.lineTo(left[i][0], left[i][1]);
  for (let i = n - 1; i >= 0; i--) path.lineTo(right[i][0], right[i][1]);
  path.closePath();
  if (!roundCaps) return;
  for (const s of [ss[0], ss[n - 1]]) {
    path.moveTo(s.x + s.w / 2, s.y);
    path.arc(s.x, s.y, s.w / 2, 0, Math.PI * 2);
  }
  // Round joins where the line turns sharply.
  for (let i = 1; i < n - 1; i++) {
    const a = ss[i - 1], b = ss[i], c = ss[i + 1];
    const ux = b.x - a.x, uy = b.y - a.y, vx = c.x - b.x, vy = c.y - b.y;
    const cos = (ux * vx + uy * vy) / Math.max(1e-6, Math.hypot(ux, uy) * Math.hypot(vx, vy));
    if (cos < 0.8) {
      path.moveTo(b.x + b.w / 2, b.y);
      path.arc(b.x, b.y, b.w / 2, 0, Math.PI * 2);
    }
  }
}

export function dash(pts: Pt[], on: number, off: number): Pt[][] {
  const out: Pt[][] = [];
  let cur: Pt[] = [pts[0]];
  let drawing = true, left = on;
  for (let i = 1; i < pts.length; i++) {
    let a = pts[i - 1];
    const b = pts[i];
    let seg = Math.hypot(b[0] - a[0], b[1] - a[1]);
    while (seg > 0) {
      const step = Math.min(seg, left);
      const t = step / seg;
      const p: Pt = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
      if (drawing) cur.push(p);
      left -= step; seg -= step; a = p;
      if (left <= 1e-4) {
        if (drawing) { out.push(cur); cur = []; } else cur = [p];
        drawing = !drawing;
        left = drawing ? on : off;
      }
    }
  }
  if (drawing && cur.length > 1) out.push(cur);
  return out;
}

/** Polylines of a shape including arrow heads and dashes. */
export function shapePolylines(sh: Shape): Pt[][] {
  const [pts, closed] = shapeOutline(sh);
  if (!pts.length) return [];
  if (closed) pts.push(pts[0]);
  const lines = sh.dashed && sh.dotted ? dash(pts, 0.01, sh.width * 2.2) : sh.dashed ? dash(pts, sh.width * 3.5, sh.width * 2.5) : [pts];
  if (sh.kind === "Arrow" && sh.points.length >= 2) {
    const [a, b] = sh.points;
    const ang = Math.atan2(b[1] - a[1], b[0] - a[0]);
    const l = Math.max(10, sh.width * 4);
    const wing = (da: number): Pt => [b[0] - l * Math.cos(ang + da), b[1] - l * Math.sin(ang + da)];
    lines.push([wing(-0.45), b, wing(0.45)]);
  }
  return lines;
}

export const highlightTint = (c: Rgba): Rgba => mix(c, [255, 255, 255, 255], 0.55);
