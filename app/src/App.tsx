import { useCallback, useEffect, useRef, useState } from "react";
import type { Doc, Id, Library } from "./core/model";
import { fromHex, newDoc, searchText, sizeDims } from "./core/model";
import * as storage from "./core/storage";
import { loadPrefs, savePrefs, type Prefs } from "./prefs";
import { LibraryView } from "./library/LibraryView";
import { EditorView } from "./editor/EditorView";
import { DocModel } from "./editor/doc";
import * as store from "./store";

export interface AppApi {
  lib: Library;
  setLib: (l: Library) => void;
  prefs: Prefs;
  setPrefs: (f: (p: Prefs) => void) => void;
  toast: (msg: string) => void;
  open: (id: Id) => void;
  createAndOpen: (doc: Doc, title: string, color?: string, folder?: Id | null, tags?: string[]) => void;
  quickNote: () => void;
}

export function App() {
  const [lib, setLibState] = useState<Library>(() => storage.loadLibrary());
  const [prefs, setPrefsState] = useState<Prefs>(() => loadPrefs());
  const prefsRef = useRef(prefs);
  const [model, setModel] = useState<DocModel | null>(null);
  const [toastMsg, setToastMsg] = useState<string | null>(null);
  const toastTimer = useRef(0);

  const toast = useCallback((msg: string) => {
    setToastMsg(msg);
    clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToastMsg(null), 2800);
  }, []);
  const setLib = useCallback((l: Library) => { setLibState(l); storage.saveLibrary(l); }, []);
  const setPrefs = useCallback((f: (p: Prefs) => void) => {
    const next = { ...prefsRef.current };
    f(next);
    prefsRef.current = next;
    setPrefsState(next);
    savePrefs(next);
  }, []);

  useEffect(() => {
    storage.setStorageErrorHandler(toast);
    let l = store.purgeOldTrash(storage.loadLibrary());
    if (!l.notebooks.length && !storage.readFlag("welcomed")) {
      storage.writeFlag("welcomed");
      l = store.createNotebook(l, store.welcomeDoc(), "Welcome to Notebook", fromHex("#5b5bd6"), null, ["Sample"]);
    }
    setLibState(l);
  }, [toast]);

  // Keep the screen awake while writing, if asked.
  useEffect(() => {
    let lock: { release: () => Promise<void> } | null = null;
    const nav = navigator as Navigator & { wakeLock?: { request: (t: string) => Promise<{ release: () => Promise<void> }> } };
    const get = () => { if (prefs.keep_awake && nav.wakeLock && document.visibilityState === "visible") nav.wakeLock.request("screen").then((l) => (lock = l)).catch(() => {}); };
    get();
    document.addEventListener("visibilitychange", get);
    return () => { document.removeEventListener("visibilitychange", get); lock?.release().catch(() => {}); };
  }, [prefs.keep_awake]);

  const open = useCallback((id: Id) => {
    const doc = storage.loadDoc(id);
    const meta = storage.loadLibrary().notebooks.find((m) => m.id === id);
    if (!doc || !meta) return toast("Could not open this notebook");
    const m = new DocModel(id, meta.title, doc);
    m.toast = toast;
    setLibState((l) => store.updateMeta(l, id, (x) => ({ ...x, opened: Date.now() })));
    setModel(m);
  }, [toast]);

  const thumbFor = useRef<{ page: unknown; at: number }>({ page: null, at: 0 });
  /** Writes the open notebook and its library entry. */
  const save = useCallback((m: DocModel) => {
    if (!m.dirty) return;
    m.dirty = false;
    storage.saveDoc(m.doc);
    // Refresh the library thumbnail when the first page changed (at most every few seconds).
    const first = m.doc.pages[0];
    if (first !== thumbFor.current.page && Date.now() - thumbFor.current.at > 4000) {
      thumbFor.current = { page: first, at: Date.now() };
      void store.saveThumbnail(m.doc);
    }
    setLibState((l) => store.updateMeta(l, m.metaId, (x) => ({
      ...x, title: m.title, updated: Date.now(), page_count: m.doc.pages.length, search_text: searchText(m.doc),
      template: m.doc.pages[0].template, paper: m.doc.pages[0].paper,
    })));
  }, []);

  // Autosave shortly after each change, and when the page is hidden or closed.
  useEffect(() => {
    if (!model) return;
    let t = 0;
    const unsub = model.subscribe(() => {
      if (!model.dirty) return;
      clearTimeout(t);
      t = window.setTimeout(() => save(model), 600);
    });
    const flush = () => { save(model); };
    addEventListener("pagehide", flush);
    document.addEventListener("visibilitychange", flush);
    return () => { unsub(); clearTimeout(t); removeEventListener("pagehide", flush); document.removeEventListener("visibilitychange", flush); };
  }, [model, save]);

  const close = useCallback(() => {
    if (!model) return;
    const changed = model.dirty || model.canUndo();
    save(model);
    if (changed) store.saveThumbnail(model.doc).then(() => setLibState((l) => ({ ...l })));
    setModel(null);
  }, [model, save]);

  const createAndOpen = useCallback((doc: Doc, title: string, color = "#0a84ff", folder: Id | null = null, tags: string[] = []) => {
    setLibState((l) => store.createNotebook(l, doc, title, fromHex(color), folder, tags));
    setTimeout(() => open(doc.id), 0);
  }, [open]);

  const quickNote = useCallback(() => {
    const p = prefsRef.current;
    const [w, h] = sizeDims(p.new_size, p.new_landscape);
    createAndOpen(newDoc(p.new_template, p.new_paper, w, h), `Notes on ${store.today()}`);
  }, [createAndOpen]);

  const api: AppApi = { lib, setLib, prefs, setPrefs, toast, open, createAndOpen, quickNote };

  return (
    <div className={"app" + (prefs.dark_ui ? "" : " light")}>
      {model ? <EditorView key={model.metaId} model={model} api={api} onClose={close} /> : <LibraryView api={api} />}
      {toastMsg && <div className="toast">{toastMsg}</div>}
    </div>
  );
}
