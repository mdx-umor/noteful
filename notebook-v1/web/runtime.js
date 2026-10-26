// Browser helpers used by the Rust app (inlined into index.html by bundle.py).
//
// NBFS: the app's "files". Everything lives in IndexedDB (no 5 MB limit like
//       localStorage) and is mirrored in memory, so the app can read synchronously.
// NBIO: picking photos and PDFs, opening links, keeping the screen awake.

window.NBFS = (() => {
  const PREFIX = "notebook-v1:";
  const mem = new Map();
  let db = null;
  let warned = false;

  const warn = (msg) => {
    if (warned) return;
    warned = true;
    window.NBIO && window.NBIO.notice(msg);
  };

  function openDb() {
    return new Promise((resolve, reject) => {
      const req = indexedDB.open("notebook-v1", 1);
      req.onupgradeneeded = () => req.result.createObjectStore("files");
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }

  function loadAll() {
    return new Promise((resolve, reject) => {
      const req = db.transaction("files").objectStore("files").openCursor();
      req.onsuccess = () => {
        const c = req.result;
        if (c) {
          mem.set(c.key, c.value);
          c.continue();
        } else resolve();
      };
      req.onerror = () => reject(req.error);
    });
  }

  function put(key, value) {
    const tx = db.transaction("files", "readwrite");
    tx.objectStore("files").put(value, key);
    tx.onerror = () => warn("Saving failed: the browser's storage is full.");
    tx.onabort = tx.onerror;
  }

  async function init() {
    try {
      if (!("indexedDB" in window)) throw new Error("no IndexedDB");
      db = await openDb();
      await loadAll();
      // First run with IndexedDB: bring over notebooks saved by the older version.
      if (mem.size === 0) {
        for (let i = 0; i < localStorage.length; i++) {
          const k = localStorage.key(i);
          if (k && k.startsWith(PREFIX)) {
            const v = localStorage.getItem(k);
            mem.set(k, v);
            put(k, v);
          }
        }
      }
      if (navigator.storage && navigator.storage.persist) navigator.storage.persist().catch(() => {});
    } catch (e) {
      console.warn("IndexedDB unavailable, using localStorage", e);
      db = null;
    }
  }

  return {
    init,
    get(key) {
      if (db) {
        const v = mem.get(key);
        return v === undefined ? undefined : v;
      }
      const v = localStorage.getItem(key);
      return v === null ? undefined : v;
    },
    set(key, value) {
      if (db) {
        mem.set(key, value);
        put(key, value);
        return true;
      }
      try {
        localStorage.setItem(key, value);
        return true;
      } catch (e) {
        return false;
      }
    },
    del(key) {
      if (db) {
        mem.delete(key);
        db.transaction("files", "readwrite").objectStore("files").delete(key);
      } else localStorage.removeItem(key);
    },
  };
})();

window.NBIO = (() => {
  const queue = [];
  let banner = null;
  let wakeLock = null;
  let wantAwake = false;
  document.addEventListener("visibilitychange", () => {
    if (wantAwake && document.visibilityState === "visible") api.keepAwake(true);
  });

  function show(text, sticky) {
    if (!banner) {
      banner = document.createElement("div");
      banner.style.cssText =
        "position:fixed;left:50%;top:18px;transform:translateX(-50%);z-index:10;" +
        "background:rgba(28,28,30,.94);color:#f2f2f4;padding:10px 18px;border-radius:18px;" +
        "font:15px -apple-system,system-ui,sans-serif;pointer-events:none;box-shadow:0 6px 24px rgba(0,0,0,.35)";
      document.body.appendChild(banner);
    }
    banner.textContent = text;
    banner.style.display = text ? "block" : "none";
    clearTimeout(show.t);
    if (text && !sticky) show.t = setTimeout(() => show(""), 4000);
  }

  function pick(accept) {
    return new Promise((resolve) => {
      const input = document.createElement("input");
      input.type = "file";
      input.accept = accept;
      input.style.cssText = "position:fixed;left:-1000px;top:0;opacity:0";
      document.body.appendChild(input);
      input.addEventListener("change", () => {
        resolve(input.files && input.files[0]);
        input.remove();
      });
      input.click();
    });
  }

  function canvasJpeg(canvas, quality) {
    return new Promise((resolve, reject) =>
      canvas.toBlob(
        (b) => (b ? b.arrayBuffer().then((a) => resolve(new Uint8Array(a))) : reject(new Error("Could not encode the picture"))),
        "image/jpeg",
        quality,
      ),
    );
  }

  async function fromImage(file) {
    const url = URL.createObjectURL(file);
    try {
      const img = new Image();
      img.src = url;
      await img.decode();
      const max = 1400;
      const s = Math.min(1, max / Math.max(img.naturalWidth, img.naturalHeight));
      const w = Math.max(1, Math.round(img.naturalWidth * s));
      const h = Math.max(1, Math.round(img.naturalHeight * s));
      const c = document.createElement("canvas");
      c.width = w;
      c.height = h;
      const ctx = c.getContext("2d");
      ctx.fillStyle = "#fff"; // transparent PNGs get a white background
      ctx.fillRect(0, 0, w, h);
      ctx.drawImage(img, 0, 0, w, h);
      return { kind: "image", name: file.name, pages: [{ w, h, jpeg: await canvasJpeg(c, 0.85) }] };
    } finally {
      URL.revokeObjectURL(url);
    }
  }

  const PDFJS = "https://cdnjs.cloudflare.com/ajax/libs/pdf.js/3.11.174/";
  async function pdfLib() {
    if (!window.pdfjsLib) {
      await new Promise((resolve, reject) => {
        const s = document.createElement("script");
        s.src = PDFJS + "pdf.min.js";
        s.onload = resolve;
        s.onerror = () => reject(new Error("Could not load the PDF reader. Are you online?"));
        document.head.appendChild(s);
      });
      window.pdfjsLib.GlobalWorkerOptions.workerSrc = PDFJS + "pdf.worker.min.js";
    }
    return window.pdfjsLib;
  }

  async function fromPdf(file) {
    const lib = await pdfLib();
    const doc = await lib.getDocument({ data: new Uint8Array(await file.arrayBuffer()) }).promise;
    const pages = [];
    const n = doc.numPages;
    for (let i = 1; i <= n; i++) {
      show(`Importing ${file.name}: page ${i} of ${n}`, true);
      const page = await doc.getPage(i);
      const base = page.getViewport({ scale: 1 });
      const vp = page.getViewport({ scale: Math.min(2.5, 1400 / base.width) });
      const c = document.createElement("canvas");
      c.width = Math.round(vp.width);
      c.height = Math.round(vp.height);
      const ctx = c.getContext("2d");
      ctx.fillStyle = "#fff";
      ctx.fillRect(0, 0, c.width, c.height);
      await page.render({ canvasContext: ctx, viewport: vp }).promise;
      pages.push({ w: base.width, h: base.height, jpeg: await canvasJpeg(c, 0.82) });
      page.cleanup();
    }
    doc.destroy();
    return { kind: "pdf", name: file.name, pages };
  }

  const api = {
    // Opens the file picker; the result is collected later with poll().
    start(accept) {
      pick(accept).then(async (file) => {
        if (!file) {
          queue.push({ kind: "cancel", name: "", pages: [] });
          return;
        }
        try {
          show("Importing " + file.name + "…", true);
          const isPdf = file.type === "application/pdf" || /\.pdf$/i.test(file.name);
          queue.push(isPdf ? await fromPdf(file) : await fromImage(file));
          show("");
        } catch (e) {
          console.error(e);
          show("");
          queue.push({ kind: "error", name: String((e && e.message) || e), pages: [] });
        }
      });
    },
    poll() {
      return queue.length ? queue.shift() : null;
    },
    openUrl(url) {
      window.open(url, "_blank", "noopener");
    },
    keepAwake(on) {
      wantAwake = on;
      if (!("wakeLock" in navigator)) return;
      if (on && !wakeLock) {
        navigator.wakeLock.request("screen").then((l) => {
          wakeLock = l;
          l.addEventListener("release", () => (wakeLock = null));
        }).catch(() => {});
      } else if (!on && wakeLock) {
        wakeLock.release();
        wakeLock = null;
      }
    },
    notice(text) {
      show(text, false);
    },
  };
  return api;
})();
