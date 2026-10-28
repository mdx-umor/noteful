// Handwriting recognition with Tesseract.js (runs in the browser, loaded on first use).
// It is tuned for printed text, so neat handwriting works best.

import type { Item, Page } from "./model";
import { kindOf } from "./model";
import { drawItem } from "./render";
import { unionBBox } from "./geom";

const CDN = "https://cdn.jsdelivr.net/npm/tesseract.js@5.1.1/dist/";
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Word = { text: string; confidence: number };
type Line = { words: Word[] };
type Worker = { recognize: (img: HTMLCanvasElement) => Promise<{ data: { text: string; lines?: Line[] } }>; terminate: () => Promise<void> };
let worker: Promise<Worker> | null = null;

function getWorker(): Promise<Worker> {
  if (!worker) {
    worker = (async () => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const w = window as any;
      if (!w.Tesseract) {
        await new Promise<void>((resolve, reject) => {
          const s = document.createElement("script");
          s.src = CDN + "tesseract.min.js";
          s.onload = () => resolve();
          s.onerror = () => reject(new Error("Could not load handwriting recognition. Are you online?"));
          document.head.appendChild(s);
        });
      }
      return w.Tesseract.createWorker("eng", 1, { workerPath: CDN + "worker.min.js" });
    })();
    worker.catch(() => (worker = null));
  }
  return worker;
}

const isInk = (it: Item) => kindOf(it) === "ink" || kindOf(it) === "shape";

/** Renders ink items large, black on white, which is what the recogniser likes. */
function renderForOcr(items: Item[]): HTMLCanvasElement | null {
  const ink = items.filter(isInk);
  const b = unionBBox(ink);
  if (!b || b.w < 4 || b.h < 4) return null;
  const pad = 20, scale = Math.min(3, 2400 / Math.max(b.w, b.h));
  const c = document.createElement("canvas");
  c.width = Math.round(b.w * scale + pad * 2);
  c.height = Math.round(b.h * scale + pad * 2);
  const ctx = c.getContext("2d")!;
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, c.width, c.height);
  ctx.setTransform(scale, 0, 0, scale, pad - b.x * scale, pad - b.y * scale);
  for (const it of ink) {
    const black: Item = "Stroke" in it.content
      ? { ...it, content: { Stroke: { ...it.content.Stroke, color: [0, 0, 0, 255], pen: "Fineliner", width: Math.max(1.6, it.content.Stroke.width) } } }
      : it;
    drawItem(ctx, black, { paper: "White" });
  }
  return c;
}

/** Recognised text. `minConfidence` drops words the recogniser is unsure about (doodles, shapes). */
export async function recognizeItems(items: Item[], minConfidence = 0): Promise<string> {
  const c = renderForOcr(items);
  if (!c) return "";
  const w = await getWorker();
  const { data } = await w.recognize(c);
  if (minConfidence > 0 && data.lines)
    return data.lines
      .map((l) => l.words.filter((x) => x.confidence >= minConfidence && /[\p{L}\p{N}]/u.test(x.text)).map((x) => x.text).join(" "))
      .filter(Boolean).join("\n");
  return data.text.replace(/\n{3,}/g, "\n\n").trim();
}

/** Identifies a page's handwriting, so it is only recognised again when it changes. */
export function inkKey(p: Page): string {
  const ink = p.items.filter(isInk);
  return ink.length ? `${ink.length}:${ink[ink.length - 1].id}:${ink[0].id}` : "";
}

export const needsOcr = (p: Page) => { const k = inkKey(p); return k !== "" && p.ocr?.key !== k; };
