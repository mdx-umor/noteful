import { useState } from "react";
import { Icon } from "../ui/icons";
import { PageThumb, Popover, rectOf, Seg, type Anchor } from "../ui/ui";
import type { DocModel } from "./doc";
import type { EditDialog } from "./Dialogs";

type Tab = "pages" | "bookmarks" | "outline" | "layers";

function PageMenu({ model, i, at, onClose, onEdit }: { model: DocModel; i: number; at: Anchor; onClose: () => void; onEdit: (d: EditDialog) => void }) {
  const p = model.page(i);
  const act = (f: () => void) => () => { f(); onClose(); };
  return (
    <Popover anchor={at} side="left" width={230} onClose={onClose}>
      <div className="menu">
        <div className="menu-title">Page {i + 1}</div>
        <button onClick={act(() => model.insertPage(i))}>Insert page after</button>
        <button onClick={act(() => model.duplicatePage(i))}>Duplicate</button>
        <button disabled={i === 0} onClick={act(() => model.movePage(i, i - 1))}>Move up</button>
        <button disabled={i === model.pages.length - 1} onClick={act(() => model.movePage(i, i + 1))}>Move down</button>
        <button onClick={act(() => onEdit({ k: "bookmark", page: i, name: p.bookmark ?? `Page ${i + 1}` }))}>{p.bookmark ? "Rename bookmark" : "Bookmark"}</button>
        <button onClick={act(() => { const id = model.addOutline(i); onEdit({ k: "outline", id, title: model.doc.outline.find((o) => o.id === id)?.title ?? "" }); })}>Add to outline</button>
        <button onClick={act(() => model.clearPage(i))}>Clear page</button>
        <button className="danger-text" onClick={act(() => model.deletePage(i))}>Delete page</button>
      </div>
    </Popover>
  );
}

export function SidePanel({ model, night, onEdit }: { model: DocModel; night: boolean; onEdit: (d: EditDialog) => void }) {
  const [tab, setTab] = useState<Tab>("pages");
  const [menu, setMenu] = useState<{ i: number; at: Anchor } | null>(null);
  const [drag, setDrag] = useState<number | null>(null);
  return (
    <aside className="side-panel">
      <div className="tabs">
        <Seg value={tab} onChange={setTab} options={[
          ["pages", <Icon name="grid" size={18} key="g" />], ["bookmarks", <Icon name="bookmark" size={18} key="b" />],
          ["outline", <Icon name="outline" size={18} key="o" />], ["layers", <Icon name="layers" size={18} key="l" />],
        ]} />
      </div>
      <div className="side-scroll">
        {tab === "pages" && (
          <>
            <div className="thumbs">
              {model.pages.map((p, i) => (
                <button key={p.id} className={"pthumb" + (i === model.current ? " on" : "")} draggable
                  onDragStart={() => setDrag(i)} onDragOver={(e) => e.preventDefault()}
                  onDrop={() => { if (drag !== null) model.movePage(drag, i); setDrag(null); }}
                  onClick={() => model.goTo(i)}
                  onContextMenu={(e) => { e.preventDefault(); setMenu({ i, at: rectOf(e.currentTarget) }); }}>
                  <PageThumb page={p} width={118} night={night} />
                  {p.bookmark && <span className="ribbon"><Icon name="bookmark" size={16} /></span>}
                  <span className="row" style={{ gap: 2 }}>{i + 1}
                    <span className="icon-btn" style={{ width: 22, height: 22 }} onClick={(e) => { e.stopPropagation(); setMenu({ i, at: rectOf(e.currentTarget) }); }}><Icon name="chevron" size={14} /></span>
                  </span>
                </button>
              ))}
            </div>
            <button className="btn" style={{ width: "100%", marginTop: 14 }} onClick={() => model.insertPage(model.pages.length - 1)}>+ Add page</button>
            <p className="muted small">Drag pages to reorder. Tap ⌄ for page actions.</p>
          </>
        )}
        {tab === "bookmarks" && (
          <>
            <button className="btn" style={{ width: "100%" }} onClick={() => onEdit({ k: "bookmark", page: model.current, name: model.page(model.current).bookmark ?? `Page ${model.current + 1}` })}>+ Bookmark this page</button>
            <div className="menu" style={{ marginTop: 8 }}>
              {model.pages.map((p, i) => p.bookmark ? (
                <button key={p.id} className={i === model.current ? "active" : ""} onClick={() => model.goTo(i)}
                  onContextMenu={(e) => { e.preventDefault(); onEdit({ k: "bookmark", page: i, name: p.bookmark! }); }}>
                  <span style={{ color: "#ff3b30" }}><Icon name="bookmark" size={18} /></span>
                  <span className="grow" style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{p.bookmark}</span>
                  <span className="muted small">{i + 1}</span>
                </button>
              ) : null)}
            </div>
            {!model.pages.some((p) => p.bookmark) && <p className="muted small">No bookmarks yet.</p>}
          </>
        )}
        {tab === "outline" && <OutlineTab model={model} onEdit={onEdit} />}
        {tab === "layers" && <LayersTab model={model} onEdit={onEdit} night={night} />}
      </div>
      {menu && <PageMenu model={model} i={menu.i} at={menu.at} onClose={() => setMenu(null)} onEdit={onEdit} />}
    </aside>
  );
}

function OutlineTab({ model, onEdit }: { model: DocModel; onEdit: (d: EditDialog) => void }) {
  const [menu, setMenu] = useState<{ k: number; at: Anchor } | null>(null);
  const idx = (pid: number) => model.pages.findIndex((p) => p.id === pid);
  const cur = model.doc.outline.filter((e) => idx(e.page) >= 0 && idx(e.page) <= model.current).sort((a, b) => idx(b.page) - idx(a.page))[0]?.id;
  return (
    <>
      <button className="btn" style={{ width: "100%" }} onClick={() => { const id = model.addOutline(); onEdit({ k: "outline", id, title: model.doc.outline.find((o) => o.id === id)?.title ?? "" }); }}>+ Add entry for this page</button>
      <div className="menu" style={{ marginTop: 8 }}>
        {model.doc.outline.map((e, k) => (
          <button key={e.id} style={{ paddingLeft: 12 + e.level * 16, fontWeight: e.id === cur ? 700 : 400, color: e.id === cur ? "var(--accent)" : undefined }}
            onClick={() => { const i = idx(e.page); if (i >= 0) model.goTo(i); }}
            onContextMenu={(ev) => { ev.preventDefault(); setMenu({ k, at: rectOf(ev.currentTarget) }); }}>
            <span className="grow" style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{e.title}</span>
            <span className="muted small">{idx(e.page) >= 0 ? idx(e.page) + 1 : "–"}</span>
            <span className="icon-btn" style={{ width: 24, height: 24 }} onClick={(ev) => { ev.stopPropagation(); setMenu({ k, at: rectOf(ev.currentTarget) }); }}><Icon name="more" size={16} /></span>
          </button>
        ))}
      </div>
      {!model.doc.outline.length && <p className="muted small">No outline yet. Add entries to build a table of contents.</p>}
      {menu && (
        <Popover anchor={menu.at} side="left" width={200} onClose={() => setMenu(null)}>
          <div className="menu">
            {(["Rename", "Indent", "Outdent", "Delete"] as const).map((a) => (
              <button key={a} className={a === "Delete" ? "danger-text" : ""} onClick={() => {
                const e = model.doc.outline[menu.k];
                setMenu(null);
                if (a === "Rename") onEdit({ k: "outline", id: e.id, title: e.title });
                else model.updateOutline((o) => a === "Delete" ? o.filter((x) => x.id !== e.id) : o.map((x) => x.id === e.id ? { ...x, level: Math.max(0, Math.min(4, x.level + (a === "Indent" ? 1 : -1))) } : x));
              }}>{a}</button>
            ))}
          </div>
        </Popover>
      )}
    </>
  );
}

function LayersTab({ model, onEdit, night }: { model: DocModel; onEdit: (d: EditDialog) => void; night: boolean }) {
  const [menu, setMenu] = useState<{ id: number; at: Anchor } | null>(null);
  const i = model.current;
  const page = model.page(i);
  const active = model.activeLayerId(i);
  void night;
  return (
    <>
      <div className="row" style={{ marginBottom: 8 }}>
        <b className="grow">Page {i + 1}</b>
        <button className="btn" onClick={() => model.addLayer(i)}>+ Layer</button>
      </div>
      {[...page.layers].reverse().map((l) => {
        const count = page.items.filter((it) => it.layer === l.id).length;
        return (
          <div key={l.id} className={"layer-row" + (l.id === active ? " on" : "")}>
            <button className="grow" style={{ textAlign: "left", color: l.id === active ? "var(--accent)" : undefined, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
              onClick={() => model.setActiveLayer(i, l.id)} title="Draw on this layer">
              {l.id === active && <Icon name="pen" size={14} />} {l.name} <span className="muted small">{count}</span>
            </button>
            <button className={"icon-btn" + (l.locked ? " active" : "")} title={l.locked ? "Unlock" : "Lock"} onClick={() => model.updateLayer(i, l.id, (x) => ({ ...x, locked: !x.locked }))}><Icon name={l.locked ? "lock" : "unlock"} size={18} /></button>
            <button className={"icon-btn" + (!l.visible ? " active" : "")} title={l.visible ? "Hide" : "Show"} onClick={() => model.updateLayer(i, l.id, (x) => ({ ...x, visible: !x.visible }))}><Icon name={l.visible ? "eye" : "eyeOff"} size={18} /></button>
            <button className="icon-btn" onClick={(e) => setMenu({ id: l.id, at: rectOf(e.currentTarget) })}><Icon name="more" size={18} /></button>
          </div>
        );
      })}
      <p className="muted small">Tap a layer to write on it. Hide a layer to make an answer sheet; lock it to protect it.</p>
      {menu && (() => {
        const pos = page.layers.findIndex((l) => l.id === menu.id);
        const l = page.layers[pos];
        const act = (f: () => void) => () => { f(); setMenu(null); };
        return (
          <Popover anchor={menu.at} side="left" width={210} onClose={() => setMenu(null)}>
            <div className="menu">
              <button onClick={act(() => onEdit({ k: "layer", page: i, id: l.id, name: l.name }))}>Rename</button>
              <button onClick={act(() => model.duplicateLayer(i, l.id))}>Duplicate</button>
              <button disabled={pos === 0} onClick={act(() => model.mergeDown(i, l.id))}>Merge down</button>
              <button disabled={pos === page.layers.length - 1} onClick={act(() => model.moveLayer(i, l.id, true))}>Move up</button>
              <button disabled={pos === 0} onClick={act(() => model.moveLayer(i, l.id, false))}>Move down</button>
              <button className="danger-text" disabled={page.layers.length < 2} onClick={act(() => model.deleteLayer(i, l.id))}>Delete layer</button>
            </div>
          </Popover>
        );
      })()}
    </>
  );
}

export function Overview({ model, night, onClose }: { model: DocModel; night: boolean; onClose: () => void }) {
  const [menu, setMenu] = useState<{ i: number; at: Anchor } | null>(null);
  const [drag, setDrag] = useState<number | null>(null);
  return (
    <div className="overview">
      <div className="lib-top">
        <h1>{model.pages.length} pages</h1>
        <span className="grow" />
        <button className="btn" onClick={() => model.insertPage(model.pages.length - 1)}>+ Page</button>
        <button className="btn primary" onClick={onClose}>Done</button>
      </div>
      <div className="grid">
        {model.pages.map((p, i) => (
          <button key={p.id} className={"pthumb" + (i === model.current ? " on" : "")} draggable
            onDragStart={() => setDrag(i)} onDragOver={(e) => e.preventDefault()} onDrop={() => { if (drag !== null) model.movePage(drag, i); setDrag(null); }}
            onClick={() => { model.goTo(i); onClose(); }}
            onContextMenu={(e) => { e.preventDefault(); setMenu({ i, at: rectOf(e.currentTarget) }); }}>
            <PageThumb page={p} width={150} night={night} />
            {p.bookmark && <span className="ribbon"><Icon name="bookmark" size={16} /></span>}
            <span className="row" style={{ gap: 2 }}>{i + 1}
              <span className="icon-btn" style={{ width: 22, height: 22 }} onClick={(e) => { e.stopPropagation(); setMenu({ i, at: rectOf(e.currentTarget) }); }}><Icon name="chevron" size={14} /></span>
            </span>
          </button>
        ))}
      </div>
      {menu && <PageMenu model={model} i={menu.i} at={menu.at} onClose={() => setMenu(null)} onEdit={() => {}} />}
    </div>
  );
}
