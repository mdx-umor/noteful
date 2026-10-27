// The writing surface. Pages scroll natively (smooth momentum on iPad); each page
// has its own canvas that is redrawn only when that page changes. The stroke being
// written, the lasso, selection, laser and ruler live on a separate overlay canvas.
//
// Input: the Apple Pencil (pointerType "pen") always writes. Fingers scroll and
// pinch-zoom, unless "draw with finger" is on. Touches while the Pencil is down are
// treated as the resting palm and ignored.

import type { InkPoint, Item, Page, Paper, Shape, Stroke } from "../core/model";
import { editable, isDark, kindOf, mm, newItem } from "../core/model";
import type { Pt, Rect } from "../core/geom";
import { contains, dist, expand, hitItem, itemInLasso, rdp, recognize, scaleItem, translateItem, unionBBox } from "../core/geom";
import { drawItem, drawPage } from "../core/render";
import { onPictureLoaded, picture } from "../core/pictures";
import { samples, ribbonPath } from "../core/ink";
import type { Prefs } from "../prefs";
import type { DocModel } from "./doc";

export type Tool = "pen" | "highlighter" | "tape" | "eraser" | "lasso" | "hand" | "laser" | "shape" | "text";

export const MIN_ZOOM = 0.25, MAX_ZOOM = 6;
const GAP = 20; // css px between pages
const PAD_TOP = 20;
const HOLD_MS = 500;

export interface Ruler { cx: number; cy: number; angle: number }
const RULER_HALF = 34, RULER_LEN = 1400;

type Action =
  | { k: "idle" }
  | { k: "ink"; page: number; pts: InkPoint[]; smooth: Pt; lastMove: Pt; stillSince: number; snapped: Shape | null; rulerEdge: number | null; hl: boolean }
  | { k: "erase"; page: number; before: Page | null; last: Pt }
  | { k: "lasso"; page: number; pts: Pt[] }
  | { k: "move"; page: number; start: Pt; delta: Pt }
  | { k: "scale"; page: number; anchor: Pt; start: number; factor: number }
  | { k: "shape"; page: number; a: Pt; b: Pt }
  | { k: "tape"; page: number; pts: Pt[] }
  | { k: "pan"; last: Pt }
  | { k: "laser" }
  | { k: "ruler"; last: Pt };

interface PageSlot { canvas: HTMLCanvasElement | null; frame: HTMLDivElement; drawn: Page | null; key: string }

export interface BoardCallbacks {
  onText(page: number, at: Pt, existing: Item | null): void;
  /** Current page, zoom or scroll position changed. */
  onChange(): void;
  onToolSwitch(t: Tool): void;
  onPencil(): void;
}

export class Board {
  zoom = 1;
  tool: Tool = "pen";
  ruler: Ruler | null = null;
  laser: { p: [number, number]; t: number }[] = [];
  private prevTool: Tool = "pen";
  private action: Action = { k: "idle" };
  private slots: PageSlot[] = [];
  private tops: number[] = [];
  private lefts: number[] = [];
  private contentW = 0;
  private overlay: HTMLCanvasElement;
  private octx: CanvasRenderingContext2D;
  private detail: HTMLCanvasElement;
  private raf = 0;
  private overlayDirty = true;
  private pagesDirty = true;
  private penId: number | null = null;
  private penActiveUntil = 0;
  private fitted = false;
  private lastW = 0;
  private pinch: { d0: number; c0: Pt; origin: Pt; s: number; center: Pt } | null = null;
  private multi = { max: 0, start: 0, moved: 0, active: false, pts: new Map<number, Pt>() };
  private downAt: { x: number; y: number; t: number; id: number } | null = null;
  private detailTimer = 0;
  private unsubs: (() => void)[] = [];
  private eraserPos: Pt | null = null;
  private hidden = new Set<number>();
  /** Zoom-window target box (page units), drawn as a blue frame. */
  zoomTarget: { page: number; x: number; y: number; w: number; h: number } | null = null;

  constructor(
    private wrap: HTMLDivElement,
    private scroller: HTMLDivElement,
    private content: HTMLDivElement,
    private model: DocModel,
    private prefs: () => Prefs,
    private cb: BoardCallbacks,
  ) {
    this.overlay = document.createElement("canvas");
    this.overlay.className = "overlay";
    this.wrap.appendChild(this.overlay);
    this.octx = this.overlay.getContext("2d")!;
    this.detail = document.createElement("canvas");
    this.detail.className = "detail";
    this.content.appendChild(this.detail);

    const on = <K extends keyof HTMLElementEventMap>(el: HTMLElement | Window, ev: K, f: (e: HTMLElementEventMap[K]) => void, opts?: AddEventListenerOptions) => {
      el.addEventListener(ev, f as EventListener, opts);
      this.unsubs.push(() => el.removeEventListener(ev, f as EventListener, opts));
    };
    on(scroller, "scroll", () => { this.pagesDirty = true; this.overlayDirty = true; this.scheduleDetail(); this.updateCurrent(); this.cb.onChange(); this.kick(); }, { passive: true });
    on(scroller, "pointerdown", (e) => this.pointerDown(e));
    on(scroller, "pointermove", (e) => this.pointerMove(e));
    on(scroller, "pointerup", (e) => this.pointerUp(e));
    on(scroller, "pointercancel", (e) => this.pointerCancel(e));
    on(scroller, "touchstart", (e) => this.touchStart(e), { passive: false });
    on(scroller, "touchmove", (e) => this.touchMove(e), { passive: false });
    on(scroller, "touchend", (e) => this.touchEnd(e), { passive: false });
    on(scroller, "touchcancel", (e) => this.touchEnd(e), { passive: false });
    on(scroller, "wheel", (e) => this.wheel(e), { passive: false });
    on(scroller, "contextmenu", (e) => e.preventDefault());
    const ro = new ResizeObserver(() => this.resize());
    ro.observe(wrap);
    this.unsubs.push(() => ro.disconnect());
    this.unsubs.push(model.subscribe(() => { this.pagesDirty = true; this.overlayDirty = true; this.kick(); }));
    this.unsubs.push(onPictureLoaded(() => { this.slots.forEach((s) => (s.drawn = null)); this.pagesDirty = true; this.kick(); }));
    this.resize();
  }

  destroy() {
    cancelAnimationFrame(this.raf);
    this.unsubs.forEach((f) => f());
    this.overlay.remove();
    this.detail.remove();
    this.slots.forEach((s) => s.frame.remove());
  }

  // ---------- layout ----------

  get viewW() { return this.scroller.clientWidth; }
  get viewH() { return this.scroller.clientHeight; }
  private maxW() { return Math.max(...this.model.pages.map((p) => p.w)); }
  fitZoom() { return Math.max(MIN_ZOOM, Math.min(2.5, (this.viewW - 32) / this.maxW())); }

  private resize() {
    const dpr = devicePixelRatio || 1;
    const r = this.wrap.getBoundingClientRect();
    this.overlay.width = Math.round(r.width * dpr);
    this.overlay.height = Math.round(r.height * dpr);
    this.overlay.style.width = r.width + "px";
    this.overlay.style.height = r.height + "px";
    if (!this.fitted && r.width > 50) {
      this.fitted = true;
      this.zoom = this.fitZoom();
    } else if (Math.abs(r.width - this.lastW) > 1 && r.width > 50 && this.lastW > 0) {
      const oldFit = Math.max(MIN_ZOOM, Math.min(2.5, (this.lastW - 32) / this.maxW()));
      if (Math.abs(this.zoom - oldFit) < 0.01) {
        const cur = this.model.current;
        this.zoom = this.fitZoom();
        this.layout();
        this.scrollToPage(cur);
      }
    }
    this.lastW = r.width;
    this.layout();
  }

  layout() {
    const z = this.zoom;
    const pages = this.model.pages;
    this.contentW = Math.max(this.viewW, this.maxW() * z + 32);
    let y = PAD_TOP;
    this.tops = [];
    this.lefts = [];
    for (const p of pages) {
      this.tops.push(y);
      this.lefts.push((this.contentW - p.w * z) / 2);
      y += p.h * z + GAP;
    }
    this.content.style.width = this.contentW + "px";
    this.content.style.height = y + this.viewH * 0.4 + "px";
    // Page frames (created once per page, positioned every layout).
    while (this.slots.length < pages.length) {
      const frame = document.createElement("div");
      frame.className = "page";
      this.content.appendChild(frame);
      this.slots.push({ canvas: null, frame, drawn: null, key: "" });
    }
    while (this.slots.length > pages.length) this.slots.pop()!.frame.remove();
    pages.forEach((p, i) => {
      const s = this.slots[i].frame.style;
      s.left = this.lefts[i] + "px";
      s.top = this.tops[i] + "px";
      s.width = p.w * z + "px";
      s.height = p.h * z + "px";
    });
    this.pagesDirty = true;
    this.overlayDirty = true;
    this.kick();
  }

  /** Content coordinates of the top-left of page i. */
  pageOrigin(i: number): Pt { return [this.lefts[i] ?? 0, this.tops[i] ?? 0]; }

  clientToContent(x: number, y: number): Pt {
    const r = this.scroller.getBoundingClientRect();
    return [x - r.left + this.scroller.scrollLeft, y - r.top + this.scroller.scrollTop];
  }
  contentToPage(i: number, c: Pt): Pt {
    return [(c[0] - this.lefts[i]) / this.zoom, (c[1] - this.tops[i]) / this.zoom];
  }
  clientToPage(i: number, x: number, y: number): Pt { return this.contentToPage(i, this.clientToContent(x, y)); }
  /** Page point → position relative to the board's wrapper (for floating UI). */
  pageToView(i: number, p: Pt): Pt {
    return [this.lefts[i] + p[0] * this.zoom - this.scroller.scrollLeft, this.tops[i] + p[1] * this.zoom - this.scroller.scrollTop];
  }
  pageAtContent(c: Pt, margin = GAP / 2): number {
    const pages = this.model.pages;
    for (let i = 0; i < pages.length; i++) {
      const r = { x: this.lefts[i], y: this.tops[i], w: pages[i].w * this.zoom, h: pages[i].h * this.zoom };
      if (contains(expand(r, margin), c[0], c[1])) return i;
    }
    return -1;
  }

  private updateCurrent() {
    const mid = this.scroller.scrollTop + this.viewH / 2;
    let best = 0;
    this.tops.forEach((t, i) => { if (t <= mid) best = i; });
    if (best !== this.model.current) {
      this.model.current = Math.min(best, this.model.pages.length - 1);
      this.cb.onChange();
    }
  }

  scrollToPage(i: number) {
    if (this.tops[i] === undefined) return;
    this.scroller.scrollTop = this.tops[i] - PAD_TOP;
    this.model.current = i;
    this.cb.onChange();
  }

  setZoom(z: number, anchorClient?: Pt) {
    z = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, z));
    const r = this.scroller.getBoundingClientRect();
    const ax = anchorClient ? anchorClient[0] - r.left : this.viewW / 2;
    const ay = anchorClient ? anchorClient[1] - r.top : this.viewH / 2;
    const cx = ax + this.scroller.scrollLeft, cy = ay + this.scroller.scrollTop;
    const i = Math.max(0, this.pageAtContent([cx, cy], 1e9));
    const pp = this.contentToPage(i, [cx, cy]);
    this.zoom = z;
    this.layout();
    this.scroller.scrollLeft = this.lefts[i] + pp[0] * z - ax;
    this.scroller.scrollTop = this.tops[i] + pp[1] * z - ay;
    this.cb.onChange();
  }

  fitWidth() {
    const cur = this.model.current;
    this.zoom = this.fitZoom();
    this.layout();
    this.scrollToPage(cur);
  }

  // ---------- rendering ----------

  kick() {
    if (!this.raf) this.raf = requestAnimationFrame(() => { this.raf = 0; this.frame(); });
  }

  private paperOf(p: Page): Paper {
    return this.prefs().night_mode && !isDark(p.paper) ? "Night" : p.paper;
  }

  /** Rendering scale for a page canvas, capped to keep memory reasonable on iPad. */
  private canvasScale(p: Page) {
    const want = this.zoom * (devicePixelRatio || 1);
    return Math.min(want, 4096 / p.w, Math.sqrt(12e6 / (p.w * p.h)));
  }

  private frame() {
    if (this.model.scrollTo !== null) {
      const i = this.model.scrollTo;
      this.model.scrollTo = null;
      this.layout();
      this.scrollToPage(i);
    }
    if (this.slots.length !== this.model.pages.length) this.layout();
    if (this.pagesDirty) {
      this.pagesDirty = false;
      this.renderPages();
    }
    if (this.overlayDirty) {
      this.overlayDirty = false;
      this.renderOverlay();
    }
    this.tick();
    if (this.action.k !== "idle" || this.laser.length) this.kick();
  }

  private renderPages() {
    const top = this.scroller.scrollTop, bottom = top + this.viewH;
    const pages = this.model.pages;
    const night = this.prefs().night_mode;
    const exclude = this.excludeSet();
    pages.forEach((p, i) => {
      const slot = this.slots[i];
      const y0 = this.tops[i], y1 = y0 + p.h * this.zoom;
      const near = y1 > top - this.viewH && y0 < bottom + this.viewH;
      if (!near) {
        if (slot.canvas) { slot.canvas.width = slot.canvas.height = 0; slot.canvas.remove(); slot.canvas = null; slot.drawn = null; }
        return;
      }
      const scale = this.canvasScale(p);
      const key = `${scale.toFixed(3)}|${night}|${[...exclude].join(",")}`;
      if (!slot.canvas) {
        slot.canvas = document.createElement("canvas");
        slot.frame.appendChild(slot.canvas);
      }
      if (slot.drawn === p && slot.key === key) return;
      const c = slot.canvas;
      const w = Math.max(1, Math.round(p.w * scale)), h = Math.max(1, Math.round(p.h * scale));
      if (c.width !== w || c.height !== h) { c.width = w; c.height = h; }
      const ctx = c.getContext("2d")!;
      ctx.setTransform(scale, 0, 0, scale, 0, 0);
      drawPage(ctx, p, { paper: this.paperOf(p), image: picture, exclude: exclude.size && this.model.selection?.page === i ? exclude : undefined });
      slot.drawn = p;
      slot.key = key;
    });
  }

  private excludeSet(): Set<number> {
    const a = this.action;
    const s = new Set<number>(this.hidden);
    if ((a.k === "move" || a.k === "scale") && this.model.selection) this.model.selection.ids.forEach((id) => s.add(id));
    return s;
  }

  /** Items hidden from the page canvas (e.g. a text box being edited). */
  setHidden(ids: number[]) {
    this.hidden = new Set(ids);
    this.pagesDirty = true;
    this.kick();
  }

  /** Sharp re-render of the visible area when zoomed past the page canvas limit. */
  private scheduleDetail() {
    this.detail.style.display = "none";
    clearTimeout(this.detailTimer);
    this.detailTimer = window.setTimeout(() => this.renderDetail(), 160);
  }
  private renderDetail() {
    const dpr = devicePixelRatio || 1;
    const want = this.zoom * dpr;
    const pages = this.model.pages;
    const sx = this.scroller.scrollLeft, sy = this.scroller.scrollTop, vw = this.viewW, vh = this.viewH;
    const visible = pages.map((_, i) => i).filter((i) => this.tops[i] < sy + vh && this.tops[i] + pages[i].h * this.zoom > sy);
    if (!visible.some((i) => this.canvasScale(pages[i]) < want - 0.01) || this.action.k !== "idle") return;
    const c = this.detail;
    c.width = Math.round(vw * dpr);
    c.height = Math.round(vh * dpr);
    c.style.width = vw + "px";
    c.style.height = vh + "px";
    c.style.left = sx + "px";
    c.style.top = sy + "px";
    const ctx = c.getContext("2d")!;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, c.width, c.height);
    const exclude = this.excludeSet();
    for (const i of visible) {
      const p = pages[i];
      ctx.save();
      ctx.setTransform(want, 0, 0, want, (this.lefts[i] - sx) * dpr, (this.tops[i] - sy) * dpr);
      ctx.beginPath();
      ctx.rect(0, 0, p.w, p.h);
      ctx.clip();
      drawPage(ctx, p, { paper: this.paperOf(p), image: picture, exclude });
      ctx.restore();
    }
    c.style.display = "block";
  }

  /** View transform for drawing page-unit things on the overlay. */
  private overlayPage(i: number) {
    const dpr = devicePixelRatio || 1;
    const z = this.zoom * dpr;
    this.octx.setTransform(z, 0, 0, z, (this.lefts[i] - this.scroller.scrollLeft) * dpr, (this.tops[i] - this.scroller.scrollTop) * dpr);
  }

  private renderOverlay() {
    const ctx = this.octx;
    const dpr = devicePixelRatio || 1;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, this.overlay.width, this.overlay.height);
    if (this.pinch) return;
    const p = this.prefs();
    const a = this.action;
    const pages = this.model.pages;
    // Selection being moved / scaled, and the selection box.
    const sel = this.model.selection;
    if (sel && pages[sel.page]) {
      const items = this.model.selectedItems();
      this.overlayPage(sel.page);
      let shown = items;
      if (a.k === "move") shown = items.map((it) => translateItem(it, a.delta[0], a.delta[1]));
      else if (a.k === "scale") shown = items.map((it) => scaleItem(it, a.anchor[0], a.anchor[1], a.factor));
      if (a.k === "move" || a.k === "scale") {
        ctx.save();
        ctx.beginPath();
        ctx.rect(0, 0, pages[sel.page].w, pages[sel.page].h);
        ctx.clip();
        for (const it of shown) drawItem(ctx, it, { paper: this.paperOf(pages[sel.page]), image: picture });
        ctx.restore();
      }
      const b = unionBBox(shown);
      if (b) {
        const z = this.zoom;
        const r = expand(b, 6 / z);
        ctx.lineWidth = 1.5 / z;
        ctx.strokeStyle = "#0a84ff";
        ctx.setLineDash([7 / z, 5 / z]);
        ctx.strokeRect(r.x, r.y, r.w, r.h);
        ctx.setLineDash([]);
        ctx.fillStyle = "#fff";
        ctx.beginPath();
        ctx.arc(r.x + r.w, r.y + r.h, 9 / z, 0, Math.PI * 2);
        ctx.fill();
        ctx.lineWidth = 2 / z;
        ctx.stroke();
      }
    }
    // Live previews.
    if (a.k === "ink") {
      this.overlayPage(a.page);
      const paper = this.paperOf(pages[a.page]);
      if (a.snapped) drawItem(ctx, newItem({ Shape: a.snapped }), { paper });
      else {
        const st = this.liveStroke(a);
        if (st) {
          if (a.hl) drawItem(ctx, newItem({ Stroke: st }), { paper });
          else {
            const path = new Path2D();
            ribbonPath(path, samples(st));
            ctx.fillStyle = drawColor(st, paper);
            ctx.fill(path);
          }
        }
      }
    } else if (a.k === "shape") {
      this.overlayPage(a.page);
      drawItem(ctx, newItem({ Shape: this.dragShape(a.a, a.b) }), { paper: this.paperOf(pages[a.page]) });
    } else if (a.k === "tape" && a.pts.length > 1) {
      this.overlayPage(a.page);
      const t = this.tapeFrom(a.pts);
      drawItem(ctx, newItem({ Tape: { ...t, points: p.tape_kind === "Free" ? a.pts : t.points } }), { paper: "White" });
    } else if (a.k === "lasso" && a.pts.length > 1) {
      this.overlayPage(a.page);
      const pts = p.lasso_rect ? rectPoly(a.pts[0], a.pts[a.pts.length - 1]) : a.pts;
      ctx.strokeStyle = "#0a84ff";
      ctx.lineWidth = 1.6 / this.zoom;
      ctx.setLineDash([6 / this.zoom, 4 / this.zoom]);
      ctx.beginPath();
      pts.forEach((q, k) => (k ? ctx.lineTo(q[0], q[1]) : ctx.moveTo(q[0], q[1])));
      ctx.closePath();
      ctx.stroke();
      ctx.setLineDash([]);
    }
    if (a.k === "erase" && this.eraserPos) {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const w = this.wrap.getBoundingClientRect();
      ctx.strokeStyle = "rgba(140,140,150,0.9)";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(this.eraserPos[0] - w.left, this.eraserPos[1] - w.top, p.eraser_size, 0, Math.PI * 2);
      ctx.stroke();
    }
    const zt = this.zoomTarget;
    if (zt && pages[zt.page]) {
      this.overlayPage(zt.page);
      ctx.strokeStyle = "#0a84ff";
      ctx.lineWidth = 2 / this.zoom;
      ctx.strokeRect(zt.x, zt.y, zt.w, zt.h);
      ctx.fillStyle = "rgba(10,132,255,0.06)";
      ctx.fillRect(zt.x, zt.y, zt.w, zt.h);
    }
    this.renderRuler();
    this.renderLaser();
  }

  private renderRuler() {
    const r = this.ruler;
    if (!r) return;
    const ctx = this.octx, dpr = devicePixelRatio || 1, z = this.zoom * dpr;
    // The ruler lives in content coordinates of page 0's frame space (document units).
    ctx.setTransform(z, 0, 0, z, (this.contentW / 2 - this.scroller.scrollLeft) * dpr, (PAD_TOP - this.scroller.scrollTop) * dpr);
    ctx.save();
    ctx.translate(r.cx, r.cy);
    ctx.rotate(r.angle);
    ctx.fillStyle = "rgba(235,235,240,0.72)";
    ctx.strokeStyle = "rgba(120,120,120,1)";
    ctx.lineWidth = 1 / this.zoom;
    ctx.fillRect(-RULER_LEN / 2, -RULER_HALF, RULER_LEN, RULER_HALF * 2);
    ctx.strokeRect(-RULER_LEN / 2, -RULER_HALF, RULER_LEN, RULER_HALF * 2);
    ctx.strokeStyle = "rgba(70,70,70,1)";
    const step = mm(5);
    ctx.beginPath();
    for (let k = -Math.floor(RULER_LEN / 2 / step); k * step <= RULER_LEN / 2; k++) {
      const x = k * step, len = k % 2 === 0 ? 14 : 8;
      ctx.moveTo(x, -RULER_HALF); ctx.lineTo(x, -RULER_HALF + len);
      ctx.moveTo(x, RULER_HALF); ctx.lineTo(x, RULER_HALF - len);
    }
    ctx.stroke();
    ctx.fillStyle = "#333";
    ctx.font = "11px -apple-system, sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    for (let k = -Math.floor(RULER_LEN / 2 / step); k * step <= RULER_LEN / 2; k += 2) if (k) ctx.fillText(String(k / 2), k * step, 0);
    ctx.font = "15px -apple-system, sans-serif";
    ctx.fillText(`${Math.round(((r.angle * 180) / Math.PI % 180 + 180) % 180)}°`, 0, 0);
    ctx.restore();
  }

  private renderLaser() {
    if (this.laser.length < 2) return;
    const ctx = this.octx, dpr = devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const c = this.prefs().laser_color;
    const now = performance.now();
    for (let k = 1; k < this.laser.length; k++) {
      const a = this.laser[k - 1], b = this.laser[k];
      if (dist(a.p, b.p) > 200) continue;
      const fade = b.t < 0 ? 1 : Math.max(0, 1 - (now - b.t) / 1000);
      const pa = this.docToView(a.p), pb = this.docToView(b.p);
      ctx.lineCap = "round";
      ctx.strokeStyle = `rgba(${c[0]},${c[1]},${c[2]},${0.25 * fade})`;
      ctx.lineWidth = 10;
      ctx.beginPath(); ctx.moveTo(pa[0], pa[1]); ctx.lineTo(pb[0], pb[1]); ctx.stroke();
      ctx.strokeStyle = `rgba(${c[0]},${c[1]},${c[2]},${fade})`;
      ctx.lineWidth = 3.5;
      ctx.beginPath(); ctx.moveTo(pa[0], pa[1]); ctx.lineTo(pb[0], pb[1]); ctx.stroke();
    }
  }

  /** Document units: content coordinates / zoom, origin at the top centre of the stack. */
  private contentToDoc(c: Pt): Pt { return [(c[0] - this.contentW / 2) / this.zoom, (c[1] - PAD_TOP) / this.zoom]; }
  private docToView(d: Pt): Pt {
    return [this.contentW / 2 + d[0] * this.zoom - this.scroller.scrollLeft, PAD_TOP + d[1] * this.zoom - this.scroller.scrollTop];
  }

  private tick() {
    const now = performance.now();
    if (this.laser.length) {
      this.laser = this.laser.filter((l) => l.t < 0 || now - l.t < 1000);
      this.overlayDirty = true;
    }
    const a = this.action;
    if (a.k === "ink" && this.prefs().shape_hold && !a.snapped && a.rulerEdge === null && a.pts.length > 6 && now - a.stillSince > HOLD_MS) {
      const hit = recognize(a.pts.map((q) => [q.x, q.y]));
      a.stillSince = Infinity;
      if (hit && (!a.hl || hit[0] === "Line")) {
        const p = this.prefs();
        a.snapped = { kind: hit[0], points: hit[1], color: a.hl ? p.hl_color : p.pen_color, width: a.hl ? mm(p.hl_mm) : mm(p.pen_mm), fill: null, dashed: false, solid: false, dotted: false };
        this.overlayDirty = true;
      }
    }
  }

  // ---------- input ----------

  private writes(e: PointerEvent) {
    if (e.pointerType === "pen") return true;
    if (e.pointerType === "mouse") return e.button === 0;
    return !this.prefs().pencil_only;
  }

  private pointerDown(e: PointerEvent) {
    if (e.pointerType === "pen") {
      if (!this.prefs().pencil_seen) this.cb.onPencil();
      this.penActiveUntil = Infinity;
    }
    if (e.pointerType === "touch" && (this.penId !== null || this.multi.pts.size > 1)) return;
    this.downAt = { x: e.clientX, y: e.clientY, t: performance.now(), id: e.pointerId };
    if (!this.writes(e)) return;
    if (this.penId !== null) return;
    this.penId = e.pointerId;
    try { this.scroller.setPointerCapture(e.pointerId); } catch { /* ignore */ }
    e.preventDefault();
    this.down(e.clientX, e.clientY, this.pressure(e), e.pointerType);
  }

  private pointerMove(e: PointerEvent) {
    if (e.pointerId !== this.penId) {
      if (this.tool === "eraser" && e.pointerType !== "touch") { this.eraserPos = [e.clientX, e.clientY]; }
      return;
    }
    const events = (e as PointerEvent & { getCoalescedEvents?: () => PointerEvent[] }).getCoalescedEvents?.() ?? [];
    for (const ev of events.length ? events : [e]) this.drag(ev.clientX, ev.clientY, this.pressure(ev));
    this.overlayDirty = true;
    this.kick();
  }

  private pointerUp(e: PointerEvent) {
    if (e.pointerId === this.penId) {
      this.penId = null;
      if (e.pointerType === "pen") this.penActiveUntil = performance.now() + 400;
      this.up(e.clientX, e.clientY);
    } else if (this.downAt && this.downAt.id === e.pointerId && e.pointerType === "touch") {
      // A finger tap (no scroll happened, so no pointercancel).
      const d = this.downAt;
      if (Math.hypot(e.clientX - d.x, e.clientY - d.y) < 10 && performance.now() - d.t < 350 && !this.multi.active) this.tap(e.clientX, e.clientY);
    }
    this.downAt = null;
  }

  private pointerCancel(e: PointerEvent) {
    if (e.pointerId === this.penId) {
      this.penId = null;
      this.cancel();
    }
    this.downAt = null;
  }

  private pressure(e: PointerEvent) {
    if (e.pointerType === "pen" && e.pressure > 0) return Math.max(0.03, Math.min(1, e.pressure * 1.3 * this.prefs().pressure_gain));
    if (e.pointerType === "pen") {
      const a = this.action;
      return a.k === "ink" && a.pts.length ? a.pts[a.pts.length - 1].p : 0.5;
    }
    return 0.5;
  }

  private touchStart(e: TouchEvent) {
    const stylus = [...e.changedTouches].some((t) => (t as Touch & { touchType?: string }).touchType === "stylus");
    const palm = this.penId !== null || performance.now() < this.penActiveUntil;
    if (stylus) { e.preventDefault(); return; }
    if (palm) { e.preventDefault(); return; }
    const m = this.multi;
    if (!m.active) { m.active = true; m.max = 0; m.start = performance.now(); m.moved = 0; m.pts.clear(); }
    for (const t of e.changedTouches) m.pts.set(t.identifier, [t.clientX, t.clientY]);
    m.max = Math.max(m.max, e.touches.length);
    if (e.touches.length >= 2) {
      e.preventDefault();
      if (this.penId !== null && !this.prefs().pencil_only) { this.penId = null; this.cancel(); }
      const [a, b] = [e.touches[0], e.touches[1]];
      const c: Pt = [(a.clientX + b.clientX) / 2, (a.clientY + b.clientY) / 2];
      const cc = this.clientToContent(c[0], c[1]);
      this.pinch = { d0: Math.max(1, Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY)), c0: c, origin: cc, s: 1, center: c };
      this.content.style.transformOrigin = `${cc[0]}px ${cc[1]}px`;
      this.overlayDirty = true;
      this.kick();
    } else if (!this.prefs().pencil_only) {
      // Finger writes: no native scrolling for single touches.
      e.preventDefault();
    } else if (this.ruler && e.touches.length === 1) {
      const t = e.touches[0];
      const d = this.contentToDoc(this.clientToContent(t.clientX, t.clientY));
      if (this.onRuler(d)) {
        e.preventDefault();
        this.action = { k: "ruler", last: [t.clientX, t.clientY] };
      }
    }
  }

  private touchMove(e: TouchEvent) {
    if ([...e.touches].some((t) => (t as Touch & { touchType?: string }).touchType === "stylus") || this.penId !== null && e.touches.length < 2) {
      if (this.penId !== null) e.preventDefault();
      return;
    }
    for (const t of e.changedTouches) {
      const p = this.multi.pts.get(t.identifier);
      if (p) this.multi.moved += Math.hypot(t.clientX - p[0], t.clientY - p[1]);
      this.multi.pts.set(t.identifier, [t.clientX, t.clientY]);
    }
    if (this.action.k === "ruler" && e.touches.length === 1) {
      e.preventDefault();
      const t = e.touches[0];
      const a = this.action;
      if (this.ruler) { this.ruler.cx += (t.clientX - a.last[0]) / this.zoom; this.ruler.cy += (t.clientY - a.last[1]) / this.zoom; }
      a.last = [t.clientX, t.clientY];
      this.overlayDirty = true;
      this.kick();
      return;
    }
    if (this.pinch && e.touches.length >= 2) {
      e.preventDefault();
      const [a, b] = [e.touches[0], e.touches[1]];
      const d = Math.max(1, Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY));
      const c: Pt = [(a.clientX + b.clientX) / 2, (a.clientY + b.clientY) / 2];
      const pz = this.pinch;
      pz.s = Math.max(MIN_ZOOM / this.zoom, Math.min(MAX_ZOOM / this.zoom, d / pz.d0));
      pz.center = c;
      this.content.style.transform = `translate(${c[0] - pz.c0[0]}px, ${c[1] - pz.c0[1]}px) scale(${pz.s})`;
      this.detail.style.display = "none";
    }
  }

  private touchEnd(e: TouchEvent) {
    if (this.action.k === "ruler" && e.touches.length === 0) this.action = { k: "idle" };
    for (const t of e.changedTouches) this.multi.pts.delete(t.identifier);
    if (this.pinch && e.touches.length < 2) {
      const pz = this.pinch;
      this.pinch = null;
      this.content.style.transform = "";
      const z = this.zoom * pz.s;
      // Keep the pinched point under the fingers.
      const i = Math.max(0, this.pageAtContent(pz.origin, 1e9));
      const pp = this.contentToPage(i, pz.origin);
      const r = this.scroller.getBoundingClientRect();
      this.zoom = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, z));
      this.layout();
      this.scroller.scrollLeft = this.lefts[i] + pp[0] * this.zoom - (pz.center[0] - r.left);
      this.scroller.scrollTop = this.tops[i] + pp[1] * this.zoom - (pz.center[1] - r.top);
      this.cb.onChange();
    }
    if (e.touches.length === 0 && this.multi.active) {
      const m = this.multi;
      m.active = false;
      const quick = performance.now() - m.start < 300 && m.moved < 30;
      if (quick && this.prefs().two_finger_undo && e.type === "touchend") {
        if (m.max === 2) { this.model.undo(); this.model.toast("Undo"); }
        else if (m.max === 3) { this.model.redo(); this.model.toast("Redo"); }
      }
    }
  }

  private wheel(e: WheelEvent) {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      this.setZoom(this.zoom * Math.exp(-e.deltaY * 0.01), [e.clientX, e.clientY]);
    }
  }

  // ---------- tool actions ----------

  private onRuler(d: Pt) {
    const r = this.ruler;
    if (!r) return false;
    const dx = d[0] - r.cx, dy = d[1] - r.cy;
    const along = dx * Math.cos(r.angle) + dy * Math.sin(r.angle);
    const off = -dx * Math.sin(r.angle) + dy * Math.cos(r.angle);
    return Math.abs(off) <= RULER_HALF && Math.abs(along) <= RULER_LEN / 2;
  }

  /** Is a view point on the selection? "handle" for the resize corner. */
  selectionHit(x: number, y: number): "handle" | "body" | null {
    const sel = this.model.selection;
    const b = this.model.selectionBox();
    if (!sel || !b) return null;
    const c = this.clientToContent(x, y);
    const p = this.contentToPage(sel.page, c);
    const z = this.zoom;
    const r = expand(b, 6 / z);
    if (Math.hypot(p[0] - (r.x + r.w), p[1] - (r.y + r.h)) * z < 26) return "handle";
    return contains(expand(r, 4 / z), p[0], p[1]) ? "body" : null;
  }

  private down(x: number, y: number, pressure: number, device: string) {
    const m = this.model;
    const prefs = this.prefs();
    const hit = this.selectionHit(x, y);
    if (hit && m.selection) {
      const page = m.selection.page;
      const pt = this.clientToPage(page, x, y);
      if (hit === "handle") {
        const b = m.selectionBox()!;
        const anchor: Pt = [b.x, b.y];
        this.action = { k: "scale", page, anchor, start: Math.max(1, dist(anchor, pt)), factor: 1 };
      } else this.action = { k: "move", page, start: pt, delta: [0, 0] };
      this.pagesDirty = true;
      return;
    }
    const content = this.clientToContent(x, y);
    if (this.tool === "hand" || (device === "mouse" && this.pageAtContent(content) < 0)) {
      this.action = { k: "pan", last: [x, y] };
      return;
    }
    const doc = this.contentToDoc(content);
    if (this.ruler && this.onRuler(doc) && this.tool !== "pen" && this.tool !== "highlighter") {
      this.action = { k: "ruler", last: [x, y] };
      return;
    }
    const page = this.pageAtContent(content);
    if (page < 0) {
      if (m.selection) { m.selection = null; m.notify(); }
      return;
    }
    const pt = this.contentToPage(page, content);
    const writes = ["pen", "highlighter", "tape", "shape", "text"].includes(this.tool);
    if (writes && !m.canWrite(page)) {
      if (!this.tapItem(page, pt, true)) m.toast("The active layer is locked or hidden. Pick another in Layers.");
      return;
    }
    if (m.selection && this.tool !== "lasso") { m.selection = null; m.notify(); }
    switch (this.tool) {
      case "pen":
      case "highlighter": {
        let rulerEdge: number | null = null;
        if (this.ruler) {
          const r = this.ruler;
          const dx = doc[0] - r.cx, dy = doc[1] - r.cy;
          const off = -dx * Math.sin(r.angle) + dy * Math.cos(r.angle);
          const along = dx * Math.cos(r.angle) + dy * Math.sin(r.angle);
          const near = 28 / this.zoom;
          if (Math.abs(along) <= RULER_LEN / 2) {
            if (Math.abs(off - RULER_HALF) < near) rulerEdge = 1;
            else if (Math.abs(off + RULER_HALF) < near) rulerEdge = 0;
          }
        }
        this.action = { k: "ink", page, pts: [{ x: pt[0], y: pt[1], p: pressure }], smooth: pt, lastMove: [x, y], stillSince: performance.now(), snapped: null, rulerEdge, hl: this.tool === "highlighter" };
        break;
      }
      case "eraser":
        this.prevTool = this.prevTool === "eraser" ? "pen" : this.prevTool;
        this.action = { k: "erase", page, before: null, last: pt };
        this.eraserPos = [x, y];
        this.eraseAt(page, pt);
        break;
      case "lasso":
        if (m.selection) { m.selection = null; m.notify(); }
        this.action = { k: "lasso", page, pts: [pt] };
        break;
      case "shape":
        this.action = { k: "shape", page, a: pt, b: pt };
        break;
      case "tape":
        this.action = { k: "tape", page, pts: [pt] };
        break;
      case "text": {
        const existing = m.itemAt(page, pt, 4, (it) => "Text" in it.content && editable(m.page(page), it)) ?? null;
        this.cb.onText(page, pt, existing);
        break;
      }
      case "laser":
        this.laser.push({ p: doc, t: -1 });
        this.action = { k: "laser" };
        break;
    }
    void prefs;
    this.overlayDirty = true;
    this.kick();
  }

  private drag(x: number, y: number, pressure: number) {
    const a = this.action;
    const prefs = this.prefs();
    const z = this.zoom;
    switch (a.k) {
      case "ink": {
        const raw = this.clientToPage(a.page, x, y);
        if (Math.hypot(x - a.lastMove[0], y - a.lastMove[1]) > 2.5) { a.stillSince = performance.now(); a.lastMove = [x, y]; }
        if (a.snapped) {
          if ((a.snapped.kind === "Line") && a.snapped.points.length === 2) a.snapped.points[1] = raw;
          return;
        }
        // Adaptive smoothing: strong when slow (steady lines), light when fast (no lag).
        const stab = Math.max(0, Math.min(1, prefs.stabilization));
        const base = 1 - stab * 0.75;
        const speed = Math.hypot(raw[0] - a.smooth[0], raw[1] - a.smooth[1]) * z;
        const k = base + (1 - base) * (speed / (speed + 14));
        const s: Pt = [a.smooth[0] + (raw[0] - a.smooth[0]) * k, a.smooth[1] + (raw[1] - a.smooth[1]) * k];
        a.smooth = s;
        const last = a.pts[a.pts.length - 1];
        if (Math.hypot(s[0] - last.x, s[1] - last.y) * z > 0.6) a.pts.push({ x: s[0], y: s[1], p: pressure });
        else last.p = Math.max(last.p, pressure);
        break;
      }
      case "erase": {
        const to = this.clientToPage(a.page, x, y);
        const r = prefs.eraser_size / z;
        const steps = Math.max(1, Math.min(64, Math.ceil(dist(a.last, to) / (r * 0.5))));
        for (let k = 1; k <= steps; k++) {
          const t = k / steps;
          this.eraseAt(a.page, [a.last[0] + (to[0] - a.last[0]) * t, a.last[1] + (to[1] - a.last[1]) * t]);
        }
        a.last = to;
        this.eraserPos = [x, y];
        break;
      }
      case "lasso": {
        const pt = this.clientToPage(a.page, x, y);
        const l = a.pts[a.pts.length - 1];
        if (dist(l, pt) * z > 2) a.pts.push(pt);
        break;
      }
      case "tape": {
        const pt = this.clientToPage(a.page, x, y);
        if (prefs.tape_kind === "Free") {
          if (dist(a.pts[a.pts.length - 1], pt) * z > 2) a.pts.push(pt);
        } else a.pts = [a.pts[0], pt];
        break;
      }
      case "move": {
        const pt = this.clientToPage(a.page, x, y);
        a.delta = [pt[0] - a.start[0], pt[1] - a.start[1]];
        break;
      }
      case "scale": {
        const pt = this.clientToPage(a.page, x, y);
        a.factor = Math.max(0.1, Math.min(20, dist(a.anchor, pt) / a.start));
        break;
      }
      case "shape": {
        let pt = this.clientToPage(a.page, x, y);
        if (prefs.shape_kind === "Line" || prefs.shape_kind === "Arrow") {
          const ang = Math.atan2(pt[1] - a.a[1], pt[0] - a.a[0]);
          const step = Math.PI / 4, snapped = Math.round(ang / step) * step;
          if (Math.abs(ang - snapped) < 0.06) { const l = dist(a.a, pt); pt = [a.a[0] + l * Math.cos(snapped), a.a[1] + l * Math.sin(snapped)]; }
        }
        a.b = pt;
        break;
      }
      case "pan":
        this.scroller.scrollLeft -= x - a.last[0];
        this.scroller.scrollTop -= y - a.last[1];
        a.last = [x, y];
        break;
      case "laser":
        this.laser.push({ p: this.contentToDoc(this.clientToContent(x, y)), t: -1 });
        break;
      case "ruler":
        if (this.ruler) { this.ruler.cx += (x - a.last[0]) / z; this.ruler.cy += (y - a.last[1]) / z; }
        a.last = [x, y];
        break;
    }
  }

  private up(x: number, y: number) {
    const a = this.action;
    this.action = { k: "idle" };
    const m = this.model;
    const prefs = this.prefs();
    const tap = this.downAt && Math.hypot(x - this.downAt.x, y - this.downAt.y) < 8 && performance.now() - this.downAt.t < 400;
    switch (a.k) {
      case "ink": {
        const pt = this.clientToPage(a.page, x, y);
        if (tap && this.tapItem(a.page, pt, false)) break;
        if (a.snapped) {
          if (a.hl) m.addItem(a.page, newItem({ Stroke: { pen: "Highlighter", color: prefs.hl_color, width: mm(prefs.hl_mm), points: a.snapped.points.slice(0, 2).map(([px, py]) => ({ x: px, y: py, p: 0.5 })) } }));
          else m.addItem(a.page, newItem({ Shape: a.snapped }));
          break;
        }
        const st = this.liveStroke(a, pt);
        if (st) m.addItem(a.page, newItem({ Stroke: st }));
        break;
      }
      case "erase":
        if (a.before) {
          // Commit the whole erase gesture as one undo step.
          const after = m.page(a.page);
          m.setPage(a.page, a.before, false);
          m.setPage(a.page, after, true);
        }
        this.eraserPos = null;
        if (prefs.eraser_return && this.prevTool !== "eraser") this.cb.onToolSwitch(this.prevTool);
        break;
      case "tape": {
        if (tap || a.pts.length < 2) { this.tapItem(a.page, a.pts[0], true); break; }
        const t = this.tapeFrom(a.pts);
        if (t.points.length >= 2 && dist(t.points[0], t.points[t.points.length - 1]) * this.zoom < 6 && t.points.length < 3) break;
        m.addItem(a.page, newItem({ Tape: t }));
        break;
      }
      case "lasso": {
        let pts = a.pts;
        let len = 0;
        for (let k = 1; k < pts.length; k++) len += dist(pts[k - 1], pts[k]);
        len *= this.zoom;
        if (prefs.lasso_rect && pts.length > 1) pts = rectPoly(pts[0], pts[pts.length - 1]);
        const pg = m.page(a.page);
        const want = (it: Item) => {
          if (!editable(pg, it)) return false;
          const k = kindOf(it);
          return ({ ink: prefs.lasso_ink, highlight: prefs.lasso_hl, text: prefs.lasso_text, shape: prefs.lasso_shapes, tape: prefs.lasso_tapes, image: prefs.lasso_images })[k];
        };
        let ids: number[];
        if (len < 12) {
          const hit = [...pg.items].reverse().find((it) => want(it) && hitItem(it, pts[0][0], pts[0][1], 8 / this.zoom));
          ids = hit ? [hit.id] : [];
        } else ids = pg.items.filter((it) => want(it) && itemInLasso(it, pts)).map((it) => it.id);
        m.selection = ids.length ? { page: a.page, ids } : null;
        m.notify();
        break;
      }
      case "move":
        if (Math.abs(a.delta[0]) + Math.abs(a.delta[1]) > 0.01) m.mapSelection((it) => translateItem(it, a.delta[0], a.delta[1]));
        else m.notify();
        break;
      case "scale":
        if (Math.abs(a.factor - 1) > 0.001) m.mapSelection((it) => scaleItem(it, a.anchor[0], a.anchor[1], a.factor));
        else m.notify();
        break;
      case "shape":
        if (dist(a.a, a.b) * this.zoom > 4) m.addItem(a.page, newItem({ Shape: this.dragShape(a.a, a.b) }));
        break;
      case "laser": {
        const t = performance.now();
        this.laser.forEach((l) => { if (l.t < 0) l.t = t; });
        break;
      }
      case "pan":
        if (tap) this.tap(x, y);
        break;
    }
    this.pagesDirty = true;
    this.overlayDirty = true;
    this.kick();
  }

  private cancel() {
    const a = this.action;
    this.action = { k: "idle" };
    if (a.k === "erase" && a.before) {
      const after = this.model.page(a.page);
      this.model.setPage(a.page, a.before, false);
      this.model.setPage(a.page, after, true);
    }
    this.pagesDirty = this.overlayDirty = true;
    this.kick();
  }

  /** Finger tap or hand-tool tap: tape, links, text, clear selection. */
  private tap(x: number, y: number) {
    const m = this.model;
    const c = this.clientToContent(x, y);
    const page = this.pageAtContent(c);
    if (page < 0) return;
    const pt = this.contentToPage(page, c);
    if (this.tool !== "text" && this.tapItem(page, pt, true)) return;
    const text = m.itemAt(page, pt, 4, (it) => "Text" in it.content && editable(m.page(page), it));
    if (text || this.tool === "text") { this.cb.onText(page, pt, text ?? null); return; }
    if (m.selection) { m.selection = null; m.notify(); }
  }

  /** Tape toggles; links are followed (not with a pen dot). */
  private tapItem(page: number, pt: Pt, links: boolean): boolean {
    const m = this.model;
    const r = 6 / this.zoom;
    if (m.onInkTap) {
      const ink = m.itemAt(page, pt, r + 2 / this.zoom, (it) => "Stroke" in it.content && !!it.content.Stroke.audio);
      if (ink && m.onInkTap(ink)) return true;
    }
    const tape = m.itemAt(page, pt, r, (it) => "Tape" in it.content);
    if (tape) { m.toggleTape(page, tape.id); return true; }
    if (links) {
      const it = m.itemAt(page, pt, r, (x) => !!x.link);
      if (it?.link) { m.followLink(it.link); return true; }
    }
    return false;
  }

  private liveStroke(a: Extract<Action, { k: "ink" }>, end?: Pt): Stroke | null {
    const prefs = this.prefs();
    let pts = a.pts.slice();
    if (end) {
      const l = pts[pts.length - 1];
      if (Math.hypot(l.x - end[0], l.y - end[1]) * this.zoom > 0.6) pts.push({ x: end[0], y: end[1], p: l.p });
    }
    if (!pts.length) return null;
    const width = a.hl ? mm(prefs.hl_mm) : mm(prefs.pen_mm);
    if (a.rulerEdge !== null && this.ruler) pts = this.projectOnRuler(a.page, pts, a.rulerEdge, width);
    else if (a.hl && prefs.hl_straight && pts.length > 1) pts = [pts[0], pts[pts.length - 1]];
    return a.hl
      ? { pen: "Highlighter", color: prefs.hl_color, width, points: pts }
      : { pen: prefs.pen_kind, color: prefs.pen_color, width, points: pts };
  }

  private projectOnRuler(page: number, pts: InkPoint[], edge: number, width: number): InkPoint[] {
    const r = this.ruler!;
    const side = edge === 1 ? 1 : -1;
    const off = side * (RULER_HALF + width / 2 + 0.5);
    const d: Pt = [Math.cos(r.angle), Math.sin(r.angle)], n: Pt = [-Math.sin(r.angle), Math.cos(r.angle)];
    const proj = (q: InkPoint): InkPoint => {
      const doc = this.contentToDoc([this.lefts[page] + q.x * this.zoom, this.tops[page] + q.y * this.zoom]);
      const t = (doc[0] - r.cx) * d[0] + (doc[1] - r.cy) * d[1];
      const pd: Pt = [r.cx + d[0] * t + n[0] * off, r.cy + d[1] * t + n[1] * off];
      const c: Pt = [this.contentW / 2 + pd[0] * this.zoom, PAD_TOP + pd[1] * this.zoom];
      const pp = this.contentToPage(page, c);
      return { x: pp[0], y: pp[1], p: q.p };
    };
    return [proj(pts[0]), proj(pts[pts.length - 1])];
  }

  private dragShape(a: Pt, b: Pt): Shape {
    const p = this.prefs();
    return { kind: p.shape_kind, points: [a, b], color: p.shape_color, width: mm(p.shape_mm), fill: p.shape_fill ? p.shape_color : null, dashed: p.shape_dashed, solid: false, dotted: false };
  }

  private tapeFrom(pts: Pt[]) {
    const p = this.prefs();
    const points = p.tape_kind === "Free" ? rdp(pts, 0.8) : [pts[0], pts[pts.length - 1]];
    return { points, width: mm(p.tape_mm), color: p.tape_color, rect: p.tape_kind === "Rect", revealed: false };
  }

  private eraseAt(page: number, pt: Pt) {
    const prefs = this.prefs();
    const m = this.model;
    const pg = m.page(page);
    const r = prefs.eraser_size / this.zoom;
    const eligible = (it: Item) => {
      if (!editable(pg, it)) return false;
      const k = kindOf(it);
      if (prefs.eraser_hl_only) return k === "highlight";
      if (k === "shape") return prefs.eraser_shapes;
      if (k === "tape") return prefs.eraser_tapes;
      return k === "ink" || k === "highlight";
    };
    const hits = new Set(pg.items.filter((it) => eligible(it) && hitItem(it, pt[0], pt[1], r)).map((it) => it.id));
    if (!hits.size) return;
    const a = this.action;
    if (a.k === "erase" && !a.before) a.before = pg;
    const out: Item[] = [];
    for (const it of pg.items) {
      if (!hits.has(it.id)) { out.push(it); continue; }
      if (prefs.eraser_precise && "Stroke" in it.content)
        for (const part of splitStroke(it.content.Stroke, pt, r)) out.push({ ...newItem({ Stroke: part }), layer: it.layer });
    }
    m.replaceItems(page, out, false);
  }

  setZoomTarget(t: Board["zoomTarget"]) {
    this.zoomTarget = t;
    this.overlayDirty = true;
    this.kick();
  }

  setTool(t: Tool) {
    if (t === "eraser" && this.tool !== "eraser") this.prevTool = this.tool;
    this.tool = t;
    if (t !== "lasso" && this.model.selection) { this.model.selection = null; this.model.notify(); }
  }

  toggleRuler() {
    if (this.ruler) this.ruler = null;
    else {
      const d = this.contentToDoc([this.scroller.scrollLeft + this.viewW / 2, this.scroller.scrollTop + this.viewH / 2]);
      this.ruler = { cx: d[0], cy: d[1], angle: 0 };
    }
    this.overlayDirty = true;
    this.kick();
  }
  rotateRuler(deg: number) {
    if (this.ruler) { this.ruler.angle += (deg * Math.PI) / 180; this.overlayDirty = true; this.kick(); }
  }

  /** Centre of the visible part of page i, in page units. */
  visibleCenter(i: number): Pt {
    const p = this.model.page(i);
    const c = this.contentToPage(i, [this.scroller.scrollLeft + this.viewW / 2, this.scroller.scrollTop + this.viewH / 2]);
    return [Math.max(40, Math.min(p.w - 40, c[0])), Math.max(40, Math.min(p.h - 40, c[1]))];
  }

  /** Selection box in view coordinates (for the floating selection menu). */
  selectionViewRect(): Rect | null {
    const sel = this.model.selection;
    const b = this.model.selectionBox();
    if (!sel || !b || this.action.k !== "idle") return null;
    const a = this.pageToView(sel.page, [b.x, b.y]);
    return { x: a[0], y: a[1], w: b.w * this.zoom, h: b.h * this.zoom };
  }

  get busy() { return this.action.k !== "idle"; }
}

function rectPoly(a: Pt, b: Pt): Pt[] {
  return [a, [b[0], a[1]], b, [a[0], b[1]]];
}

function drawColor(st: Stroke, paper: Paper): string {
  const c = st.color;
  if (isDark(paper) && (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]) / 255 < 0.3)
    return `rgb(${255 - (c[0] >> 1)},${255 - (c[1] >> 1)},${255 - (c[2] >> 1)})`;
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}

/**
 * Removes the part of a stroke under the eraser circle. Works on the stroke as it is
 * drawn (smoothed samples with their real widths), cuts exactly where the ink meets
 * the circle, and stores the pieces with explicit widths so they look exactly the
 * same as before: no re-tapering, no jumps in thickness.
 */
function splitStroke(s: Stroke, c: Pt, r: number): Stroke[] {
  const ss = samples(s);
  // Ink at a sample is erased when the eraser touches it (centre within r + w/2).
  const reach = (w: number) => r + w / 2;
  const inside = (q: { x: number; y: number; w: number }) => Math.hypot(q.x - c[0], q.y - c[1]) < reach(q.w);
  // Point where the segment a→b crosses the reach circle.
  const cross = (a: typeof ss[0], b: typeof ss[0]) => {
    const R = reach((a.w + b.w) / 2);
    const dx = b.x - a.x, dy = b.y - a.y, fx = a.x - c[0], fy = a.y - c[1];
    const A = dx * dx + dy * dy, B = 2 * (fx * dx + fy * dy), C = fx * fx + fy * fy - R * R;
    const disc = Math.max(0, B * B - 4 * A * C);
    const roots = [(-B - Math.sqrt(disc)) / (2 * A || 1), (-B + Math.sqrt(disc)) / (2 * A || 1)].filter((t) => t >= 0 && t <= 1);
    const t = inside(a) ? Math.max(...roots, 0) : Math.min(...roots, 1);
    return { x: a.x + dx * t, y: a.y + dy * t, w: a.w + (b.w - a.w) * t };
  };
  const pieces: (typeof ss)[] = [];
  let cur: typeof ss = [];
  for (let i = 0; i < ss.length; i++) {
    const q = ss[i];
    const prev = ss[i - 1];
    if (inside(q)) {
      if (prev && !inside(prev)) cur.push(cross(prev, q));
      if (cur.length > 1) pieces.push(cur);
      cur = [];
    } else {
      if (prev && inside(prev)) cur.push(cross(prev, q));
      cur.push(q);
    }
  }
  if (cur.length > 1) pieces.push(cur);
  // Drop crumbs too small to see.
  return pieces
    .filter((p) => p.reduce((len, q, k) => len + (k ? Math.hypot(q.x - p[k - 1].x, q.y - p[k - 1].y) : 0), 0) > 0.6)
    .map((p) => ({ ...s, points: p.map((q) => ({ x: q.x, y: q.y, p: 0.5 })), widths: p.map((q) => q.w) }));
}
