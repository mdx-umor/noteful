import { useCallback, useEffect, useRef, useState } from "react";
import type { InkPoint, Item, Stroke } from "../core/model";
import { editable, isDark, mm, newItem } from "../core/model";
import { drawItem, drawPage } from "../core/render";
import { hitItem } from "../core/geom";
import { onPictureLoaded, picture } from "../core/pictures";
import type { Prefs } from "../prefs";
import { Icon } from "../ui/icons";
import type { Board, Tool } from "./board";
import type { DocModel } from "./doc";

type Target = { page: number; x: number; y: number; w: number; h: number };

/**
 * Docked magnifier: write large in the strip, the ink lands small inside the blue box
 * on the page. Lifting the pen near the right edge moves the box along the line.
 */
export function ZoomWindow({ model, board, prefs, tool, onClose }: {
  model: DocModel; board: Board; prefs: Prefs; tool: Tool; onClose: () => void;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const [mag, setMag] = useState(3);
  const [size, setSize] = useState({ w: 600, h: 200 });
  const [target, setTargetState] = useState<Target>(() => {
    const p = model.page(model.current);
    const c = board.visibleCenter(model.current);
    const w = Math.min(p.w * 0.9, 600 / 3);
    return { page: model.current, x: Math.max(20, c[0] - w / 2), y: c[1] - 20, w, h: 200 / 3 };
  });
  const live = useRef<{ pts: InkPoint[]; id: number; erase: boolean } | null>(null);
  const [, tick] = useState(0);

  // Keep the box sized to the strip and the magnification.
  const setTarget = useCallback((t: Target) => {
    const p = model.page(t.page);
    const w = Math.min(p.w, size.w / mag), h = size.h / mag;
    const fixed = { ...t, w, h, x: Math.max(0, Math.min(p.w - w, t.x)), y: Math.max(0, Math.min(p.h - h, t.y)) };
    setTargetState(fixed);
    board.setZoomTarget(fixed);
  }, [board, model, mag, size]);

  useEffect(() => {
    const ro = new ResizeObserver(() => {
      const r = wrap.current!.getBoundingClientRect();
      setSize({ w: r.width, h: r.height });
    });
    ro.observe(wrap.current!);
    return () => { ro.disconnect(); board.setZoomTarget(null); };
  }, [board]);
  useEffect(() => { setTarget(target); }, [mag, size]); // eslint-disable-line react-hooks/exhaustive-deps

  // Redraw on document changes.
  useEffect(() => {
    const a = model.subscribe(() => tick((x) => x + 1));
    const b = onPictureLoaded(() => tick((x) => x + 1));
    return () => { a(); b(); };
  }, [model]);

  const paper = (() => { const p = model.page(target.page); return prefs.night_mode && !isDark(p.paper) ? "Night" as const : p.paper; })();

  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const dpr = devicePixelRatio || 1;
    c.width = Math.round(size.w * dpr);
    c.height = Math.round(size.h * dpr);
    const ctx = c.getContext("2d")!;
    const k = (size.w / target.w) * dpr;
    ctx.setTransform(k, 0, 0, k, -target.x * k, -target.y * k);
    const page = model.page(target.page);
    if (!page) return;
    drawPage(ctx, page, { paper, image: picture });
    const l = live.current;
    if (l && !l.erase && l.pts.length) drawItem(ctx, newItem({ Stroke: stroke(l.pts) }), { paper });
    // Shaded right margin: lifting the pen here advances the box.
    ctx.fillStyle = "rgba(10,132,255,0.08)";
    ctx.fillRect(target.x + target.w * 0.82, target.y, target.w * 0.18, target.h);
  });

  const hl = tool === "highlighter";
  function stroke(pts: InkPoint[]): Stroke {
    return hl
      ? { pen: "Highlighter", color: prefs.hl_color, width: mm(prefs.hl_mm), points: pts }
      : { pen: prefs.pen_kind, color: prefs.pen_color, width: mm(prefs.pen_mm), points: pts };
  }

  const toPage = (e: React.PointerEvent): [number, number] => {
    const r = canvas.current!.getBoundingClientRect();
    return [target.x + ((e.clientX - r.left) / r.width) * target.w, target.y + ((e.clientY - r.top) / r.height) * target.h];
  };
  const pressure = (e: React.PointerEvent) => (e.pointerType === "pen" && e.pressure > 0 ? Math.min(1, e.pressure * 1.3 * prefs.pressure_gain) : 0.5);

  const eraseAt = (p: [number, number]) => {
    const pg = model.page(target.page);
    const r = prefs.eraser_size / board.zoom / mag;
    const keep = pg.items.filter((it: Item) => !(editable(pg, it) && ("Stroke" in it.content) && hitItem(it, p[0], p[1], r)));
    if (keep.length !== pg.items.length) model.replaceItems(target.page, keep);
  };

  const down = (e: React.PointerEvent) => {
    if (e.pointerType === "touch" && prefs.pencil_only) return;
    if (!model.canWrite(target.page)) return model.toast("The active layer is locked or hidden");
    e.preventDefault();
    (e.target as Element).setPointerCapture(e.pointerId);
    const p = toPage(e);
    const erase = tool === "eraser";
    live.current = { pts: [{ x: p[0], y: p[1], p: pressure(e) }], id: e.pointerId, erase };
    if (erase) eraseAt(p);
  };
  const move = (e: React.PointerEvent) => {
    const l = live.current;
    if (!l || l.id !== e.pointerId) return;
    const evs = (e.nativeEvent as PointerEvent).getCoalescedEvents?.() ?? [];
    for (const ev of evs.length ? evs : [e.nativeEvent]) {
      const r = canvas.current!.getBoundingClientRect();
      const p: [number, number] = [target.x + ((ev.clientX - r.left) / r.width) * target.w, target.y + ((ev.clientY - r.top) / r.height) * target.h];
      if (l.erase) eraseAt(p);
      else l.pts.push({ x: p[0], y: p[1], p: ev.pointerType === "pen" && ev.pressure > 0 ? Math.min(1, ev.pressure * 1.3 * prefs.pressure_gain) : 0.5 });
    }
    tick((x) => x + 1);
  };
  const up = (e: React.PointerEvent) => {
    const l = live.current;
    if (!l || l.id !== e.pointerId) return;
    live.current = null;
    if (!l.erase && l.pts.length) {
      model.addItem(target.page, newItem({ Stroke: stroke(l.pts) }));
      // Auto-advance when the pen lifts in the shaded margin.
      const last = l.pts[l.pts.length - 1];
      if (last.x > target.x + target.w * 0.82) advance();
    }
    tick((x) => x + 1);
  };

  const advance = () => {
    const p = model.page(target.page);
    const step = target.w * 0.65;
    if (target.x + target.w + 4 < p.w - 20) setTarget({ ...target, x: target.x + step });
    else newLine();
  };
  const newLine = () => {
    const p = model.page(target.page);
    const ny = target.y + target.h * 0.9;
    if (ny + target.h > p.h && target.page + 1 < model.pages.length) setTarget({ ...target, page: target.page + 1, x: 40, y: 60 });
    else setTarget({ ...target, x: p.template === "Lined" ? 60 : 30, y: ny });
  };

  return (
    <div className="zoom-window">
      <div className="row" style={{ padding: "4px 8px", gap: 4 }}>
        <span className="small muted grow">Zoom window · write here, ink goes in the blue box (page {target.page + 1})</span>
        <button className="icon-btn" title="Left" onClick={() => setTarget({ ...target, x: target.x - target.w * 0.5 })}><Icon name="back" size={18} /></button>
        <button className="icon-btn" title="Right" onClick={() => setTarget({ ...target, x: target.x + target.w * 0.5 })}><Icon name="chevronRight" size={18} /></button>
        <button className="icon-btn" title="Up" onClick={() => setTarget({ ...target, y: target.y - target.h * 0.5 })}><span style={{ transform: "rotate(180deg)", display: "grid" }}><Icon name="chevron" size={18} /></span></button>
        <button className="icon-btn" title="Down" onClick={() => setTarget({ ...target, y: target.y + target.h * 0.5 })}><Icon name="chevron" size={18} /></button>
        <button className="btn" title="Next line" onClick={newLine}>↵ Line</button>
        <button className="btn" title="Bring the box to the middle of the screen" onClick={() => { const c = board.visibleCenter(model.current); setTarget({ ...target, page: model.current, x: c[0] - target.w / 2, y: c[1] - target.h / 2 }); }}>Here</button>
        <select value={mag} onChange={(e) => setMag(Number(e.target.value))} style={{ background: "var(--raised)", color: "var(--text)", border: 0, borderRadius: 8, padding: "6px" }}>
          {[2, 2.5, 3, 4, 5].map((m) => <option key={m} value={m}>{m}×</option>)}
        </select>
        <button className="icon-btn" title="Close" onClick={onClose}><Icon name="close" size={18} /></button>
      </div>
      <div ref={wrap} className="zoom-canvas">
        <canvas ref={canvas} style={{ width: "100%", height: "100%", touchAction: "none" }}
          onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={() => (live.current = null)} />
      </div>
    </div>
  );
}
