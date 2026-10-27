import { useEffect, useMemo, useReducer, useState } from "react";
import type { Filter, Id, NotebookMeta, Paper, Rgba, Template, TagNode } from "../core/model";
import { css, fromHex, matches, newDoc, normalizeTag, PAPERS, PAPER_SIZES, paperBg, query, SIZE_NAMES, sizeDims, tagMatches, tagTree } from "../core/model";
import * as storage from "../core/storage";
import { importFile, pickFile } from "../core/pictures";
import type { AppApi } from "../App";
import { Icon } from "../ui/icons";
import { Modal, Popover, rectOf, Seg, TemplatePicker, timeAgo, Toggle, PaperSwatches, type Anchor } from "../ui/ui";
import * as store from "../store";

type Dialog =
  | { k: "new"; title: string; cover: string; template: Template; paper: Paper; folder: Id | null; tags: string }
  | { k: "rename"; id: Id; name: string }
  | { k: "tags"; ids: Id[]; text: string }
  | { k: "move"; ids: Id[] }
  | { k: "newFolder"; name: string }
  | { k: "renameFolder"; id: Id; name: string }
  | { k: "renameTag"; path: string; name: string }
  | { k: "confirmDelete"; ids: Id[] }
  | { k: "emptyTrash" }
  | { k: "settings" };

type Menu = { k: "card"; id: Id; at: Anchor } | { k: "sort"; at: Anchor } | { k: "fab"; at: Anchor } | { k: "folder"; id: Id; at: Anchor } | { k: "tag"; path: string; at: Anchor };

const title = (f: Filter, folders: { id: Id; name: string }[]) =>
  ({ all: "Notebooks", untagged: "Untagged", unfiled: "Unfiled", trash: "Trash", pinned: "Pinned" } as Record<string, string>)[f.k] ??
  (f.k === "folder" ? folders.find((x) => x.id === f.id)?.name ?? "Folder" : f.k === "tag" ? "#" + f.path : "");

export function LibraryView({ api }: { api: AppApi }) {
  const { lib, setLib, prefs, setPrefs, toast } = api;
  const [filter, setFilter] = useState<Filter>({ k: "all" });
  const [search, setSearch] = useState<string | null>(null);
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<Set<Id>>(new Set());
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  const [sideOpen, setSideOpen] = useState(() => innerWidth > 760);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [cardSize, setCardSize] = useState(150);
  const [busy, setBusy] = useState<string | null>(null);
  const [, refresh] = useReducer((x: number) => x + 1, 0);
  // Thumbnails are rendered in the background; redraw when one is ready.
  useEffect(() => { addEventListener("nb-thumb", refresh); return () => removeEventListener("nb-thumb", refresh); }, []);

  const list = useMemo(() => query(lib, filter, search ?? "", prefs.sort), [lib, filter, search, prefs.sort]);
  const tags = useMemo(() => tagTree(lib), [lib]);
  const count = (f: Filter) => lib.notebooks.filter((m) => matches(m, f)).length;
  const same = (a: Filter, b: Filter) => JSON.stringify(a) === JSON.stringify(b);
  const pick = (f: Filter) => { setFilter(f); setSelecting(false); setSelected(new Set()); if (innerWidth <= 760) setSideOpen(false); };
  const update = (ids: Id[], f: (m: NotebookMeta) => NotebookMeta) => setLib({ ...lib, notebooks: lib.notebooks.map((m) => (ids.includes(m.id) ? f(m) : m)) });

  const startNew = () => setDialog({
    k: "new", title: "", cover: store.COVERS[0], template: prefs.new_template, paper: prefs.new_paper,
    folder: filter.k === "folder" ? filter.id : null, tags: filter.k === "tag" ? filter.path : "",
  });

  const doImport = async () => {
    const file = await pickFile("application/pdf,image/*");
    if (!file) return;
    try {
      setBusy("Importing " + file.name + "…");
      const imp = await importFile(file, setBusy);
      const doc = store.notebookFromImport(imp);
      api.createAndOpen(doc, file.name.replace(/\.[^.]+$/, ""), "#30b0c7", filter.k === "folder" ? filter.id : null, ["Imported"]);
    } catch (e) {
      toast("Import failed: " + (e instanceof Error ? e.message : String(e)));
    } finally {
      setBusy(null);
    }
  };

  const restore = async () => {
    const file = await pickFile("application/json,.json");
    if (!file) return;
    try {
      const n = await store.restoreBackup(file);
      setLib(storage.loadLibrary());
      toast(`Restored ${n} notebook${n === 1 ? "" : "s"}`);
    } catch (e) {
      toast("Restore failed: " + (e instanceof Error ? e.message : String(e)));
    }
  };

  const tagRows = (nodes: TagNode[], depth: number): JSX.Element[] => nodes.flatMap((n) => {
    const f: Filter = { k: "tag", path: n.path };
    const open = expanded.has(n.path);
    return [
      <div key={n.path} className="row" style={{ paddingLeft: depth * 16 }}>
        <button className={"side-row" + (same(filter, f) ? " on" : "")} onClick={() => pick(f)}
          onContextMenu={(e) => { e.preventDefault(); setMenu({ k: "tag", path: n.path, at: rectOf(e.currentTarget) }); }}>
          <Icon name="hash" />
          <span>{n.name}</span>
          <span className="count">{n.count}</span>
        </button>
        {n.children.length > 0 && (
          <button className="icon-btn" style={{ width: 28, height: 28 }} onClick={() => setExpanded((s) => { const x = new Set(s); if (open) x.delete(n.path); else x.add(n.path); return x; })}>
            <Icon name={open ? "chevron" : "chevronRight"} size={16} />
          </button>
        )}
      </div>,
      ...(open ? tagRows(n.children, depth + 1) : []),
    ];
  });

  const sideRow = (f: Filter, icon: string, label: string, indent = 0) => (
    <button key={label + icon} className={"side-row" + (same(filter, f) ? " on" : "")} style={{ paddingLeft: 10 + indent }} onClick={() => pick(f)}>
      <Icon name={icon} /><span>{label}</span><span className="count">{count(f)}</span>
    </button>
  );

  const openCard = (m: NotebookMeta) => {
    if (selecting) setSelected((s) => { const x = new Set(s); if (x.has(m.id)) x.delete(m.id); else x.add(m.id); return x; });
    else if (m.trashed_at != null) setMenu(null);
    else api.open(m.id);
  };

  const thumb = (m: NotebookMeta) => {
    const t = storage.loadThumb(m.id);
    return t ? <img src={"data:image/png;base64," + t} alt="" draggable={false} style={{ borderBottom: `3px solid ${css(m.cover)}` }} /> : <div className="cover" style={{ background: css(m.cover) }} />;
  };

  return (
    <div className="library">
      {sideOpen && (
        <aside className="lib-side">
          <div className="brand">
            <div className="logo">N</div>Notebook
            <span className="grow" />
            <button className="icon-btn" title="Settings" onClick={() => setDialog({ k: "settings" })}><Icon name="gear" /></button>
          </div>
          {sideRow({ k: "all" }, "notebook", "Notebooks")}
          {sideRow({ k: "untagged" }, "untagged", "Untagged", 18)}
          {sideRow({ k: "unfiled" }, "unfiled", "Unfiled", 18)}
          {sideRow({ k: "trash" }, "trash", "Trash", 18)}
          {sideRow({ k: "pinned" }, "pin", "Pinned")}
          <div className="section" style={{ padding: "0 10px" }}>Folders</div>
          {lib.folders.map((f) => (
            <button key={f.id} className={"side-row" + (same(filter, { k: "folder", id: f.id }) ? " on" : "")} onClick={() => pick({ k: "folder", id: f.id })}
              onContextMenu={(e) => { e.preventDefault(); setMenu({ k: "folder", id: f.id, at: rectOf(e.currentTarget) }); }}>
              <Icon name="folder" /><span>{f.name}</span><span className="count">{count({ k: "folder", id: f.id })}</span>
            </button>
          ))}
          <button className="side-row" onClick={() => setDialog({ k: "newFolder", name: "" })}><Icon name="plus" /><span className="muted">New folder</span></button>
          <div className="side-sep" />
          {tags.length ? tagRows(tags, 0) : <div className="muted small" style={{ padding: "4px 10px" }}>Tags appear here. Add one from a notebook's menu, e.g. School/Physics.</div>}
          <div className="grow" />
          <div className="muted small" style={{ padding: "16px 10px 4px" }}>Right-click or long-press folders and tags to rename or delete them.</div>
        </aside>
      )}
      <main className="lib-main">
        <div className="lib-top">
          <button className={"icon-btn" + (sideOpen ? " active" : "")} onClick={() => setSideOpen(!sideOpen)} title="Sidebar"><Icon name="sidebar" /></button>
          {search === null ? (
            <h1>{title(filter, lib.folders)}</h1>
          ) : (
            <input type="search" autoFocus placeholder="Find in library" value={search} onChange={(e) => setSearch(e.target.value)} style={{ maxWidth: 420 }} />
          )}
          <span className="grow" />
          <button className={"icon-btn" + (search !== null ? " active" : "")} title="Search" onClick={() => setSearch(search === null ? "" : null)}><Icon name={search === null ? "search" : "close"} /></button>
          <button className="icon-btn" title={prefs.list_view ? "Grid" : "List"} onClick={() => setPrefs((p) => (p.list_view = !p.list_view))}><Icon name={prefs.list_view ? "grid" : "list"} /></button>
          <button className="icon-btn" title="Sort" onClick={(e) => setMenu({ k: "sort", at: rectOf(e.currentTarget) })}><Icon name="sort" /></button>
          <button className={"icon-btn" + (selecting ? " active" : "")} title="Select" onClick={() => { setSelecting(!selecting); setSelected(new Set()); }}><Icon name="select" /></button>
          <button className="icon-btn" title="Quick note" onClick={api.quickNote}><Icon name="quick" /></button>
        </div>
        {search !== null && search && <div className="muted small" style={{ padding: "0 22px" }}>{list.length} match{list.length === 1 ? "" : "es"} found</div>}
        {filter.k === "trash" && list.length > 0 && (
          <div className="row" style={{ padding: "0 22px" }}>
            <span className="muted small grow">Notebooks in the trash are deleted after 30 days.</span>
            <button className="btn danger" onClick={() => setDialog({ k: "emptyTrash" })}>Empty trash</button>
          </div>
        )}
        {prefs.list_view ? (
          <div className="lib-list">
            {list.map((m) => (
              <div key={m.id} className="list-row" onClick={() => openCard(m)} onContextMenu={(e) => { e.preventDefault(); setMenu({ k: "card", id: m.id, at: rectOf(e.currentTarget) }); }}>
                {selecting && <span className={"check" + (selected.has(m.id) ? " on" : "")} style={{ position: "static", flex: "none" }} />}
                {thumb(m)}
                <div className="grow" style={{ minWidth: 0 }}>
                  <div style={{ fontWeight: 600 }}>{m.title}</div>
                  <div className="chips" style={{ justifyContent: "flex-start", marginTop: 4 }}>{m.tags.map((t) => <span className="chip" key={t}>#{t}</span>)}</div>
                </div>
                <span className="muted small">{timeAgo(m.updated)} ago · {m.page_count} p.</span>
                <button className="icon-btn" onClick={(e) => { e.stopPropagation(); setMenu({ k: "card", id: m.id, at: rectOf(e.currentTarget) }); }}><Icon name="more" /></button>
              </div>
            ))}
            {!list.length && <div className="empty">No notebooks</div>}
          </div>
        ) : (
          <div className="lib-grid" style={{ ["--card" as string]: cardSize + "px" }}>
            {list.map((m) => (
              <div key={m.id} className="card" onContextMenu={(e) => { e.preventDefault(); setMenu({ k: "card", id: m.id, at: rectOf(e.currentTarget) }); }}>
                <button className="thumb" onClick={() => openCard(m)}>
                  {thumb(m)}
                  <span className="badge">{timeAgo(m.updated)}</span>
                  {m.pinned && <span className="pin"><Icon name="pin" size={14} /></span>}
                  {selecting && <span className={"check" + (selected.has(m.id) ? " on" : "")} />}
                </button>
                <button className="title" onClick={(e) => setMenu({ k: "card", id: m.id, at: rectOf(e.currentTarget) })}>{m.title}</button>
                {m.tags.length > 0 && <div className="chips">{m.tags.map((t) => <span className="chip" key={t}>#{t}</span>)}</div>}
              </div>
            ))}
            {!list.length && <div className="empty">{search ? "Nothing matches your search." : filter.k === "trash" ? "Trash is empty." : "No notebooks yet. Tap + to create one."}</div>}
          </div>
        )}
        {selecting ? (
          <div className="sel-bar">
            <button className="btn" disabled={!selected.size} onClick={() => setDialog({ k: "tags", ids: [...selected], text: "" })}>Tags</button>
            <button className="btn" disabled={!selected.size} onClick={() => setDialog({ k: "move", ids: [...selected] })}>Move</button>
            <button className="btn" disabled={!selected.size} onClick={() => { let l = lib; selected.forEach((id) => (l = store.duplicateNotebook(l, id))); setLib(l); setSelecting(false); }}>Duplicate</button>
            <button className="btn" disabled={!selected.size} onClick={() => { update([...selected], (m) => ({ ...m, pinned: !m.pinned })); }}>Pin</button>
            {filter.k === "trash" ? (
              <>
                <button className="btn" disabled={!selected.size} onClick={() => { update([...selected], (m) => ({ ...m, trashed_at: null })); setSelected(new Set()); }}>Restore</button>
                <button className="btn danger" disabled={!selected.size} onClick={() => setDialog({ k: "confirmDelete", ids: [...selected] })}>Delete</button>
              </>
            ) : (
              <button className="btn danger" disabled={!selected.size} onClick={() => { update([...selected], (m) => ({ ...m, trashed_at: Date.now() })); setSelected(new Set()); toast("Moved to trash"); }}>Delete</button>
            )}
            <button className="btn primary" onClick={() => { setSelecting(false); setSelected(new Set()); }}>Done</button>
          </div>
        ) : filter.k !== "trash" && (
          <button className="fab" onClick={(e) => setMenu({ k: "fab", at: rectOf(e.currentTarget) })} title="New"><Icon name="plus" /></button>
        )}
      </main>

      {menu && (
        <Popover anchor={menu.at} side={menu.k === "fab" ? "above" : menu.k === "sort" ? "below" : "right"} width={menu.k === "fab" ? 250 : 240} onClose={() => setMenu(null)}>
          <div className="menu">{renderMenu()}</div>
        </Popover>
      )}
      {dialog && renderDialog()}
      {busy && <div className="toast">{busy}</div>}
    </div>
  );

  function renderMenu() {
    const close = () => setMenu(null);
    if (!menu) return null;
    if (menu.k === "sort") return (
      <>
        <div className="menu-title">Sort by</div>
        {(["Modified", "Opened", "Created", "Name"] as const).map((s) => (
          <button key={s} onClick={() => { setPrefs((p) => (p.sort = s)); close(); }}>
            <span className="grow">{{ Modified: "Date modified", Opened: "Last opened", Created: "Date created", Name: "Name" }[s]}</span>{prefs.sort === s && <Icon name="check" size={18} />}
          </button>
        ))}
        <hr />
        <button onClick={() => setCardSize((s) => Math.min(240, s + 30))}><Icon name="zoomIn" size={18} />Bigger thumbnails</button>
        <button onClick={() => setCardSize((s) => Math.max(100, s - 30))}><Icon name="zoomOut" size={18} />Smaller thumbnails</button>
      </>
    );
    if (menu.k === "fab") return (
      <>
        <button onClick={() => { close(); startNew(); }}><Icon name="notebook" size={18} />New notebook</button>
        <button onClick={() => { close(); api.quickNote(); }}><Icon name="quick" size={18} />Quick note</button>
        <button onClick={() => { close(); doImport(); }}><Icon name="file" size={18} />Import PDF or photo…</button>
        <button onClick={() => { close(); setDialog({ k: "newFolder", name: "" }); }}><Icon name="folder" size={18} />New folder</button>
      </>
    );
    if (menu.k === "folder") {
      const f = lib.folders.find((x) => x.id === menu.id);
      return (
        <>
          <button onClick={() => { close(); setDialog({ k: "renameFolder", id: menu.id, name: f?.name ?? "" }); }}>Rename folder</button>
          <button className="danger-text" onClick={() => { close(); setLib({ folders: lib.folders.filter((x) => x.id !== menu.id), notebooks: lib.notebooks.map((m) => (m.folder === menu.id ? { ...m, folder: null } : m)) }); if (filter.k === "folder") setFilter({ k: "all" }); }}>Delete folder (keeps notebooks)</button>
        </>
      );
    }
    if (menu.k === "tag") return (
      <>
        <button onClick={() => { close(); setDialog({ k: "renameTag", path: menu.path, name: menu.path }); }}>Rename tag</button>
        <button className="danger-text" onClick={() => { close(); setLib({ ...lib, notebooks: lib.notebooks.map((m) => ({ ...m, tags: m.tags.filter((t) => !tagMatches(t, menu.path)) })) }); setFilter({ k: "all" }); }}>Delete tag</button>
      </>
    );
    const m = lib.notebooks.find((x) => x.id === menu.id);
    if (!m) return null;
    if (m.trashed_at != null) return (
      <>
        <div className="menu-title">{m.title}</div>
        <button onClick={() => { close(); update([m.id], (x) => ({ ...x, trashed_at: null })); }}>Restore</button>
        <button className="danger-text" onClick={() => { close(); setDialog({ k: "confirmDelete", ids: [m.id] }); }}>Delete forever</button>
      </>
    );
    return (
      <>
        <div className="menu-title">{m.title}</div>
        <button onClick={() => { close(); api.open(m.id); }}><Icon name="notebook" size={18} />Open</button>
        <button onClick={() => { close(); setDialog({ k: "rename", id: m.id, name: m.title }); }}><Icon name="text" size={18} />Rename</button>
        <button onClick={() => { close(); setDialog({ k: "tags", ids: [m.id], text: m.tags.join(", ") }); }}><Icon name="hash" size={18} />Tags…</button>
        <button onClick={() => { close(); update([m.id], (x) => ({ ...x, pinned: !x.pinned })); }}><Icon name="pin" size={18} />{m.pinned ? "Unpin" : "Pin"}</button>
        <button onClick={() => { close(); setDialog({ k: "move", ids: [m.id] }); }}><Icon name="folder" size={18} />Move to folder…</button>
        <div className="menu-title">Cover colour</div>
        <div className="swatches" style={{ padding: "2px 10px 8px" }}>
          {store.COVERS.map((c) => <button key={c} className="swatch" style={{ background: c, width: 24, height: 24 }} onClick={() => { update([m.id], (x) => ({ ...x, cover: fromHex(c) })); close(); }} />)}
        </div>
        <button onClick={() => { close(); setLib(store.duplicateNotebook(lib, m.id)); }}><Icon name="duplicate" size={18} />Duplicate</button>
        <button onClick={() => { close(); store.exportNotebookPdf(m.id, m.title); }}><Icon name="export" size={18} />Export PDF</button>
        <hr />
        <button className="danger-text" onClick={() => { close(); update([m.id], (x) => ({ ...x, trashed_at: Date.now() })); toast("Moved to trash"); }}><Icon name="trash" size={18} />Move to trash</button>
      </>
    );
  }

  function renderDialog() {
    const close = () => setDialog(null);
    const d = dialog!;
    const enter = (f: () => void) => (e: React.KeyboardEvent) => { if (e.key === "Enter") f(); };
    switch (d.k) {
      case "new": {
        const create = () => {
          setPrefs((p) => { p.new_template = d.template; p.new_paper = d.paper; });
          const [w, h] = sizeDims(prefs.new_size, prefs.new_landscape);
          const tags = d.tags.split(",").map(normalizeTag).filter(Boolean) as string[];
          close();
          api.createAndOpen(newDoc(d.template, d.paper, w, h), d.title.trim() || "Untitled", d.cover, d.folder, tags);
        };
        return (
          <Modal onClose={close} wide>
            <div className="row"><h2 className="grow">New notebook</h2><button className="btn primary" onClick={create}>Create</button></div>
            <input type="text" autoFocus placeholder="Notebook name" value={d.title} onChange={(e) => setDialog({ ...d, title: e.target.value })} onKeyDown={enter(create)} />
            <div className="section">Cover</div>
            <div className="swatches">{store.COVERS.map((c) => <button key={c} className={"swatch" + (c === d.cover ? " on" : "")} style={{ background: c }} onClick={() => setDialog({ ...d, cover: c })} />)}</div>
            <div className="section">Paper</div>
            <TemplatePicker value={d.template} paper={d.paper} onPick={(t) => setDialog({ ...d, template: t })} />
            <div className="section">Background colour</div>
            <PaperSwatches papers={PAPERS} value={d.paper} bg={paperBg} onPick={(p) => setDialog({ ...d, paper: p })} />
            <div className="section">Paper size</div>
            <div className="row" style={{ flexWrap: "wrap" }}>
              <Seg value={prefs.new_size} options={PAPER_SIZES.map((s) => [s, SIZE_NAMES[s]])} onChange={(s) => setPrefs((p) => (p.new_size = s))} />
              <Seg value={prefs.new_landscape ? "l" : "p"} options={[["p", "Portrait"], ["l", "Landscape"]]} onChange={(v) => setPrefs((p) => (p.new_landscape = v === "l"))} />
            </div>
            <div className="section">Folder</div>
            <Seg value={String(d.folder ?? "none")} options={[["none", "None"], ...lib.folders.map((f) => [String(f.id), f.name] as [string, string])]} onChange={(v) => setDialog({ ...d, folder: v === "none" ? null : Number(v) })} />
            <div className="section">Tags</div>
            <input type="text" placeholder="Comma separated, e.g. School/Physics" value={d.tags} onChange={(e) => setDialog({ ...d, tags: e.target.value })} />
          </Modal>
        );
      }
      case "rename": {
        const ok = () => { if (d.name.trim()) update([d.id], (m) => ({ ...m, title: d.name.trim() })); close(); };
        return (
          <Modal onClose={close}>
            <h2>Rename</h2>
            <input type="text" autoFocus value={d.name} onChange={(e) => setDialog({ ...d, name: e.target.value })} onKeyDown={enter(ok)} />
            <div className="actions"><button className="btn" onClick={close}>Cancel</button><button className="btn primary" onClick={ok}>Save</button></div>
          </Modal>
        );
      }
      case "tags": {
        const all = [...new Set(lib.notebooks.flatMap((m) => m.tags))].sort();
        const ok = () => {
          const t = d.text.split(",").map(normalizeTag).filter(Boolean) as string[];
          update(d.ids, (m) => ({ ...m, tags: d.ids.length > 1 ? [...new Set([...m.tags, ...t])] : t }));
          close();
        };
        return (
          <Modal onClose={close}>
            <h2>Tags</h2>
            <input type="text" autoFocus placeholder="Comma separated, e.g. School/Physics" value={d.text} onChange={(e) => setDialog({ ...d, text: e.target.value })} onKeyDown={enter(ok)} />
            {all.length > 0 && <div className="chips" style={{ justifyContent: "flex-start", marginTop: 10 }}>{all.map((t) => (
              <button key={t} className="chip" onClick={() => setDialog({ ...d, text: d.text.trim() ? d.text.replace(/,?\s*$/, ", ") + t : t })}>#{t}</button>
            ))}</div>}
            <div className="actions"><button className="btn" onClick={close}>Cancel</button><button className="btn primary" onClick={ok}>Save</button></div>
          </Modal>
        );
      }
      case "move":
        return (
          <Modal onClose={close}>
            <h2>Move to folder</h2>
            <div className="menu">
              <button onClick={() => { update(d.ids, (m) => ({ ...m, folder: null })); close(); }}><Icon name="unfiled" size={18} />No folder</button>
              {lib.folders.map((f) => <button key={f.id} onClick={() => { update(d.ids, (m) => ({ ...m, folder: f.id })); close(); }}><Icon name="folder" size={18} />{f.name}</button>)}
            </div>
            <div className="actions"><button className="btn" onClick={close}>Cancel</button></div>
          </Modal>
        );
      case "newFolder":
      case "renameFolder": {
        const ok = () => {
          const n = d.name.trim();
          if (n) {
            if (d.k === "newFolder") setLib({ ...lib, folders: [...lib.folders, { id: Date.now(), name: n }] });
            else setLib({ ...lib, folders: lib.folders.map((f) => (f.id === d.id ? { ...f, name: n } : f)) });
          }
          close();
        };
        return (
          <Modal onClose={close}>
            <h2>{d.k === "newFolder" ? "New folder" : "Rename folder"}</h2>
            <input type="text" autoFocus placeholder="Folder name" value={d.name} onChange={(e) => setDialog({ ...d, name: e.target.value })} onKeyDown={enter(ok)} />
            <div className="actions"><button className="btn" onClick={close}>Cancel</button><button className="btn primary" onClick={ok}>Save</button></div>
          </Modal>
        );
      }
      case "renameTag": {
        const ok = () => {
          const n = normalizeTag(d.name);
          if (n) setLib({ ...lib, notebooks: lib.notebooks.map((m) => ({ ...m, tags: [...new Set(m.tags.map((t) => (tagMatches(t, d.path) ? n + t.slice(d.path.length) : t)))] })) });
          setFilter({ k: "all" });
          close();
        };
        return (
          <Modal onClose={close}>
            <h2>Rename tag</h2>
            <input type="text" autoFocus value={d.name} onChange={(e) => setDialog({ ...d, name: e.target.value })} onKeyDown={enter(ok)} />
            <div className="actions"><button className="btn" onClick={close}>Cancel</button><button className="btn primary" onClick={ok}>Save</button></div>
          </Modal>
        );
      }
      case "confirmDelete":
        return (
          <Modal onClose={close}>
            <h2>Delete forever?</h2>
            <p className="muted">{d.ids.length} notebook{d.ids.length === 1 ? "" : "s"} will be deleted. This can't be undone.</p>
            <div className="actions"><button className="btn" onClick={close}>Cancel</button><button className="btn danger" onClick={() => { setLib(store.deleteForever(lib, d.ids)); setSelected(new Set()); close(); }}>Delete</button></div>
          </Modal>
        );
      case "emptyTrash":
        return (
          <Modal onClose={close}>
            <h2>Empty trash?</h2>
            <p className="muted">Everything in the trash will be deleted. This can't be undone.</p>
            <div className="actions"><button className="btn" onClick={close}>Cancel</button><button className="btn danger" onClick={() => { setLib(store.deleteForever(lib, lib.notebooks.filter((m) => m.trashed_at != null).map((m) => m.id))); close(); }}>Empty trash</button></div>
          </Modal>
        );
      case "settings":
        return (
          <Modal onClose={close}>
            <div className="row"><div className="logo">N</div><h2 className="grow" style={{ margin: 0 }}>Notebook</h2><button className="btn" onClick={close}>Done</button></div>
            <div className="section">Appearance</div>
            <Seg value={prefs.dark_ui ? "d" : "l"} options={[["l", "Light"], ["d", "Dark"]]} onChange={(v) => setPrefs((p) => (p.dark_ui = v === "d"))} />
            <div className="section">Writing</div>
            <Toggle label="Only Apple Pencil writes (fingers scroll)" value={prefs.pencil_only} onChange={(v) => setPrefs((p) => (p.pencil_only = v))} />
            <Toggle label="Two-finger tap to undo, three to redo" value={prefs.two_finger_undo} onChange={(v) => setPrefs((p) => (p.two_finger_undo = v))} />
            <Toggle label="Hold to draw shapes" value={prefs.shape_hold} onChange={(v) => setPrefs((p) => (p.shape_hold = v))} />
            <div className="section">Backup</div>
            <p className="muted small" style={{ margin: "0 0 8px" }}>
              Notebooks are saved automatically in this browser on this device. Download a backup now and then to keep a copy, or to move your notebooks to another browser or device.
              {prefs.last_backup ? ` Last backup: ${new Date(prefs.last_backup).toLocaleDateString()}.` : ""}
            </p>
            <div className="row">
              <button className="btn primary" onClick={() => { store.downloadBackup(); setPrefs((p) => (p.last_backup = Date.now())); }}><span className="row"><Icon name="download" size={18} />Download backup</span></button>
              <button className="btn" onClick={restore}><span className="row"><Icon name="upload" size={18} />Restore…</span></button>
            </div>
          </Modal>
        );
    }
  }
}

export type { Rgba };
