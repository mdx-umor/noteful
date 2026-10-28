// Ready-made shapes (Insert Shape) and the sample sticker pack.

import type { Item, Rgba, Shape, Sticker } from "./model";
import { fromHex, newId, newItem } from "./model";
import type { Pt } from "./geom";

export type Preset =
  | "Square" | "Rounded" | "Pill" | "Circle" | "Diamond" | "Triangle"
  | "Hexagon" | "Cloud" | "Burst" | "Star" | "Heart" | "Arrow";
export const PRESETS: Preset[] = ["Square", "Rounded", "Pill", "Circle", "Diamond", "Triangle", "Hexagon", "Cloud", "Burst", "Star", "Heart", "Arrow"];

const PI = Math.PI, TAU = Math.PI * 2;

function roundedRect(x: number, y: number, w: number, h: number, r: number): Pt[] {
  r = Math.min(r, w / 2, h / 2);
  const pts: Pt[] = [];
  const corners: [Pt, number][] = [[[x + w - r, y + r], -PI / 2], [[x + w - r, y + h - r], 0], [[x + r, y + h - r], PI / 2], [[x + r, y + r], PI]];
  for (const [c, a0] of corners) for (let k = 0; k <= 8; k++) { const a = a0 + (k / 8) * (PI / 2); pts.push([c[0] + r * Math.cos(a), c[1] + r * Math.sin(a)]); }
  return pts;
}
const regular = (cx: number, cy: number, r: number, n: number, rot: number): Pt[] =>
  Array.from({ length: n }, (_, i) => [cx + r * Math.cos(rot + (i / n) * TAU), cy + r * Math.sin(rot + (i / n) * TAU)] as Pt);
const star = (cx: number, cy: number, ro: number, ri: number, n: number): Pt[] =>
  Array.from({ length: n * 2 }, (_, i) => { const a = -PI / 2 + (i / (n * 2)) * TAU, r = i % 2 ? ri : ro; return [cx + r * Math.cos(a), cy + r * Math.sin(a)] as Pt; });

export function presetShape(p: Preset, cx: number, cy: number, size: number): [Shape["kind"], Pt[]] {
  const h = size / 2;
  switch (p) {
    case "Square": return ["Rect", [[cx - h, cy - h], [cx + h, cy + h]]];
    case "Rounded": return ["Polygon", roundedRect(cx - h, cy - h, size, size, size * 0.18)];
    case "Pill": return ["Polygon", roundedRect(cx - h, cy - h * 0.55, size, size * 0.55, size)];
    case "Circle": return ["Ellipse", [[cx - h, cy - h], [cx + h, cy + h]]];
    case "Diamond": return ["Polygon", regular(cx, cy, h, 4, -PI / 2)];
    case "Triangle": return ["Polygon", [[cx, cy - h], [cx + h, cy + h * 0.8], [cx - h, cy + h * 0.8]]];
    case "Hexagon": return ["Polygon", regular(cx, cy, h, 6, -PI / 2)];
    case "Cloud": return ["Polygon", Array.from({ length: 144 }, (_, i) => { const a = (i / 144) * TAU, b = 1 + 0.09 * Math.abs(Math.sin(a * 9)); return [cx + h * b * Math.cos(a), cy + h * 0.62 * b * Math.sin(a)] as Pt; })];
    case "Burst": return ["Polygon", star(cx, cy, h, h * 0.78, 14)];
    case "Star": return ["Polygon", star(cx, cy, h, h * 0.42, 5)];
    case "Heart": return ["Polygon", Array.from({ length: 96 }, (_, i) => {
      const t = (i / 96) * TAU, x = 16 * Math.sin(t) ** 3;
      const y = 13 * Math.cos(t) - 5 * Math.cos(2 * t) - 2 * Math.cos(3 * t) - Math.cos(4 * t);
      return [cx + (x / 17) * h, cy - (y / 17) * h - h * 0.1] as Pt;
    })];
    case "Arrow": return ["Arrow", [[cx - h, cy + h * 0.5], [cx + h, cy - h * 0.5]]];
  }
}

export function presetItem(p: Preset, cx: number, cy: number, size: number, color: Rgba): Item {
  const [kind, points] = presetShape(p, cx, cy, size);
  const sh: Shape = { kind, points, color, width: p === "Arrow" ? 3 : 1.5, fill: null, dashed: false, solid: false, dotted: false };
  if (p !== "Arrow") { sh.fill = color; sh.solid = true; }
  return newItem({ Shape: sh });
}

const text = (x: number, y: number, w: number, t: string, size: number, color: Rgba): Item =>
  newItem({ Text: { x, y, w, text: t, size, color } });
const line = (pts: Pt[], color: Rgba, width: number): Item =>
  newItem({ Stroke: { pen: "Fineliner", color, width, points: pts.map(([x, y]) => ({ x, y, p: 0.6 })) } });
const WHITE: Rgba = [255, 255, 255, 255];

export function sampleStickers(): Sticker[] {
  const c = fromHex;
  const packs: Item[][] = [
    [presetItem("Circle", 30, 30, 56, c("#7b3fe4")), text(19, 8, 30, "?", 34, WHITE)],
    [presetItem("Circle", 30, 30, 56, c("#ff3b30")), text(24, 8, 30, "!", 34, WHITE)],
    [presetItem("Star", 30, 30, 60, c("#ffcc00"))],
    [presetItem("Heart", 30, 30, 58, c("#ff6f91"))],
    [line([[8, 30], [24, 46], [54, 12]], c("#34c759"), 6)],
    [line([[10, 10], [50, 50]], c("#ff3b30"), 6), line([[50, 10], [10, 50]], c("#ff3b30"), 6)],
    [newItem({ Shape: { kind: "Arrow", points: [[6, 44], [58, 14]], color: c("#0a84ff"), width: 4, fill: null } })],
    [presetItem("Circle", 30, 24, 40, c("#ffd60a")), presetItem("Square", 30, 50, 14, c("#8e8e93"))],
  ];
  for (const [label, col] of [["Important", "#ff3b30"], ["Exam", "#7b3fe4"], ["Review", "#0a84ff"], ["Done", "#34c759"]]) {
    const w = 18 + label.length * 9.5;
    const [, pts] = presetShape("Pill", w / 2, 18, w);
    packs.push([newItem({ Shape: { kind: "Polygon", points: pts, color: c(col), width: 1, fill: c(col), solid: true } }), text(10, 6, w, label, 16, WHITE)]);
  }
  return packs.map((items) => ({ id: newId(), items, used: 0 }));
}
