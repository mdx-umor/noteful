// The open notebook: pages, undo history and every document operation.
// Pages and items are treated as immutable: a change makes new objects, so
// the renderer can tell what changed by identity and undo is cheap.

import type { Content, Doc, Id, Item, Layer, Link, Page, Paper, Rgba, Sticker, Template } from "../core/model";
import { editable, newId, newPage, visible } from "../core/model";
import type { Pt } from "../core/geom";
import { hitItem, translateItem, unionBBox } from "../core/geom";
import type { Imported } from "../core/pictures";

export interface Selection { page: number; ids: Id[] }

let clipboard: Item[] = [];

export class DocModel {
  doc: Doc;
  title: string;
  readonly metaId: Id;
  private undoStack: Page[][] = [];
  private redoStack: Page[][] = [];
  current = 0;
  activeLayer = new Map<Id, Id>();
  selection: Selection | null = null;
  dirty = false;
  version = 0;
  private listeners = new Set<() => void>();
  toast: (msg: string) => void = () => {};
  /** Scroll request for the view. */
  scrollTo: number | null = null;
  backStack: number[] = [];
  /** While recording: stamps new ink with the recording time. */
  audioClock: (() => { rec: Id; t: number }) | null = null;
  /** While a recording plays: tapping ink written during it seeks there. Returns true if handled. */
  onInkTap: ((it: Item) => boolean) | null = null;

  constructor(metaId: Id, title: string, doc: Doc) {
    this.metaId = metaId;
    this.title = title;
    this.doc = doc;
  }

  subscribe(f: () => void) {
    this.listeners.add(f);
    return () => { this.listeners.delete(f); };
  }
  /** Something visible changed (not necessarily saved data). */
  notify() {
    this.version++;
    this.listeners.forEach((f) => f());
  }
  /** Saved data changed. */
  touch() {
    this.dirty = true;
    this.notify();
  }

  get pages() { return this.doc.pages; }
  page(i: number) { return this.doc.pages[i]; }

  // ---------- history ----------

  private setPages(pages: Page[], undoable = true) {
    if (undoable) {
      this.undoStack.push(this.doc.pages);
      if (this.undoStack.length > 300) this.undoStack.shift();
      this.redoStack = [];
    }
    this.doc = { ...this.doc, pages };
    this.current = Math.min(this.current, pages.length - 1);
    this.touch();
  }
  setPage(i: number, page: Page, undoable = true) {
    const pages = this.doc.pages.slice();
    pages[i] = page;
    this.setPages(pages, undoable);
  }
  updatePage(i: number, f: (p: Page) => Page, undoable = true) {
    this.setPage(i, f(this.doc.pages[i]), undoable);
  }
  canUndo() { return this.undoStack.length > 0; }
  canRedo() { return this.redoStack.length > 0; }
  undo() {
    const prev = this.undoStack.pop();
    if (!prev) return;
    this.redoStack.push(this.doc.pages);
    this.doc = { ...this.doc, pages: prev };
    this.selection = null;
    this.current = Math.min(this.current, prev.length - 1);
    this.touch();
  }
  redo() {
    const next = this.redoStack.pop();
    if (!next) return;
    this.undoStack.push(this.doc.pages);
    this.doc = { ...this.doc, pages: next };
    this.selection = null;
    this.current = Math.min(this.current, next.length - 1);
    this.touch();
  }

  // ---------- layers ----------

  activeLayerId(i: number): Id {
    const p = this.doc.pages[i];
    const a = this.activeLayer.get(p.id);
    if (a !== undefined && p.layers.some((l) => l.id === a)) return a;
    return p.layers[p.layers.length - 1]?.id ?? 0;
  }
  canWrite(i: number) {
    const p = this.doc.pages[i];
    const l = p.layers.find((x) => x.id === this.activeLayerId(i));
    return !l || (l.visible && !l.locked);
  }
  setActiveLayer(i: number, id: Id) {
    this.activeLayer.set(this.doc.pages[i].id, id);
    this.notify();
  }
  addLayer(i: number) {
    const id = newId();
    this.updatePage(i, (p) => ({ ...p, layers: [...p.layers, { id, name: `Layer ${p.layers.length + 1}`, visible: true, locked: false }] }));
    this.activeLayer.set(this.doc.pages[i].id, id);
  }
  updateLayer(i: number, id: Id, f: (l: Layer) => Layer) {
    this.updatePage(i, (p) => ({ ...p, layers: p.layers.map((l) => (l.id === id ? f(l) : l)) }));
    this.selection = null;
  }
  deleteLayer(i: number, id: Id) {
    if (this.doc.pages[i].layers.length <= 1) return this.toast("A page needs at least one layer");
    this.updatePage(i, (p) => ({ ...p, layers: p.layers.filter((l) => l.id !== id), items: p.items.filter((it) => it.layer !== id) }));
    this.selection = null;
  }
  duplicateLayer(i: number, id: Id) {
    this.updatePage(i, (p) => {
      const pos = p.layers.findIndex((l) => l.id === id);
      if (pos < 0) return p;
      const nid = newId();
      const layers = p.layers.slice();
      layers.splice(pos + 1, 0, { ...p.layers[pos], id: nid, name: p.layers[pos].name + " copy" });
      const copies = p.items.filter((it) => it.layer === id).map((it) => ({ ...it, id: newId(), layer: nid }));
      return { ...p, layers, items: [...p.items, ...copies] };
    });
  }
  moveLayer(i: number, id: Id, up: boolean) {
    const p = this.doc.pages[i];
    const pos = p.layers.findIndex((l) => l.id === id);
    const to = up ? pos + 1 : pos - 1;
    if (pos < 0 || to < 0 || to >= p.layers.length) return;
    const layers = p.layers.slice();
    [layers[pos], layers[to]] = [layers[to], layers[pos]];
    this.setPage(i, { ...p, layers });
  }
  mergeDown(i: number, id: Id) {
    const p = this.doc.pages[i];
    const pos = p.layers.findIndex((l) => l.id === id);
    if (pos <= 0) return;
    const below = p.layers[pos - 1].id;
    this.setPage(i, { ...p, layers: p.layers.filter((l) => l.id !== id), items: p.items.map((it) => (it.layer === id ? { ...it, layer: below } : it)) });
  }

  // ---------- items ----------

  addItem(i: number, it: Item) {
    let item = { ...it, layer: this.activeLayerId(i) };
    if (this.audioClock && "Stroke" in item.content) item = { ...item, content: { Stroke: { ...item.content.Stroke, audio: this.audioClock() } } };
    this.updatePage(i, (p) => ({ ...p, items: [...p.items, item] }));
  }
  /** Adds items as one step and selects them. */
  addItems(i: number, items: Item[]) {
    const layer = this.activeLayerId(i);
    const fresh = items.map((it) => ({ ...it, id: newId(), layer }));
    this.updatePage(i, (p) => ({ ...p, items: [...p.items, ...fresh] }));
    this.selection = { page: i, ids: fresh.map((x) => x.id) };
  }
  replaceItems(i: number, items: Item[], undoable = true) {
    this.updatePage(i, (p) => ({ ...p, items }), undoable);
  }
  selectedItems(): Item[] {
    const s = this.selection;
    if (!s || !this.doc.pages[s.page]) return [];
    const ids = new Set(s.ids);
    return this.doc.pages[s.page].items.filter((it) => ids.has(it.id));
  }
  selectionBox() { return unionBBox(this.selectedItems()); }
  mapSelection(f: (it: Item) => Item) {
    const s = this.selection;
    if (!s) return;
    const ids = new Set(s.ids);
    this.updatePage(s.page, (p) => ({ ...p, items: p.items.map((it) => (ids.has(it.id) ? f(it) : it)) }));
  }
  deleteSelection() {
    const s = this.selection;
    if (!s) return;
    const ids = new Set(s.ids);
    this.selection = null;
    this.updatePage(s.page, (p) => ({ ...p, items: p.items.filter((it) => !ids.has(it.id)) }));
  }
  copySelection() { clipboard = this.selectedItems().map((x) => ({ ...x })); }
  hasClipboard() { return clipboard.length > 0; }
  cutSelection() { this.copySelection(); this.deleteSelection(); }
  duplicateSelection() {
    const s = this.selection;
    if (!s) return;
    this.addItems(s.page, this.selectedItems().map((it) => translateItem(it, 16, 16)));
  }
  paste(center: Pt) {
    if (!clipboard.length) return;
    const b = unionBBox(clipboard)!;
    const dx = center[0] - (b.x + b.w / 2), dy = center[1] - (b.y + b.h / 2);
    this.addItems(this.current, clipboard.map((it) => translateItem(it, dx, dy)));
  }
  reorderSelection(front: boolean) {
    const s = this.selection;
    if (!s) return;
    const ids = new Set(s.ids);
    this.updatePage(s.page, (p) => {
      const sel = p.items.filter((it) => ids.has(it.id)), rest = p.items.filter((it) => !ids.has(it.id));
      return { ...p, items: front ? [...rest, ...sel] : [...sel, ...rest] };
    });
  }
  moveSelectionToLayer(layer: Id) { this.mapSelection((it) => ({ ...it, layer })); }
  setSelectionLink(link: Link | null) { this.mapSelection((it) => ({ ...it, link })); }
  selectAll() {
    const p = this.doc.pages[this.current];
    const ids = p.items.filter((it) => editable(p, it)).map((it) => it.id);
    this.selection = ids.length ? { page: this.current, ids } : null;
    this.notify();
  }

  itemAt(i: number, pt: Pt, r: number, f: (it: Item) => boolean): Item | undefined {
    const p = this.doc.pages[i];
    for (let k = p.items.length - 1; k >= 0; k--) {
      const it = p.items[k];
      if (visible(p, it) && f(it) && hitItem(it, pt[0], pt[1], r)) return it;
    }
  }

  // ---------- tape ----------

  toggleTape(i: number, id: Id) {
    this.updatePage(i, (p) => ({
      ...p,
      items: p.items.map((it) => (it.id === id && "Tape" in it.content ? { ...it, content: { Tape: { ...it.content.Tape, revealed: !it.content.Tape.revealed } } } : it)),
    }), false);
  }
  setAllTapes(revealed: boolean) {
    const p = this.doc.pages[this.current];
    if (!p.items.some((it) => "Tape" in it.content)) return this.toast("No tape on this page");
    this.setPage(this.current, {
      ...p,
      items: p.items.map((it) => ("Tape" in it.content ? { ...it, content: { Tape: { ...it.content.Tape, revealed } } } : it)),
    }, false);
  }

  followLink(link: Link) {
    if ("Url" in link) {
      window.open(link.Url, "_blank", "noopener");
      return;
    }
    const i = this.doc.pages.findIndex((p) => p.id === link.Page);
    if (i < 0) return this.toast("That page no longer exists");
    this.backStack.push(this.current);
    this.scrollTo = i;
    this.notify();
  }
  goBack() {
    const i = this.backStack.pop();
    if (i !== undefined) { this.scrollTo = Math.min(i, this.doc.pages.length - 1); this.notify(); }
  }
  goTo(i: number) {
    this.scrollTo = Math.max(0, Math.min(this.doc.pages.length - 1, i));
    this.notify();
  }

  // ---------- pages ----------

  insertPage(after: number, template?: Template, paper?: Paper) {
    const src = this.doc.pages[Math.min(after, this.doc.pages.length - 1)];
    const p = newPage(template ?? src.template, paper ?? src.paper, src.w, src.h);
    const pages = this.doc.pages.slice();
    pages.splice(after + 1, 0, p);
    this.setPages(pages);
    this.scrollTo = after + 1;
  }
  duplicatePage(i: number) {
    const pages = this.doc.pages.slice();
    pages.splice(i + 1, 0, { ...pages[i], id: newId() });
    this.setPages(pages);
  }
  deletePage(i: number) {
    if (this.doc.pages.length === 1) {
      const p = this.doc.pages[0];
      return this.setPages([newPage(p.template, p.paper, p.w, p.h)]);
    }
    this.selection = null;
    this.setPages(this.doc.pages.filter((_, k) => k !== i));
  }
  movePage(from: number, to: number) {
    if (to < 0 || to >= this.doc.pages.length || from === to) return;
    const pages = this.doc.pages.slice();
    const [p] = pages.splice(from, 1);
    pages.splice(to, 0, p);
    this.setPages(pages);
    this.current = to;
  }
  clearPage(i: number) {
    this.selection = null;
    this.updatePage(i, (p) => ({ ...p, items: [] }));
  }
  setTemplate(t: Template, paper: Paper, all: boolean) {
    this.setPages(this.doc.pages.map((p, i) => (all || i === this.current ? { ...p, template: t, paper } : p)));
  }
  setPageSize(w: number, h: number, all: boolean) {
    this.setPages(this.doc.pages.map((p, i) => ((all || i === this.current) && !p.background ? { ...p, w, h } : p)));
  }
  setBookmark(i: number, name: string | null) { this.updatePage(i, (p) => ({ ...p, bookmark: name })); }

  /** Stores recognised handwriting if the page's ink hasn't changed meanwhile. */
  setOcr(pageId: Id, key: string, text: string, currentKey: (p: Page) => string) {
    const i = this.doc.pages.findIndex((p) => p.id === pageId);
    if (i < 0 || currentKey(this.doc.pages[i]) !== key) return;
    this.updatePage(i, (p) => ({ ...p, ocr: { key, text } }), false);
  }

  /** Replaces the selected handwriting with a text box. */
  convertSelectionToText(text: string, size: number, color: Rgba) {
    const s = this.selection;
    const b = this.selectionBox();
    if (!s || !b) return;
    const ids = new Set(this.selectedItems().filter((it) => "Stroke" in it.content || "Shape" in it.content).map((it) => it.id));
    const layer = this.activeLayerId(s.page);
    const box: Item = { id: newId(), layer, link: null, content: { Text: { x: b.x, y: b.y, w: Math.max(120, b.w), text, size, color } } };
    this.updatePage(s.page, (p) => ({ ...p, items: [...p.items.filter((it) => !ids.has(it.id)), box] }));
    this.selection = { page: s.page, ids: [box.id] };
  }

  // ---------- recordings ----------

  get recordings() { return this.doc.recordings ?? []; }
  setRecordings(f: (r: NonNullable<Doc["recordings"]>) => NonNullable<Doc["recordings"]>) {
    this.doc = { ...this.doc, recordings: f(this.recordings) };
    this.touch();
  }

  // ---------- outline ----------

  addOutline(i = this.current): Id {
    const page = this.doc.pages[i];
    const id = newId();
    const entry = { id, title: page.bookmark ?? `Page ${i + 1}`, page: page.id, level: 0 };
    const order = (pid: Id) => this.doc.pages.findIndex((p) => p.id === pid);
    const outline = this.doc.outline.slice();
    const at = outline.findIndex((e) => order(e.page) > i);
    outline.splice(at < 0 ? outline.length : at, 0, entry);
    this.doc = { ...this.doc, outline };
    this.touch();
    return id;
  }
  updateOutline(f: (o: Doc["outline"]) => Doc["outline"]) {
    this.doc = { ...this.doc, outline: f(this.doc.outline) };
    this.touch();
  }

  // ---------- stickers & pictures ----------

  selectionAsSticker(): Sticker | null {
    const items = this.selectedItems();
    const b = unionBBox(items);
    if (!b) return null;
    return { id: newId(), items: items.map((it) => ({ ...translateItem(it, -b.x, -b.y), layer: 0 })), used: Date.now() };
  }
  placeItems(items: Item[], center: Pt) {
    if (!this.canWrite(this.current)) return this.toast("The active layer is locked or hidden");
    const b = unionBBox(items);
    if (!b) return;
    this.addItems(this.current, items.map((it) => translateItem(it, center[0] - b.x - b.w / 2, center[1] - b.y - b.h / 2)));
  }
  insertPicture(imp: Imported, center: Pt) {
    const pg = imp.pages[0];
    if (!pg) return;
    const pw = this.doc.pages[this.current].w;
    const s = Math.min(1, (pw * 0.6) / pg.w, (pw * 0.6) / pg.h);
    const w = pg.w * s, h = pg.h * s;
    const content: Content = { Image: { asset: pg.asset, x: center[0] - w / 2, y: center[1] - h / 2, w, h } };
    this.addItems(this.current, [{ id: newId(), content, layer: 0, link: null }]);
  }
  insertPicturePages(imp: Imported) {
    const at = this.current;
    const baseW = this.doc.pages[at].w;
    const pages = this.doc.pages.slice();
    imp.pages.forEach((pg, k) => {
      const [w, h] = imp.kind === "pdf" ? [pg.w, pg.h] : [baseW, (baseW * pg.h) / pg.w];
      pages.splice(at + 1 + k, 0, { ...newPage("Blank", "White", w, h), background: pg.asset });
    });
    this.setPages(pages);
    this.scrollTo = at + 1;
    this.toast(`Added ${imp.pages.length} page${imp.pages.length === 1 ? "" : "s"}`);
  }

  find(q: string): { page: number; text: string }[] {
    q = q.trim().toLowerCase();
    if (!q) return [];
    const out: { page: number; text: string }[] = [];
    this.doc.pages.forEach((p, i) => {
      const texts = [p.bookmark ?? "", p.ocr?.text ?? "", ...p.items.flatMap((it) => ("Text" in it.content ? [it.content.Text.text] : [])),
        ...this.doc.outline.filter((o) => o.page === p.id).map((o) => o.title)];
      for (const t of texts) {
        const pos = t.toLowerCase().indexOf(q);
        if (pos >= 0) out.push({ page: i, text: t.slice(Math.max(0, pos - 24), pos + q.length + 40).replace(/\n/g, " ") });
      }
    });
    return out;
  }
}
