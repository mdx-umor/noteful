// Notebook storage in the browser. Same IndexedDB database, keys and value format
// as the previous version ("t:" + text, or "b:" + base64 bytes), so nothing is lost.

import type { Doc, Id, Library } from "./model";
import { normalizeDoc } from "./model";

const PREFIX = "notebook-v1:data/";
const SNAP = "notebook-v1:snapshots/";
const mem = new Map<string, string>();
let db: IDBDatabase | null = null;
let onError: (msg: string) => void = () => {};

export function setStorageErrorHandler(f: (msg: string) => void) {
  onError = f;
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open("notebook-v1", 1);
    req.onupgradeneeded = () => req.result.createObjectStore("files");
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function initStorage(): Promise<void> {
  try {
    db = await openDb();
    await new Promise<void>((resolve, reject) => {
      const req = db!.transaction("files").objectStore("files").openCursor();
      req.onsuccess = () => {
        const c = req.result;
        if (c) {
          // Backup snapshots are big and only read on demand.
          if (!String(c.key).startsWith(SNAP)) mem.set(String(c.key), c.value as string);
          c.continue();
        } else resolve();
      };
      req.onerror = () => reject(req.error);
    });
    // Very first versions kept everything in localStorage: bring it over once.
    if (mem.size === 0) {
      for (let i = 0; i < localStorage.length; i++) {
        const k = localStorage.key(i);
        if (k && k.startsWith("notebook-v1:")) {
          const v = localStorage.getItem(k)!;
          mem.set(k, v);
          put(k, v);
        }
      }
    }
    navigator.storage?.persist?.().catch(() => {});
  } catch (e) {
    console.warn("IndexedDB unavailable, using localStorage", e);
    db = null;
  }
}

function put(key: string, value: string) {
  if (!db) {
    try {
      localStorage.setItem(key, value);
    } catch {
      onError("Saving failed: the browser's storage is full.");
    }
    return;
  }
  const tx = db.transaction("files", "readwrite");
  tx.objectStore("files").put(value, key);
  tx.onerror = tx.onabort = () => onError("Saving failed: the browser's storage is full.");
}

function getRaw(name: string): string | undefined {
  const key = PREFIX + name;
  if (db) return mem.get(key);
  return localStorage.getItem(key) ?? undefined;
}

function setRaw(name: string, value: string) {
  const key = PREFIX + name;
  if (db) mem.set(key, value);
  put(key, value);
}

export const deleteRaw = (name: string) => delRaw(name);
function delRaw(name: string) {
  const key = PREFIX + name;
  mem.delete(key);
  if (db) db.transaction("files", "readwrite").objectStore("files").delete(key);
  else localStorage.removeItem(key);
}

export function b64encode(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}
export function b64decode(s: string): Uint8Array {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export function readText(name: string): string | null {
  const v = getRaw(name);
  if (v === undefined) return null;
  if (v.startsWith("t:")) return v.slice(2);
  if (v.startsWith("b:")) return new TextDecoder().decode(b64decode(v.slice(2)));
  return null;
}
export const writeText = (name: string, text: string) => setRaw(name, "t:" + text);

export function readBytes(name: string): Uint8Array | null {
  const v = getRaw(name);
  if (v === undefined) return null;
  if (v.startsWith("b:")) return b64decode(v.slice(2));
  if (v.startsWith("t:")) return new TextEncoder().encode(v.slice(2));
  return null;
}
export const writeBytes = (name: string, bytes: Uint8Array) => setRaw(name, "b:" + b64encode(bytes));

function readJson<T>(name: string): T | null {
  const t = readText(name);
  if (t == null) return null;
  try {
    return JSON.parse(t) as T;
  } catch {
    return null;
  }
}

export const loadLibrary = (): Library => {
  const l = readJson<Library>("library.json");
  return { notebooks: (l?.notebooks ?? []).map((m) => ({ ...m, tags: m.tags ?? [], pinned: !!m.pinned })), folders: l?.folders ?? [] };
};
export const saveLibrary = (l: Library) => writeText("library.json", JSON.stringify(l));

export function loadDoc(id: Id): Doc | null {
  const d = readJson<Doc>(`notebooks/${id}.json`);
  return d ? normalizeDoc(d) : null;
}
export const saveDoc = (d: Doc) => writeText(`notebooks/${d.id}.json`, JSON.stringify(d));
export const deleteDoc = (id: Id) => delRaw(`notebooks/${id}.json`);

export const loadThumb = (id: Id) => getRaw(`thumbs/${id}.png`)?.replace(/^b:/, "") ?? null;
export const saveThumb = (id: Id, pngBase64: string) => setRaw(`thumbs/${id}.png`, "b:" + pngBase64);
export const deleteThumb = (id: Id) => delRaw(`thumbs/${id}.png`);

export const loadAsset = (id: Id) => readBytes(`assets/${id}.jpg`);
export const saveAsset = (id: Id, jpeg: Uint8Array) => writeBytes(`assets/${id}.jpg`, jpeg);
export const deleteAsset = (id: Id) => delRaw(`assets/${id}.jpg`);

export const readFlag = (name: string) => getRaw(name) !== undefined;
export const writeFlag = (name: string) => setRaw(name, "t:1");

/** Every stored file, for a full backup. */
export function allFiles(): [string, string][] {
  if (db) return [...mem.entries()].filter(([k]) => k.startsWith(PREFIX)).map(([k, v]) => [k.slice(PREFIX.length), v]);
  const out: [string, string][] = [];
  for (let i = 0; i < localStorage.length; i++) {
    const k = localStorage.key(i)!;
    if (k.startsWith(PREFIX)) out.push([k.slice(PREFIX.length), localStorage.getItem(k)!]);
  }
  return out;
}
export const restoreFile = (name: string, value: string) => setRaw(name, value);

// ---------- automatic backups (kept in IndexedDB, outside the in-memory cache) ----------

function idb<T>(mode: IDBTransactionMode, f: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    if (!db) return reject(new Error("Storage unavailable"));
    const req = f(db.transaction("files", mode).objectStore("files"));
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function listSnapshots(): Promise<number[]> {
  if (!db) return [];
  const keys = await idb("readonly", (s) => s.getAllKeys());
  return keys.map(String).filter((k) => k.startsWith(SNAP)).map((k) => Number(k.slice(SNAP.length))).sort((a, b) => b - a);
}
export async function saveSnapshot(json: string, keep = 7) {
  if (!db) return;
  await idb("readwrite", (s) => s.put(json, SNAP + Date.now()));
  const all = await listSnapshots();
  for (const ts of all.slice(keep)) await idb("readwrite", (s) => s.delete(SNAP + ts));
}
export const loadSnapshot = (ts: number) => idb<string>("readonly", (s) => s.get(SNAP + ts));
