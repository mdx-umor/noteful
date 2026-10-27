// Library-level operations: create, open, save, delete, thumbnails, import, backup.

import type { Doc, Id, Library, NotebookMeta, Paper, Rgba, Template } from "./core/model";
import { assetIds, fromHex, newDoc, newId, newItem, newPage, searchText, A4_W } from "./core/model";
import * as storage from "./core/storage";
import { drawPage } from "./core/render";
import { picture, pictureReady, type Imported } from "./core/pictures";
import { exportPdf, download } from "./core/pdf";

export const COVERS = ["#5b5bd6", "#0a84ff", "#30b0c7", "#34c759", "#ffcc00", "#ff9500", "#ff3b30", "#af52de"];

export function makeMeta(doc: Doc, title: string, cover: Rgba, folder: Id | null, tags: string[]): NotebookMeta {
  const t = Date.now();
  return {
    id: doc.id, title, cover, folder, tags, pinned: false, trashed_at: null, created: t, updated: t, opened: t,
    page_count: doc.pages.length, search_text: searchText(doc), template: doc.pages[0].template, paper: doc.pages[0].paper,
  };
}

/** Renders the first page as a small PNG for the library card. */
export async function saveThumbnail(doc: Doc) {
  const page = doc.pages[0];
  const ids = [page.background, ...page.items.flatMap((i) => ("Image" in i.content ? [i.content.Image.asset] : []))].filter(Boolean) as Id[];
  await Promise.race([Promise.all(ids.map(pictureReady)), new Promise((r) => setTimeout(r, 3000))]);
  const w = 320, s = w / page.w;
  const c = document.createElement("canvas");
  c.width = w;
  c.height = Math.round(page.h * s);
  const ctx = c.getContext("2d")!;
  ctx.scale(s, s);
  drawPage(ctx, page, { paper: page.paper, image: picture });
  storage.saveThumb(doc.id, c.toDataURL("image/png").split(",")[1]);
  dispatchEvent(new Event("nb-thumb"));
}

export function createNotebook(lib: Library, doc: Doc, title: string, cover: Rgba, folder: Id | null, tags: string[]): Library {
  storage.saveDoc(doc);
  void saveThumbnail(doc);
  const next = { ...lib, notebooks: [...lib.notebooks, makeMeta(doc, title, cover, folder, tags)] };
  storage.saveLibrary(next);
  return next;
}

export function updateMeta(lib: Library, id: Id, f: (m: NotebookMeta) => NotebookMeta): Library {
  const next = { ...lib, notebooks: lib.notebooks.map((m) => (m.id === id ? f(m) : m)) };
  storage.saveLibrary(next);
  return next;
}

export function deleteForever(lib: Library, ids: Id[]): Library {
  const assets = new Set<Id>();
  for (const id of ids) {
    const d = storage.loadDoc(id);
    if (d) assetIds(d).forEach((a) => assets.add(a));
    storage.deleteDoc(id);
    storage.deleteThumb(id);
  }
  const next = { ...lib, notebooks: lib.notebooks.filter((m) => !ids.includes(m.id)) };
  // Pictures can be shared by duplicated notebooks: only delete unused ones.
  if (assets.size) {
    for (const m of next.notebooks) { const d = storage.loadDoc(m.id); if (d) assetIds(d).forEach((a) => assets.delete(a)); }
    assets.forEach(storage.deleteAsset);
  }
  storage.saveLibrary(next);
  return next;
}

export function purgeOldTrash(lib: Library): Library {
  const cutoff = Date.now() - 30 * 86400000;
  const old = lib.notebooks.filter((m) => m.trashed_at != null && m.trashed_at < cutoff).map((m) => m.id);
  return old.length ? deleteForever(lib, old) : lib;
}

export function duplicateNotebook(lib: Library, id: Id): Library {
  const d = storage.loadDoc(id);
  const m = lib.notebooks.find((x) => x.id === id);
  if (!d || !m) return lib;
  const copy: Doc = { ...d, id: newId(), pages: d.pages.map((p) => ({ ...p, id: newId() })) };
  return createNotebook(lib, copy, `${m.title} copy`, m.cover, m.folder ?? null, m.tags);
}

export function notebookFromImport(imp: Imported): Doc {
  return {
    id: newId(),
    outline: [],
    pages: imp.pages.map((pg) => {
      const [w, h] = imp.kind === "pdf" ? [pg.w, pg.h] : [A4_W, (A4_W * pg.h) / pg.w];
      return { ...newPage("Blank", "White", w, h), background: pg.asset };
    }),
  };
}

export function exportNotebookPdf(id: Id, title: string, doc?: Doc) {
  const d = doc ?? storage.loadDoc(id);
  if (d) download(`${title}.pdf`, exportPdf(d, title), "application/pdf");
}

export function today(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getDate())}/${p(d.getMonth() + 1)}/${String(d.getFullYear()).slice(2)}`;
}

// ---------- backup ----------

export function downloadBackup() {
  const files = Object.fromEntries(storage.allFiles());
  const blob = new Blob([JSON.stringify({ app: "notebook", version: 2, created: Date.now(), files })], { type: "application/json" });
  const d = new Date();
  download(`Notebook backup ${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}.json`, blob, "application/json");
}

/** Restores a backup file. Existing notebooks are kept; ones in the backup are added or replaced. */
export async function restoreBackup(file: File): Promise<number> {
  const data = JSON.parse(await file.text());
  if (!data || data.app !== "notebook" || typeof data.files !== "object") throw new Error("This is not a Notebook backup file");
  const current = storage.loadLibrary();
  const files = data.files as Record<string, string>;
  let restored = 0;
  for (const [name, value] of Object.entries(files)) {
    if (name === "library.json" || name === "prefs.json") continue;
    storage.restoreFile(name, value);
  }
  const libText = files["library.json"];
  if (libText) {
    const backupLib = JSON.parse(libText.startsWith("t:") ? libText.slice(2) : libText) as Library;
    const byId = new Map(current.notebooks.map((m) => [m.id, m]));
    for (const m of backupLib.notebooks) { byId.set(m.id, m); restored++; }
    const folders = new Map(current.folders.map((f) => [f.id, f]));
    for (const f of backupLib.folders ?? []) folders.set(f.id, f);
    storage.saveLibrary({ notebooks: [...byId.values()], folders: [...folders.values()] });
  }
  return restored;
}

// ---------- welcome notebook ----------

export function welcomeDoc(): Doc {
  const doc = newDoc("Lined", "Cream");
  const ink: Rgba = [0x1d, 0x1d, 0x1f, 255], blue = fromHex("#1e5bd8");
  const text = (y: number, s: string, size: number, color: Rgba = ink) => newItem({ Text: { x: 70, y, w: 470, text: s, size, color } });
  const items = [
    newItem({ Stroke: { pen: "Highlighter", color: fromHex("#ffd60a"), width: 17, points: Array.from({ length: 21 }, (_, i) => ({ x: 68 + i * 14, y: 58, p: 0.5 })) } }),
    text(40, "Welcome to Notebook", 28),
    text(92, "Write with Apple Pencil. Fingers scroll, pinch to zoom, tap with two fingers to undo and three to redo.", 14),
    text(160, "Pens: fineliner, fountain (thick and thin like a real nib) and brush. Tap the active tool again for sizes, colours and settings.", 14),
    text(240, "Draw a circle, box, triangle or line and keep the Pencil still for a moment: it snaps into a clean shape.", 14),
    text(310, "Lasso to move, resize, recolour, copy or delete. Layers, stickers, shapes, links, outline, PDF import and more are in the toolbar and the ⋯ menu.", 14),
    newItem({ Stroke: { pen: "Fountain", color: blue, width: 2.8, points: Array.from({ length: 81 }, (_, i) => { const t = i / 80; return { x: 70 + t * 300, y: 400 + Math.sin(t * 14) * 6, p: 0.3 + 0.6 * Math.sin(t * Math.PI) }; }) } }),
    newItem({ Shape: { kind: "Ellipse", points: [[400, 370], [530, 440]], color: [217, 48, 37, 255], width: 2, fill: null } }),
    text(470, "Study with tape. Q: What is the capital of France?", 14),
    text(500, "A: Paris", 14, blue),
    newItem({ Tape: { points: [[66, 510], [150, 510]], width: 22, color: fromHex("#f6b9a4"), rect: false, revealed: false } }),
    text(540, "Tap the tape to reveal the answer.", 12),
  ];
  doc.pages[0].items = items;
  doc.pages.push(newPage("Grid", "White"), newPage("Dotted", "Night"));
  return doc;
}

export const defaultTemplate = (t: Template, p: Paper) => ({ t, p });
