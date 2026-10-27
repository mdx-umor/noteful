// Pictures: decoding stored JPEGs, importing photos and PDFs (via pdf.js).

import type { Id } from "./model";
import { newId } from "./model";
import { loadAsset, saveAsset } from "./storage";

const cache = new Map<Id, ImageBitmap | null>();
const pending = new Set<Id>();
const listeners = new Set<() => void>();
const order: Id[] = [];

/** Called when a picture finishes loading (pages redraw). */
export function onPictureLoaded(f: () => void) {
  listeners.add(f);
  return () => { listeners.delete(f); };
}

/** Decoded picture, or undefined while it loads (a redraw follows). */
export function picture(id: Id): ImageBitmap | undefined {
  const hit = cache.get(id);
  if (hit) return hit;
  if (hit === null || pending.has(id)) return undefined;
  const bytes = loadAsset(id);
  if (!bytes) { cache.set(id, null); return undefined; }
  pending.add(id);
  createImageBitmap(new Blob([bytes], { type: "image/jpeg" }))
    .then((bmp) => {
      cache.set(id, bmp);
      order.push(id);
      // Keep memory bounded: big PDFs have many pages.
      while (order.length > 24) { const old = order.shift()!; cache.get(old)?.close(); cache.delete(old); }
      listeners.forEach((f) => f());
    })
    .catch(() => cache.set(id, null))
    .finally(() => pending.delete(id));
  return undefined;
}

export function pictureReady(id: Id): Promise<void> {
  if (cache.get(id)) return Promise.resolve();
  picture(id);
  return new Promise((resolve) => {
    const t = setInterval(() => { if (cache.has(id) && !pending.has(id)) { clearInterval(t); resolve(); } }, 30);
  });
}

export interface ImportedPage { w: number; h: number; asset: Id }
export interface Imported { kind: "image" | "pdf"; name: string; pages: ImportedPage[] }

export function pickFile(accept: string): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accept;
    input.style.cssText = "position:fixed;left:-1000px;top:0;opacity:0";
    document.body.appendChild(input);
    input.addEventListener("change", () => { resolve(input.files?.[0] ?? null); input.remove(); });
    input.addEventListener("cancel", () => { resolve(null); input.remove(); });
    input.click();
  });
}

const toJpeg = (c: HTMLCanvasElement, q: number) =>
  new Promise<Uint8Array>((resolve, reject) =>
    c.toBlob((b) => (b ? b.arrayBuffer().then((a) => resolve(new Uint8Array(a))) : reject(new Error("Could not encode picture"))), "image/jpeg", q));

function store(bytes: Uint8Array): Id {
  const id = newId();
  saveAsset(id, bytes);
  return id;
}

async function fromImage(file: File): Promise<Imported> {
  const bmp = await createImageBitmap(file);
  const s = Math.min(1, 1600 / Math.max(bmp.width, bmp.height));
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.round(bmp.width * s));
  c.height = Math.max(1, Math.round(bmp.height * s));
  const ctx = c.getContext("2d")!;
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, c.width, c.height);
  ctx.drawImage(bmp, 0, 0, c.width, c.height);
  bmp.close();
  return { kind: "image", name: file.name, pages: [{ w: c.width, h: c.height, asset: store(await toJpeg(c, 0.85)) }] };
}

const PDFJS = "https://cdnjs.cloudflare.com/ajax/libs/pdf.js/3.11.174/";
// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function pdfLib(): Promise<any> {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const w = window as any;
  if (!w.pdfjsLib) {
    await new Promise<void>((resolve, reject) => {
      const s = document.createElement("script");
      s.src = PDFJS + "pdf.min.js";
      s.onload = () => resolve();
      s.onerror = () => reject(new Error("Could not load the PDF reader. Are you online?"));
      document.head.appendChild(s);
    });
    w.pdfjsLib.GlobalWorkerOptions.workerSrc = PDFJS + "pdf.worker.min.js";
  }
  return w.pdfjsLib;
}

async function fromPdf(file: File, progress: (msg: string) => void): Promise<Imported> {
  const lib = await pdfLib();
  const doc = await lib.getDocument({ data: new Uint8Array(await file.arrayBuffer()) }).promise;
  const pages: ImportedPage[] = [];
  for (let i = 1; i <= doc.numPages; i++) {
    progress(`Importing ${file.name}: page ${i} of ${doc.numPages}`);
    const page = await doc.getPage(i);
    const base = page.getViewport({ scale: 1 });
    const vp = page.getViewport({ scale: Math.min(2.5, 1600 / base.width) });
    const c = document.createElement("canvas");
    c.width = Math.round(vp.width);
    c.height = Math.round(vp.height);
    const ctx = c.getContext("2d")!;
    ctx.fillStyle = "#fff";
    ctx.fillRect(0, 0, c.width, c.height);
    await page.render({ canvasContext: ctx, viewport: vp }).promise;
    pages.push({ w: base.width, h: base.height, asset: store(await toJpeg(c, 0.82)) });
    page.cleanup();
  }
  doc.destroy();
  return { kind: "pdf", name: file.name, pages };
}

export async function importFile(file: File, progress: (msg: string) => void): Promise<Imported> {
  const isPdf = file.type === "application/pdf" || /\.pdf$/i.test(file.name);
  return isPdf ? fromPdf(file, progress) : fromImage(file);
}

/** Width, height and colour components of a JPEG (for PDF export). */
export function jpegInfo(b: Uint8Array): { w: number; h: number; comps: number } | null {
  let i = 2;
  while (i + 9 < b.length) {
    if (b[i] !== 0xff) { i++; continue; }
    const m = b[i + 1];
    const len = (b[i + 2] << 8) | b[i + 3];
    if (m >= 0xc0 && m <= 0xcf && m !== 0xc4 && m !== 0xc8 && m !== 0xcc)
      return { h: (b[i + 5] << 8) | b[i + 6], w: (b[i + 7] << 8) | b[i + 8], comps: b[i + 9] };
    i += 2 + len;
  }
  return null;
}
