import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { Page, Paper, Rgba, Template } from "../core/model";
import { css, isDark, sameColor, TEMPLATE_NAMES, TEMPLATES } from "../core/model";
import { drawPage, templatePreview } from "../core/render";
import { onPictureLoaded, picture } from "../core/pictures";

export type Anchor = { x: number; y: number; w: number; h: number };
export const rectOf = (el: Element): Anchor => { const r = el.getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; };

/** Floating panel next to an anchor; closes on outside tap or Escape. */
export function Popover({ anchor, side = "right", onClose, children, width = 320 }: {
  anchor: Anchor; side?: "right" | "below" | "above" | "left"; onClose: () => void; children: ReactNode; width?: number;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>({ left: -9999, top: -9999 });
  useLayoutEffect(() => {
    const el = ref.current!;
    const w = el.offsetWidth, h = el.offsetHeight, vw = innerWidth, vh = innerHeight;
    let left = 0, top = 0;
    if (side === "right") { left = anchor.x + anchor.w + 10; top = anchor.y - 8; }
    else if (side === "left") { left = anchor.x - w - 10; top = anchor.y - 8; }
    else if (side === "below") { left = anchor.x + anchor.w - w; top = anchor.y + anchor.h + 6; }
    else { left = anchor.x; top = anchor.y - h - 8; }
    left = Math.max(8, Math.min(vw - w - 8, left));
    top = Math.max(8, Math.min(vh - h - 8, top));
    setPos({ left, top });
  }, [anchor.x, anchor.y, anchor.w, anchor.h, side, children]);
  useEffect(() => {
    const k = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    addEventListener("keydown", k);
    return () => removeEventListener("keydown", k);
  }, [onClose]);
  return (
    <>
      <div className="scrim" onPointerDown={(e) => { e.preventDefault(); onClose(); }} />
      <div ref={ref} className="popover" style={{ ...pos, width }}>{children}</div>
    </>
  );
}

export function Modal({ onClose, children, wide }: { onClose: () => void; children: ReactNode; wide?: boolean }) {
  useEffect(() => {
    const k = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    addEventListener("keydown", k);
    return () => removeEventListener("keydown", k);
  }, [onClose]);
  return (
    <div className="scrim dim" onPointerDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={"modal" + (wide ? " wide" : "")}>{children}</div>
    </div>
  );
}

export function Toggle({ label, value, onChange }: { label: ReactNode; value: boolean; onChange: (v: boolean) => void }) {
  return (
    <button className="toggle-row" style={{ width: "100%" }} onClick={() => onChange(!value)}>
      <span>{label}</span>
      <span className={"switch" + (value ? " on" : "")} />
    </button>
  );
}

export function Seg<T extends string>({ value, options, onChange }: { value: T; options: [T, ReactNode][]; onChange: (v: T) => void }) {
  return (
    <div className="seg">
      {options.map(([v, label]) => (
        <button key={v} className={v === value ? "on" : ""} onClick={() => onChange(v)}>{label}</button>
      ))}
    </div>
  );
}

export function Swatches({ colors, value, onPick, onRemove }: { colors: Rgba[]; value?: Rgba; onPick: (c: Rgba) => void; onRemove?: (i: number) => void }) {
  return (
    <div className="swatches">
      {colors.map((c, i) => (
        <button key={i} className={"swatch" + (value && sameColor(c, value) ? " on" : "")} style={{ background: css(c) }}
          onClick={() => onPick(c)}
          onContextMenu={(e) => { e.preventDefault(); if (onRemove && colors.length > 1) onRemove(i); }}
          title={onRemove ? "Right-click / long-press to remove" : undefined} />
      ))}
    </div>
  );
}

export function SizeRow({ sizes, value, onPick }: { sizes: number[]; value: number; onPick: (v: number) => void }) {
  return (
    <div className="sizes">
      {sizes.map((s) => {
        const d = Math.max(3, Math.min(18, s * 3.4));
        return (
          <button key={s} className={"size" + (Math.abs(s - value) < 0.01 ? " on" : "")} onClick={() => onPick(s)}>
            <i style={{ width: d, height: d }} />
            {s}mm
          </button>
        );
      })}
    </div>
  );
}

/** A canvas that renders a page thumbnail (redraws when the page object changes). */
export function PageThumb({ page, width, night = false }: { page: Page; width: number; night?: boolean }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => onPictureLoaded(() => setTick((t) => t + 1)), []);
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const dpr = Math.min(2, devicePixelRatio || 1);
    const s = (width * dpr) / page.w;
    c.width = Math.round(page.w * s);
    c.height = Math.round(page.h * s);
    const ctx = c.getContext("2d")!;
    ctx.setTransform(s, 0, 0, s, 0, 0);
    const paper: Paper = night && !isDark(page.paper) ? "Night" : page.paper;
    drawPage(ctx, page, { paper, image: picture });
  }, [page, width, night, tick]);
  return <canvas ref={ref} style={{ width, aspectRatio: `${page.w} / ${page.h}` }} />;
}

function TemplateTile({ t, paper, on, onPick }: { t: Template; paper: Paper; on: boolean; onPick: () => void }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const src = templatePreview(t, paper, 150);
    const c = ref.current!;
    c.width = src.width;
    c.height = src.height;
    c.getContext("2d")!.drawImage(src, 0, 0);
  }, [t, paper]);
  return (
    <button className={"template" + (on ? " on" : "")} onClick={onPick}>
      <canvas ref={ref} />
      {TEMPLATE_NAMES[t]}
    </button>
  );
}

export function TemplatePicker({ value, paper, onPick }: { value: Template; paper: Paper; onPick: (t: Template) => void }) {
  return (
    <div className="template-grid">
      {TEMPLATES.map((t) => <TemplateTile key={t} t={t} paper={paper} on={t === value} onPick={() => onPick(t)} />)}
    </div>
  );
}

export function PaperSwatches({ value, onPick, papers, bg }: { value: Paper; onPick: (p: Paper) => void; papers: Paper[]; bg: (p: Paper) => Rgba }) {
  return (
    <div className="swatches">
      {papers.map((p) => (
        <button key={p} title={p} className={"swatch" + (p === value ? " on" : "")} style={{ background: css(bg(p)) }} onClick={() => onPick(p)} />
      ))}
    </div>
  );
}

export function timeAgo(ts: number): string {
  const s = Math.max(0, (Date.now() - ts) / 1000);
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  if (s < 2592000) return `${Math.floor(s / 86400)}d`;
  return `${Math.floor(s / 604800)}w`;
}
