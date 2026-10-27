import { useCallback, useEffect, useLayoutEffect, useReducer, useRef, useState } from "react";
import type { Id, Item, Link, Rgba } from "../core/model";
import { css, newItem, sameColor } from "../core/model";
import type { Pt } from "../core/geom";
import { LINE_HEIGHT, FONT_FAMILY } from "../core/text";
import { importFile, pickFile } from "../core/pictures";
import type { AppApi } from "../App";
import { Icon } from "../ui/icons";
import { rectOf, type Anchor } from "../ui/ui";
import { Board, type Tool } from "./board";
import type { DocModel } from "./doc";
import { Panels, type PanelKind } from "./Panels";
import { SidePanel, Overview } from "./SidePanel";
import { EditDialogs, type EditDialog } from "./Dialogs";
import * as store from "../store";

interface TextEdit { page: number; id: Id | null; x: number; y: number; w: number; text: string; size: number; color: Rgba }

const TOOLS: [Tool, string, string, string][] = [
  ["pen", "pen", "Pen (F)", "Pen"],
  ["highlighter", "highlighter", "Highlighter (G)", "Highlighter"],
  ["tape", "tape", "Tape (K): hide notes to test yourself", "Tape"],
  ["eraser", "eraser", "Eraser (E)", "Eraser"],
  ["lasso", "lasso", "Lasso (A)", "Lasso"],
  ["hand", "hand", "Hand (H): scroll, follow links", "Hand"],
  ["laser", "laser", "Laser pointer", "Laser"],
  ["shape", "shapes", "Shapes (S)", "Shapes"],
  ["text", "text", "Text (T)", "Text"],
];

export function EditorView({ model, api, onClose }: { model: DocModel; api: AppApi; onClose: () => void }) {
  const { prefs, setPrefs } = api;
  const prefsRef = useRef(prefs);
  prefsRef.current = prefs;
  const wrapRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const [board, setBoard] = useState<Board | null>(null);
  const [, bump] = useReducer((x: number) => x + 1, 0);
  const [tool, setToolState] = useState<Tool>("pen");
  const [panel, setPanel] = useState<{ kind: PanelKind; at: Anchor } | null>(null);
  const [side, setSide] = useState(false);
  const [overview, setOverview] = useState(false);
  const [dialog, setDialog] = useState<EditDialog | null>(null);
  const [text, setText] = useState<TextEdit | null>(null);
  const [timer, setTimer] = useState<{ since: number | null; acc: number }>({ since: null, acc: 0 });
  const [, setNow] = useState(0);
  const textRef = useRef<TextEdit | null>(null);
  textRef.current = text;
  const viewChanged = useRef<() => void>(() => {});

  // Re-render the chrome (at most once a frame) when the document or view changes.
  useEffect(() => {
    let raf = 0;
    const f = () => { if (!raf) raf = requestAnimationFrame(() => { raf = 0; bump(); }); };
    const un = model.subscribe(f);
    viewChanged.current = f;
    return () => { un(); cancelAnimationFrame(raf); };
  }, [model]);

  const finishText = useCallback(() => {
    const te = textRef.current;
    if (!te) return;
    setText(null);
    board?.setHidden([]);
    const trimmed = te.text.replace(/\s+$/, "");
    const page = model.page(te.page);
    if (!page) return;
    if (te.id !== null) {
      const it = page.items.find((i) => i.id === te.id);
      if (!it) return;
      if (!trimmed) model.replaceItems(te.page, page.items.filter((i) => i.id !== te.id));
      else {
        const next: Item = { ...it, content: { Text: { x: te.x, y: te.y, w: te.w, text: trimmed, size: te.size, color: te.color } } };
        if (JSON.stringify(next.content) !== JSON.stringify(it.content)) model.replaceItems(te.page, page.items.map((i) => (i.id === te.id ? next : i)));
      }
    } else if (trimmed) {
      model.addItem(te.page, newItem({ Text: { x: te.x, y: te.y, w: te.w, text: trimmed, size: te.size, color: te.color } }));
    }
  }, [board, model]);

  const startText = useCallback((page: number, at: Pt, existing: Item | null) => {
    finishText();
    const p = prefsRef.current;
    if (existing && "Text" in existing.content) {
      const t = existing.content.Text;
      setText({ page, id: existing.id, x: t.x, y: t.y, w: t.w, text: t.text, size: t.size, color: t.color });
      boardRef.current?.setHidden([existing.id]);
      return;
    }
    const pw = model.page(page).w;
    const w = Math.max(80, Math.min(380, pw - at[0] - 28));
    setText({ page, id: null, x: at[0], y: at[1] - (p.text_size * LINE_HEIGHT) / 2, w, text: "", size: p.text_size, color: p.text_color });
  }, [finishText, model]);

  const boardRef = useRef<Board | null>(null);
  const setTool = useCallback((t: Tool) => {
    finishText();
    boardRef.current?.setTool(t);
    setToolState(t);
  }, [finishText]);
  const startTextRef = useRef(startText);
  startTextRef.current = startText;
  const setToolRef = useRef(setTool);
  setToolRef.current = setTool;

  useLayoutEffect(() => {
    const b = new Board(wrapRef.current!, scrollRef.current!, contentRef.current!, model, () => prefsRef.current, {
      onText: (page, at, existing) => startTextRef.current(page, at, existing),
      onChange: () => viewChanged.current(),
      onToolSwitch: (t) => setToolRef.current(t),
      onPencil: () => {
        setPrefs((p) => { p.pencil_seen = true; p.pencil_only = true; });
        api.toast("Apple Pencil detected: write with the Pencil, scroll and zoom with fingers");
      },
    });
    boardRef.current = b;
    setBoard(b);
    return () => { b.destroy(); boardRef.current = null; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [model]);

  useEffect(() => { board?.kick(); }, [board, prefs.night_mode]);

  useEffect(() => {
    if (timer.since === null) return;
    const t = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(t);
  }, [timer.since]);

  const openPanel = (kind: PanelKind, el: Element) => setPanel(panel?.kind === kind ? null : { kind, at: rectOf(el) });

  const pickImport = async (asPages: boolean) => {
    const file = await pickFile(asPages ? "application/pdf,image/*" : "image/*");
    if (!file) return;
    try {
      api.toast("Importing " + file.name + "…");
      const imp = await importFile(file, api.toast);
      if (asPages || imp.kind === "pdf") model.insertPicturePages(imp);
      else if (board) model.insertPicture(imp, board.visibleCenter(model.current));
      if (!asPages) setTool("lasso");
    } catch (e) {
      api.toast("Import failed: " + (e instanceof Error ? e.message : String(e)));
    }
  };

  // ---------- keyboard ----------
  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || dialog || textRef.current) return;
      const cmd = e.metaKey || e.ctrlKey;
      const key = e.key.toLowerCase();
      if (cmd && key === "z") { e.preventDefault(); if (e.shiftKey) model.redo(); else model.undo(); }
      else if (cmd && key === "y") { e.preventDefault(); model.redo(); }
      else if (cmd && key === "c") model.copySelection();
      else if (cmd && key === "x") model.cutSelection();
      else if (cmd && key === "v") { if (board) model.paste(board.visibleCenter(model.current)); setTool("lasso"); }
      else if (cmd && key === "d") { e.preventDefault(); model.duplicateSelection(); }
      else if (cmd && key === "a") { e.preventDefault(); model.selectAll(); setTool("lasso"); }
      else if (cmd && key === "f") { e.preventDefault(); setDialog({ k: "find", q: "" }); }
      else if (cmd && key === "g") { e.preventDefault(); setDialog({ k: "goto", text: String(model.current + 1) }); }
      else if (cmd && (key === "=" || key === "+")) { e.preventDefault(); board?.setZoom(board.zoom * 1.25); }
      else if (cmd && key === "-") { e.preventDefault(); board?.setZoom(board.zoom / 1.25); }
      else if (cmd && key === "0") { e.preventDefault(); board?.fitWidth(); }
      else if (cmd && key === "[") model.goBack();
      else if (!cmd) {
        if (key === "backspace" || key === "delete") model.deleteSelection();
        else if (key === "escape") { model.selection = null; model.notify(); setPanel(null); }
        else if (key === "pagedown" || key === "arrowdown") model.goTo(model.current + 1);
        else if (key === "pageup" || key === "arrowup") model.goTo(model.current - 1);
        else if (/^[1-9]$/.test(key)) { const c = prefs.swatches[Number(key) - 1]; if (c) { setPrefs((p) => (p.pen_color = c)); setTool("pen"); } }
        else if (key === "[" || key === "]") {
          const p = model.page(model.current), cur = model.activeLayerId(model.current);
          const pos = p.layers.findIndex((l) => l.id === cur);
          const to = Math.max(0, Math.min(p.layers.length - 1, pos + (key === "]" ? 1 : -1)));
          model.setActiveLayer(model.current, p.layers[to].id);
          api.toast("Drawing on " + p.layers[to].name);
        } else {
          const map: Record<string, Tool> = { f: "pen", p: "pen", g: "highlighter", k: "tape", e: "eraser", a: "lasso", l: "lasso", h: "hand", s: "shape", t: "text" };
          if (map[key]) setTool(map[key]);
        }
      }
    };
    addEventListener("keydown", k);
    return () => removeEventListener("keydown", k);
  }, [board, model, dialog, prefs.swatches, setPrefs, setTool, api]);

  const toolColor = (): Rgba => ({ highlighter: prefs.hl_color, tape: prefs.tape_color, shape: prefs.shape_color, text: prefs.text_color, laser: prefs.laser_color } as Partial<Record<Tool, Rgba>>)[tool] ?? prefs.pen_color;
  const setToolColor = (c: Rgba) => setPrefs((p) => {
    if (tool === "highlighter") p.hl_color = c;
    else if (tool === "tape") p.tape_color = c;
    else if (tool === "shape") p.shape_color = c;
    else if (tool === "text") p.text_color = c;
    else if (tool === "laser") p.laser_color = c;
    else p.pen_color = c;
  });
  const favs = tool === "highlighter" ? prefs.hl_swatches : prefs.swatches;
  const visibleTool = (name: string) => !prefs.hidden_tools.includes(name);
  const zoomPct = board ? Math.round((board.zoom / board.fitZoom()) * 100) : 100;
  const selRect = board?.selectionViewRect() ?? null;
  const seconds = Math.floor(timer.acc + (timer.since ? (Date.now() - timer.since) / 1000 : 0));

  let textStyle: React.CSSProperties | null = null;
  if (text && board) {
    const [vx, vy] = board.pageToView(text.page, [text.x, text.y]);
    const z = board.zoom;
    textStyle = {
      left: vx - 2, top: vy - 2, width: text.w * z + 4, minHeight: text.size * LINE_HEIGHT * z + 4,
      fontSize: text.size * z, color: css(text.color), fontFamily: FONT_FAMILY, lineHeight: LINE_HEIGHT,
    };
  }

  return (
    <div className="editor">
      <div className="ed-top">
        <button className="icon-btn" title="Library" onClick={() => { finishText(); onClose(); }}><Icon name="back" /></button>
        <button className="ed-title" onClick={() => setDialog({ k: "rename", name: model.title })}>{model.title}</button>
        <span className="muted small">{model.current + 1} / {model.pages.length}</span>
        {model.page(model.current)?.bookmark && <span className="muted small row"><Icon name="bookmark" size={14} />{model.page(model.current).bookmark}</span>}
        <span className="grow" />
        <button className={"icon-btn" + (side ? " active" : "")} title="Pages, bookmarks, outline, layers" onClick={() => setSide(!side)}><Icon name="sidebar" /></button>
        <button className={"icon-btn" + (panel?.kind === "more" ? " active" : "")} title="More" onClick={(e) => openPanel("more", e.currentTarget)}><Icon name="more" /></button>
      </div>
      <div className="ed-body">
        <nav className="toolbar">
          <button className="icon-btn" title="All pages" onClick={() => setOverview(true)}><Icon name="grid" /></button>
          <div className="sep" />
          {TOOLS.filter(([t, , , name]) => visibleTool(name) || t === tool).map(([t, icon, tip]) => (
            <button key={t} className={"icon-btn" + (tool === t ? " active" : "")} title={tip}
              onClick={(e) => { if (tool === t) openPanel(("tool:" + t) as PanelKind, e.currentTarget); else { setTool(t); setPanel(null); } }}
              onContextMenu={(e) => { e.preventDefault(); setTool(t); openPanel(("tool:" + t) as PanelKind, e.currentTarget); }}>
              <Icon name={icon} />
            </button>
          ))}
          <div className="sep" />
          {visibleTool("Stickers") && <button className={"icon-btn" + (panel?.kind === "stickers" ? " active" : "")} title="Stickers" onClick={(e) => openPanel("stickers", e.currentTarget)}><Icon name="sticker" /></button>}
          {visibleTool("Insert") && <button className={"icon-btn" + (panel?.kind === "insert" ? " active" : "")} title="Insert" onClick={(e) => openPanel("insert", e.currentTarget)}><Icon name="plusCircle" /></button>}
          <button className="icon-btn" title="Undo (⌘Z)" disabled={!model.canUndo()} onClick={() => model.undo()}><Icon name="undo" /></button>
          <button className="icon-btn" title="Redo (⇧⌘Z)" disabled={!model.canRedo()} onClick={() => model.redo()}><Icon name="redo" /></button>
          <div className="sep" />
          <button className="fav on" title="Colour" onClick={(e) => openPanel(tool === "tape" ? "tool:tape" : "color", e.currentTarget)}><i style={{ background: css(toolColor()) }} /></button>
          {(tool === "pen" || tool === "highlighter") && favs.slice(0, 5).map((c, i) => (
            <button key={i} className={"fav" + (sameColor(c, toolColor()) ? " on" : "")} onClick={() => setToolColor(c)}><i style={{ background: css(c) }} /></button>
          ))}
        </nav>
        <div className="board-wrap" ref={wrapRef}>
          <div className={"board" + (prefs.pencil_only ? "" : " finger")} ref={scrollRef}>
            <div className="board-content" ref={contentRef} />
          </div>

          {/* Bottom-left: back chip after a link, page number (tap = go to page). */}
          <div className="floating row" style={{ left: 14, bottom: "calc(14px + env(safe-area-inset-bottom))" }}>
            {model.backStack.length > 0 && <button className="chip-btn" onClick={() => model.goBack()}>↶ Page {model.backStack[model.backStack.length - 1] + 1}</button>}
            <button className="chip-btn" onClick={() => setDialog({ k: "goto", text: String(model.current + 1) })}>{model.current + 1} of {model.pages.length}</button>
          </div>

          <div className="floating zoom-ctl">
            {board?.ruler && (
              <>
                <button className="icon-btn small" onClick={() => board.rotateRuler(-15)}>−15°</button>
                <button className="icon-btn small" onClick={() => board.rotateRuler(15)}>+15°</button>
              </>
            )}
            <button className={"icon-btn" + (board?.ruler ? " active" : "")} title="Ruler" onClick={() => { board?.toggleRuler(); bump(); }}><Icon name="ruler" /></button>
            <button className="icon-btn" title="Zoom in" onClick={() => board?.setZoom(board.zoom * 1.25)}><Icon name="zoomIn" /></button>
            <button className="pct" title="Fit width" onClick={() => board?.fitWidth()}>{zoomPct}%</button>
            <button className="icon-btn" title="Zoom out" onClick={() => board?.setZoom(board.zoom / 1.25)}><Icon name="zoomOut" /></button>
          </div>

          {prefs.show_timer && (
            <div className="floating timer">
              <Icon name="timer" size={18} />
              <span>{String(Math.floor(seconds / 3600)).padStart(2, "0")}:{String(Math.floor(seconds / 60) % 60).padStart(2, "0")}:{String(seconds % 60).padStart(2, "0")}</span>
              <button className="btn" onClick={() => setTimer((t) => (t.since ? { since: null, acc: t.acc + (Date.now() - t.since) / 1000 } : { ...t, since: Date.now() }))}>{timer.since ? "Pause" : "Start"}</button>
              <button className="btn" onClick={() => setTimer({ since: null, acc: 0 })}>Reset</button>
            </div>
          )}

          {selRect && !text && !dialog && (
            <SelectionMenu model={model} rect={selRect} api={api} onPanel={(kind, el) => setPanel({ kind, at: rectOf(el) })} onLink={() => {
              const l = model.selectedItems()[0]?.link;
              const page = l && "Page" in l ? String(model.pages.findIndex((p) => p.id === l.Page) + 1) : "";
              setDialog({ k: "link", url: l && "Url" in l ? l.Url : "", page, web: !l || "Url" in l });
            }} />
          )}

          {text && textStyle && (
            <>
              <div className="floating text-bar sel-menu" style={{ left: textStyle.left as number, top: Math.max(4, (textStyle.top as number) - 50) }}>
                <button className="btn" onClick={() => setText({ ...text, size: Math.max(8, text.size - 2) })}>A−</button>
                <span className="small" style={{ minWidth: 26, textAlign: "center" }}>{Math.round(text.size)}</span>
                <button className="btn" onClick={() => setText({ ...text, size: Math.min(96, text.size + 2) })}>A+</button>
                <span className="vsep" />
                {prefs.swatches.slice(0, 6).map((c, i) => (
                  <button key={i} className="dot" onClick={() => { setText({ ...text, color: c }); setPrefs((p) => (p.text_color = c)); }}>
                    <i style={{ background: css(c), boxShadow: sameColor(c, text.color) ? "0 0 0 2px var(--accent)" : undefined }} />
                  </button>
                ))}
                <span className="vsep" />
                <button className="btn primary" onClick={finishText}>Done</button>
              </div>
              <textarea className="text-edit" autoFocus style={textStyle} value={text.text} placeholder="Type here"
                onChange={(e) => setText({ ...text, text: e.target.value })}
                onKeyDown={(e) => { if (e.key === "Escape") finishText(); }}
                ref={(el) => { if (el) { el.style.height = "auto"; el.style.height = el.scrollHeight + "px"; } }} />
            </>
          )}
        </div>
        {side && <SidePanel model={model} night={prefs.night_mode} onEdit={setDialog} />}
      </div>

      {panel && board && (
        <Panels kind={panel.kind} at={panel.at} model={model} board={board} api={api} tool={tool}
          onClose={() => setPanel(null)}
          setPanel={(kind) => setPanel(kind ? { kind, at: panel.at } : null)}
          setTool={setTool}
          openDialog={(d) => { setPanel(null); setDialog(d); }}
          onImport={(asPages) => { setPanel(null); pickImport(asPages); }}
          onExportPdf={() => { setPanel(null); store.exportNotebookPdf(model.metaId, model.title, model.doc); }}
          onOverview={() => { setPanel(null); setOverview(true); }}
        />
      )}
      {overview && <Overview model={model} night={prefs.night_mode} onClose={() => setOverview(false)} />}
      {dialog && <EditDialogs dialog={dialog} model={model} onClose={() => setDialog(null)} setDialog={setDialog} toast={api.toast} />}
    </div>
  );
}

function SelectionMenu({ model, rect, api, onPanel, onLink }: {
  model: DocModel; rect: { x: number; y: number; w: number; h: number }; api: AppApi;
  onPanel: (k: PanelKind, el: Element) => void; onLink: () => void;
}) {
  const [more, setMore] = useState<Anchor | null>(null);
  const top = Math.max(8, rect.y - 62);
  const items = model.selectedItems();
  const hasShape = items.some((i) => "Shape" in i.content);
  const link: Link | null | undefined = items[0]?.link;
  return (
    <>
      <div className="floating sel-menu" style={{ left: Math.max(8, rect.x + rect.w / 2 - 190), top }}>
        {api.prefs.swatches.slice(0, 6).map((c, i) => (
          <button key={i} className="dot" title="Recolour" onClick={() => model.mapSelection((it) => ({ ...it, content: recolor(it, c) }))}><i style={{ background: css(c) }} /></button>
        ))}
        <span className="vsep" />
        <button className="icon-btn" title="Copy" onClick={() => { model.copySelection(); api.toast("Copied"); }}><Icon name="copy" /></button>
        <button className="icon-btn" title="Cut" onClick={() => model.cutSelection()}><Icon name="cut" /></button>
        <button className="icon-btn" title="Duplicate" onClick={() => model.duplicateSelection()}><Icon name="duplicate" /></button>
        <button className="icon-btn" title="Delete" onClick={() => model.deleteSelection()}><Icon name="trash" /></button>
        <span className="vsep" />
        <button className="btn" onClick={(e) => setMore(rectOf(e.currentTarget))}>More ▾</button>
      </div>
      {more && (
        <div className="scrim" onPointerDown={() => setMore(null)}>
          <div className="popover menu" style={{ left: Math.min(innerWidth - 250, more.x), top: more.y + more.h + 6, width: 240 }} onPointerDown={(e) => e.stopPropagation()}>
            <button disabled={!hasShape} onClick={(e) => { setMore(null); onPanel("shapeStyle", e.currentTarget); }}><Icon name="shapes" size={18} />Shape style…</button>
            <button onClick={() => { setMore(null); onLink(); }}><Icon name="link" size={18} />Link…</button>
            {link && <button onClick={() => { setMore(null); model.followLink(link); }}><Icon name="chevronRight" size={18} />Follow link</button>}
            <button onClick={() => {
              setMore(null);
              const st = model.selectionAsSticker();
              if (st) { api.setPrefs((p) => (p.stickers = [st, ...p.stickers].slice(0, 40))); api.toast("Saved to Stickers"); }
            }}><Icon name="sticker" size={18} />Save as sticker</button>
            <hr />
            <button onClick={() => { setMore(null); model.reorderSelection(true); }}>Bring to front</button>
            <button onClick={() => { setMore(null); model.reorderSelection(false); }}>Send to back</button>
            {model.page(model.selection?.page ?? 0)?.layers.length > 1 && <button onClick={(e) => { setMore(null); onPanel("moveLayer", e.currentTarget); }}><Icon name="layers" size={18} />Move to layer…</button>}
          </div>
        </div>
      )}
    </>
  );
}

function recolor(it: Item, c: Rgba): Item["content"] {
  const x = it.content;
  if ("Stroke" in x) return { Stroke: { ...x.Stroke, color: c } };
  if ("Shape" in x) return { Shape: { ...x.Shape, color: c, fill: x.Shape.fill ? c : x.Shape.fill } };
  if ("Text" in x) return { Text: { ...x.Text, color: c } };
  if ("Tape" in x) return { Tape: { ...x.Tape, color: c } };
  return x;
}
