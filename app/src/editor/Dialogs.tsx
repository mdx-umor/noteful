import { Modal } from "../ui/ui";
import type { Id } from "../core/model";
import type { DocModel } from "./doc";

export type EditDialog =
  | { k: "rename"; name: string }
  | { k: "bookmark"; page: number; name: string }
  | { k: "outline"; id: Id; title: string }
  | { k: "layer"; page: number; id: Id; name: string }
  | { k: "link"; url: string; page: string; web: boolean }
  | { k: "find"; q: string }
  | { k: "goto"; text: string };

export function EditDialogs({ dialog: d, model, onClose, setDialog, toast }: {
  dialog: EditDialog; model: DocModel; onClose: () => void; setDialog: (d: EditDialog) => void; toast: (s: string) => void;
}) {
  const enter = (f: () => void) => (e: React.KeyboardEvent) => { if (e.key === "Enter") { e.preventDefault(); f(); } };
  const simple = (heading: string, value: string, set: (v: string) => void, ok: () => void, extra?: React.ReactNode) => (
    <Modal onClose={onClose}>
      <h2>{heading}</h2>
      <input type="text" autoFocus value={value} onChange={(e) => set(e.target.value)} onKeyDown={enter(ok)} onFocus={(e) => e.target.select()} />
      <div className="actions">{extra}<span className="grow" /><button className="btn" onClick={onClose}>Cancel</button><button className="btn primary" onClick={ok}>Save</button></div>
    </Modal>
  );
  switch (d.k) {
    case "rename":
      return simple("Rename notebook", d.name, (name) => setDialog({ ...d, name }), () => {
        if (d.name.trim()) { model.title = d.name.trim(); model.touch(); }
        onClose();
      });
    case "bookmark":
      return simple("Bookmark", d.name, (name) => setDialog({ ...d, name }), () => {
        model.setBookmark(d.page, d.name.trim() || `Page ${d.page + 1}`);
        onClose();
      }, model.page(d.page)?.bookmark ? <button className="btn danger" onClick={() => { model.setBookmark(d.page, null); onClose(); }}>Remove</button> : undefined);
    case "outline":
      return simple("Outline entry", d.title, (title) => setDialog({ ...d, title }), () => {
        if (d.title.trim()) model.updateOutline((o) => o.map((e) => (e.id === d.id ? { ...e, title: d.title.trim() } : e)));
        onClose();
      });
    case "layer":
      return simple("Rename layer", d.name, (name) => setDialog({ ...d, name }), () => {
        if (d.name.trim()) model.updateLayer(d.page, d.id, (l) => ({ ...l, name: d.name.trim() }));
        onClose();
      });
    case "goto": {
      const go = () => { const n = parseInt(d.text, 10); if (n) model.goTo(n - 1); onClose(); };
      return (
        <Modal onClose={onClose}>
          <h2>Go to page</h2>
          <div className="row">
            <input type="number" autoFocus min={1} max={model.pages.length} value={d.text} onChange={(e) => setDialog({ ...d, text: e.target.value })} onKeyDown={enter(go)} onFocus={(e) => e.target.select()} style={{ width: 100 }} />
            <span className="muted">of {model.pages.length}</span>
          </div>
          <div className="actions">
            <button className="btn" onClick={() => { model.goTo(0); onClose(); }}>First</button>
            <button className="btn" onClick={() => { model.goTo(model.pages.length - 1); onClose(); }}>Last</button>
            <span className="grow" />
            <button className="btn primary" onClick={go}>Go</button>
          </div>
        </Modal>
      );
    }
    case "find": {
      const hits = model.find(d.q);
      return (
        <Modal onClose={onClose}>
          <div className="row"><h2 className="grow" style={{ margin: 0 }}>Find in notebook</h2><button className="btn" onClick={onClose}>Done</button></div>
          <input type="search" autoFocus placeholder="Handwriting, typed text, bookmarks, outline" value={d.q} onChange={(e) => setDialog({ ...d, q: e.target.value })} style={{ marginTop: 12 }} />
          {d.q.trim() && <p className="muted small">{hits.length} match{hits.length === 1 ? "" : "es"} found</p>}
          <div className="menu" style={{ maxHeight: 320, overflowY: "auto" }}>
            {hits.map((h, k) => (
              <button key={k} onClick={() => { model.goTo(h.page); onClose(); }} style={{ whiteSpace: "normal" }}>
                <span className="muted small" style={{ flex: "none" }}>p.{h.page + 1}</span><span>{h.text}</span>
              </button>
            ))}
          </div>
        </Modal>
      );
    }
    case "link": {
      const save = () => {
        let link = null;
        if (d.web) {
          const u = d.url.trim();
          if (u) link = { Url: /^[a-z]+:/i.test(u) ? u : "https://" + u };
        } else {
          const n = parseInt(d.page, 10);
          if (n >= 1 && n <= model.pages.length) link = { Page: model.page(n - 1).id };
        }
        if (!link) return toast("Enter a web address or a valid page number");
        model.setSelectionLink(link);
        toast("Link added. Tap it with a finger or the Hand tool.");
        onClose();
      };
      return (
        <Modal onClose={onClose}>
          <h2>Link Settings</h2>
          <div className="seg" style={{ marginBottom: 12 }}>
            <button className={d.web ? "on" : ""} onClick={() => setDialog({ ...d, web: true })}>Web page</button>
            <button className={!d.web ? "on" : ""} onClick={() => setDialog({ ...d, web: false })}>Page in this notebook</button>
          </div>
          {d.web ? (
            <input type="url" autoFocus placeholder="https://example.com" value={d.url} onChange={(e) => setDialog({ ...d, url: e.target.value })} onKeyDown={enter(save)} />
          ) : (
            <div className="row">
              <span>Page</span>
              <input type="number" autoFocus min={1} max={model.pages.length} value={d.page} onChange={(e) => setDialog({ ...d, page: e.target.value })} onKeyDown={enter(save)} style={{ width: 90 }} />
              <span className="muted">of {model.pages.length}</span>
            </div>
          )}
          <div className="actions">
            <button className="btn danger" onClick={() => { model.setSelectionLink(null); onClose(); }}>Remove link</button>
            <span className="grow" />
            <button className="btn" onClick={onClose}>Cancel</button>
            <button className="btn primary" onClick={save}>Save</button>
          </div>
        </Modal>
      );
    }
  }
}
