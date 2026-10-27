import { useEffect, useRef } from "react";
import type { PenKind, Rgba, Shape, ShapeKind } from "../core/model";
import { fromHex, mm, newItem, PAPERS, PAPER_SIZES, paperBg, SIZE_NAMES, sizeDims } from "../core/model";
import { drawItem } from "../core/render";
import { unionBBox } from "../core/geom";
import { PRESETS, presetItem, sampleStickers } from "../core/presets";
import type { AppApi } from "../App";
import { HL_SIZES_MM, PALETTE, PEN_SIZES_MM, TAPE_COLORS, TAPE_SIZES_MM, TOOL_NAMES, type Prefs } from "../prefs";
import { Icon } from "../ui/icons";
import { PaperSwatches, Popover, Seg, SizeRow, Swatches, TemplatePicker, Toggle, type Anchor } from "../ui/ui";
import type { Board, Tool } from "./board";
import type { DocModel } from "./doc";
import type { EditDialog } from "./Dialogs";

export type PanelKind =
  | `tool:${Tool}` | "color" | "insert" | "shapes" | "stickers" | "more" | "template" | "shapeStyle" | "moveLayer" | "customize";

interface Props {
  kind: PanelKind; at: Anchor; model: DocModel; board: Board; api: AppApi; tool: Tool;
  onClose: () => void; setPanel: (k: PanelKind | null) => void; setTool: (t: Tool) => void;
  openDialog: (d: EditDialog) => void; onImport: (asPages: boolean) => void; onExportPdf: () => void; onOverview: () => void;
}

/** Draws items into a small canvas (pen preview, shapes, stickers). */
function ItemsPreview({ items, w, h, pad = 6 }: { items: ReturnType<typeof newItem>[]; w: number; h: number; pad?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current!;
    const dpr = devicePixelRatio || 1;
    c.width = w * dpr;
    c.height = h * dpr;
    const ctx = c.getContext("2d")!;
    const b = unionBBox(items);
    if (!b) return;
    const k = Math.min((w - pad * 2) / Math.max(1, b.w), (h - pad * 2) / Math.max(1, b.h), 1.6);
    ctx.setTransform(k * dpr, 0, 0, k * dpr, (w / 2 - (b.x + b.w / 2) * k) * dpr, (h / 2 - (b.y + b.h / 2) * k) * dpr);
    for (const it of items) drawItem(ctx, it, { paper: "White" });
  }, [items, w, h, pad]);
  return <canvas ref={ref} style={{ width: w, height: h }} />;
}

function penPreview(p: Prefs, hl: boolean) {
  const pts = Array.from({ length: 61 }, (_, i) => { const t = i / 60; return { x: 10 + t * 260, y: 30 - Math.sin(t * 6) * 14, p: 0.25 + 0.75 * Math.sin(t * Math.PI) }; });
  return [newItem({ Stroke: hl ? { pen: "Highlighter", color: p.hl_color, width: mm(p.hl_mm), points: pts } : { pen: p.pen_kind, color: p.pen_color, width: mm(p.pen_mm), points: pts } })];
}

export function Panels(props: Props) {
  const { kind, at, onClose } = props;
  const side = kind === "more" || kind === "template" || kind === "customize" ? "below" : kind === "shapeStyle" || kind === "moveLayer" ? "below" : "right";
  return (
    <Popover anchor={at} side={side} onClose={onClose} width={kind === "more" ? 300 : 320}>
      <PanelBody {...props} />
    </Popover>
  );
}

function PanelBody({ kind, model, board, api, tool, onClose, setPanel, setTool, openDialog, onImport, onExportPdf, onOverview }: Props) {
  const { prefs, setPrefs, toast } = api;
  const set = (f: (p: Prefs) => void) => setPrefs(f);
  const center = () => board.visibleCenter(model.current);

  switch (kind) {
    case "tool:pen":
      return (
        <>
          <h3>Pen</h3>
          <div style={{ background: "var(--raised)", borderRadius: 12 }}><ItemsPreview items={penPreview(prefs, false)} w={290} h={64} /></div>
          <div className="section">Pen type</div>
          <Seg value={prefs.pen_kind} options={[["Fineliner", "Ballpoint"], ["Fountain", "Fountain"], ["Brush", "Brush"]] as [PenKind, string][]} onChange={(v) => set((p) => (p.pen_kind = v))} />
          <div className="section">Size</div>
          <SizeRow sizes={PEN_SIZES_MM} value={prefs.pen_mm} onPick={(v) => set((p) => (p.pen_mm = v))} />
          <input type="range" min={0.1} max={6} step={0.05} value={prefs.pen_mm} onChange={(e) => set((p) => (p.pen_mm = Number(e.target.value)))} />
          <div className="section">Colour</div>
          <Swatches colors={prefs.swatches} value={prefs.pen_color} onPick={(c) => set((p) => (p.pen_color = c))} onRemove={(i) => set((p) => (p.swatches = p.swatches.filter((_, k) => k !== i)))} />
          <div className="section">Feel</div>
          <div className="row"><span style={{ width: 110 }}>Smoothing</span><input type="range" min={0} max={1} step={0.05} value={prefs.stabilization} onChange={(e) => set((p) => (p.stabilization = Number(e.target.value)))} /></div>
          <div className="row"><span style={{ width: 110 }}>Pressure</span><input type="range" min={0.5} max={2} step={0.05} value={prefs.pressure_gain} onChange={(e) => set((p) => (p.pressure_gain = Number(e.target.value)))} /></div>
          <Toggle label="Hold to draw shapes" value={prefs.shape_hold} onChange={(v) => set((p) => (p.shape_hold = v))} />
        </>
      );
    case "tool:highlighter":
      return (
        <>
          <h3>Highlighter</h3>
          <div style={{ background: "var(--raised)", borderRadius: 12 }}><ItemsPreview items={penPreview(prefs, true)} w={290} h={64} /></div>
          <div className="section">Size</div>
          <SizeRow sizes={HL_SIZES_MM} value={prefs.hl_mm} onPick={(v) => set((p) => (p.hl_mm = v))} />
          <div className="section">Colour</div>
          <Swatches colors={prefs.hl_swatches} value={prefs.hl_color} onPick={(c) => set((p) => (p.hl_color = c))} onRemove={(i) => set((p) => (p.hl_swatches = p.hl_swatches.filter((_, k) => k !== i)))} />
          <Toggle label="Draw straight lines" value={prefs.hl_straight} onChange={(v) => set((p) => (p.hl_straight = v))} />
        </>
      );
    case "tool:tape":
      return (
        <>
          <h3>Tape</h3>
          <div className="section">Tape type</div>
          <Seg value={prefs.tape_kind} options={[["Line", "Straight"], ["Rect", "Rectangle"], ["Free", "Freehand"]]} onChange={(v) => set((p) => (p.tape_kind = v))} />
          {prefs.tape_kind !== "Rect" && (<><div className="section">Size</div><SizeRow sizes={TAPE_SIZES_MM} value={prefs.tape_mm} onPick={(v) => set((p) => (p.tape_mm = v))} /></>)}
          <div className="section">Pattern</div>
          <div className="row">
            {TAPE_COLORS.map((h) => {
              const c = fromHex(h);
              const on = c[0] === prefs.tape_color[0] && c[1] === prefs.tape_color[1] && c[2] === prefs.tape_color[2];
              return <button key={h} onClick={() => set((p) => (p.tape_color = c))} style={{ flex: 1, height: 30, borderRadius: 12, background: `repeating-linear-gradient(-55deg, ${h} 0 5px, rgba(255,255,255,.45) 5px 6.5px)`, outline: on ? "2.5px solid var(--accent)" : "none", outlineOffset: 2 }} />;
            })}
          </div>
          <div className="tip">💡 Cover your notes with tape, then test yourself. Tap a tape to reveal it.</div>
          <div className="row" style={{ marginTop: 10 }}>
            <button className="btn grow" onClick={() => model.setAllTapes(false)}>Cover all</button>
            <button className="btn grow" onClick={() => model.setAllTapes(true)}>Reveal all</button>
          </div>
        </>
      );
    case "tool:eraser":
      return (
        <>
          <h3>Eraser</h3>
          <Seg value={prefs.eraser_precise ? "p" : "w"} options={[["w", "Whole stroke"], ["p", "Precise"]]} onChange={(v) => set((p) => (p.eraser_precise = v === "p"))} />
          <div className="section">Size</div>
          <input type="range" min={4} max={60} value={prefs.eraser_size} onChange={(e) => set((p) => (p.eraser_size = Number(e.target.value)))} />
          <Toggle label="Erase highlighter only" value={prefs.eraser_hl_only} onChange={(v) => set((p) => (p.eraser_hl_only = v))} />
          <Toggle label="Erase shapes" value={prefs.eraser_shapes} onChange={(v) => set((p) => (p.eraser_shapes = v))} />
          <Toggle label="Erase tape" value={prefs.eraser_tapes} onChange={(v) => set((p) => (p.eraser_tapes = v))} />
          <Toggle label="Return to previous tool" value={prefs.eraser_return} onChange={(v) => set((p) => (p.eraser_return = v))} />
          <button className="btn danger" style={{ marginTop: 8 }} onClick={() => { model.clearPage(model.current); onClose(); toast("Page cleared (undo to restore)"); }}>Clear this page</button>
        </>
      );
    case "tool:lasso":
      return (
        <>
          <h3>Lasso</h3>
          <div className="section">Shape</div>
          <Seg value={prefs.lasso_rect ? "r" : "f"} options={[["f", "Freeform"], ["r", "Rectangle"]]} onChange={(v) => set((p) => (p.lasso_rect = v === "r"))} />
          <div className="section">Object selection</div>
          <Toggle label="Handwriting" value={prefs.lasso_ink} onChange={(v) => set((p) => (p.lasso_ink = v))} />
          <Toggle label="Highlights" value={prefs.lasso_hl} onChange={(v) => set((p) => (p.lasso_hl = v))} />
          <Toggle label="Text boxes" value={prefs.lasso_text} onChange={(v) => set((p) => (p.lasso_text = v))} />
          <Toggle label="Images" value={prefs.lasso_images} onChange={(v) => set((p) => (p.lasso_images = v))} />
          <Toggle label="Shapes" value={prefs.lasso_shapes} onChange={(v) => set((p) => (p.lasso_shapes = v))} />
          <Toggle label="Tapes" value={prefs.lasso_tapes} onChange={(v) => set((p) => (p.lasso_tapes = v))} />
          <button className="btn" style={{ marginTop: 8 }} onClick={() => { model.selectAll(); onClose(); }}>Select everything on this page</button>
        </>
      );
    case "tool:shape":
      return (
        <>
          <h3>Shapes</h3>
          <Seg value={prefs.shape_kind} options={[["Line", "Line"], ["Arrow", "Arrow"], ["Rect", "Rectangle"], ["Ellipse", "Ellipse"]] as [ShapeKind, string][]} onChange={(v) => set((p) => (p.shape_kind = v))} />
          <div className="section">Line width</div>
          <input type="range" min={0.2} max={4} step={0.05} value={prefs.shape_mm} onChange={(e) => set((p) => (p.shape_mm = Number(e.target.value)))} />
          <div className="section">Colour</div>
          <Swatches colors={prefs.swatches} value={prefs.shape_color} onPick={(c) => set((p) => (p.shape_color = c))} />
          <Toggle label="Fill" value={prefs.shape_fill} onChange={(v) => set((p) => (p.shape_fill = v))} />
          <Toggle label="Dashed" value={prefs.shape_dashed} onChange={(v) => set((p) => (p.shape_dashed = v))} />
          <button className="btn" onClick={() => setPanel("shapes")}>Insert a ready-made shape…</button>
        </>
      );
    case "tool:text":
      return (
        <>
          <h3>Text</h3>
          <div className="section">Size: {prefs.text_size}</div>
          <input type="range" min={8} max={72} value={prefs.text_size} onChange={(e) => set((p) => (p.text_size = Number(e.target.value)))} />
          <div className="section">Colour</div>
          <Swatches colors={prefs.swatches} value={prefs.text_color} onPick={(c) => set((p) => (p.text_color = c))} />
          <div className="tip">Tap the page to add a text box. Tap existing text to edit it. Apple Pencil Scribble works too.</div>
        </>
      );
    case "tool:laser":
      return (
        <>
          <h3>Laser pointer</h3>
          <p className="muted small">Point at things while presenting. The trail fades and is never saved.</p>
          <Swatches colors={["#ff3b30", "#34c759", "#0a84ff", "#ffcc00", "#af52de"].map(fromHex)} value={prefs.laser_color} onPick={(c) => set((p) => (p.laser_color = c))} />
        </>
      );
    case "tool:hand":
      return (<><h3>Hand</h3><p className="muted small">Scroll without writing. Tap links to follow them and tape to reveal it.</p></>);
    case "color": {
      const cur: Rgba = ({ highlighter: prefs.hl_color, shape: prefs.shape_color, text: prefs.text_color, laser: prefs.laser_color } as Partial<Record<Tool, Rgba>>)[tool] ?? prefs.pen_color;
      const setC = (c: Rgba) => set((p) => {
        if (tool === "highlighter") p.hl_color = c; else if (tool === "shape") p.shape_color = c;
        else if (tool === "text") p.text_color = c; else if (tool === "laser") p.laser_color = c; else p.pen_color = c;
      });
      const hl = tool === "highlighter";
      const favs = hl ? prefs.hl_swatches : prefs.swatches;
      const isFav = favs.some((f) => f[0] === cur[0] && f[1] === cur[1] && f[2] === cur[2]);
      return (
        <>
          <h3>Colour</h3>
          <Swatches colors={PALETTE.map(fromHex)} value={cur} onPick={setC} />
          <div className="row" style={{ marginTop: 10 }}>
            <span>Custom</span>
            <input type="color" value={"#" + cur.slice(0, 3).map((v) => v.toString(16).padStart(2, "0")).join("")} onChange={(e) => setC(fromHex(e.target.value))} style={{ width: 44, height: 32, border: 0, background: "none" }} />
          </div>
          <div className="section">Favourites</div>
          <Swatches colors={favs} value={cur} onPick={setC} onRemove={(i) => set((p) => { if (hl) p.hl_swatches = p.hl_swatches.filter((_, k) => k !== i); else p.swatches = p.swatches.filter((_, k) => k !== i); })} />
          {!isFav && <button className="btn" style={{ marginTop: 8 }} onClick={() => set((p) => { if (hl) p.hl_swatches = [...p.hl_swatches, cur]; else p.swatches = [...p.swatches, cur]; })}>Add to favourites</button>}
        </>
      );
    }
    case "insert":
      return (
        <div className="menu">
          <div className="menu-title">Insert</div>
          <button onClick={() => { model.insertPage(model.current); onClose(); }}><Icon name="page" size={18} />Page after this one</button>
          <button onClick={() => onImport(false)}><Icon name="image" size={18} />Image…</button>
          <button onClick={() => onImport(true)}><Icon name="file" size={18} />Import PDF or photo as pages…</button>
          <button onClick={() => { setTool("text"); onClose(); toast("Tap the page where the text should go"); }}><Icon name="text" size={18} />Text box</button>
          <button onClick={() => setPanel("shapes")}><Icon name="shapes" size={18} />Shape…</button>
          <button onClick={() => setPanel("stickers")}><Icon name="sticker" size={18} />Sticker…</button>
          <button onClick={() => { board.toggleRuler(); onClose(); }}><Icon name="ruler" size={18} />Ruler</button>
          <button disabled={!model.hasClipboard()} onClick={() => { model.paste(center()); setTool("lasso"); onClose(); }}><Icon name="paste" size={18} />Paste</button>
        </div>
      );
    case "shapes":
      return (
        <>
          <h3>Insert Shape</h3>
          <div className="grid3">
            {PRESETS.map((p) => (
              <button key={p} title={p} onClick={() => {
                if (!model.canWrite(model.current)) return toast("The active layer is locked or hidden");
                const c = center();
                model.addItems(model.current, [presetItem(p, c[0], c[1], 120, prefs.fill_color)]);
                setTool("lasso");
                onClose();
              }}>
                <ItemsPreview items={[presetItem(p, 0, 0, 48, prefs.fill_color)]} w={80} h={64} />
              </button>
            ))}
          </div>
          <div className="section">Colour</div>
          <Swatches colors={PALETTE.map(fromHex)} value={prefs.fill_color} onPick={(c) => set((p) => (p.fill_color = c))} />
        </>
      );
    case "stickers":
      return (
        <>
          <h3>Stickers</h3>
          {!prefs.stickers.length && <p className="muted small">No stickers yet. Select something with the Lasso, then More ▾ › Save as sticker.</p>}
          <div className="grid3">
            {prefs.stickers.map((st) => (
              <button key={st.id} style={{ background: "var(--raised)" }}
                onClick={() => {
                  model.placeItems(st.items, center());
                  set((p) => (p.stickers = [{ ...st, used: Date.now() }, ...p.stickers.filter((x) => x.id !== st.id)]));
                  setTool("lasso");
                  onClose();
                }}
                onContextMenu={(e) => { e.preventDefault(); set((p) => (p.stickers = p.stickers.filter((x) => x.id !== st.id))); }}>
                <ItemsPreview items={st.items} w={80} h={64} />
              </button>
            ))}
          </div>
          <div className="row" style={{ marginTop: 10 }}>
            <button className="btn" onClick={() => set((p) => (p.stickers = [...sampleStickers(), ...p.stickers].slice(0, 40)))}>Install samples</button>
            <span className="muted small">Right-click / long-press to delete</span>
          </div>
        </>
      );
    case "shapeStyle": {
      const cur = model.selectedItems().find((i) => "Shape" in i.content);
      if (!cur || !("Shape" in cur.content)) return <p className="muted">Select a shape first.</p>;
      const sh = cur.content.Shape;
      const apply = (f: (s: Shape) => Shape) => model.mapSelection((it) => ("Shape" in it.content ? { ...it, content: { Shape: f(it.content.Shape) } } : it));
      const fillMode = !sh.fill ? "o" : sh.solid ? "f" : "t";
      const border = !sh.dashed ? "s" : sh.dotted ? "dot" : "d";
      return (
        <>
          <h3>Shape Style</h3>
          <Swatches colors={PALETTE.map(fromHex)} value={sh.color} onPick={(c) => { apply((s) => ({ ...s, color: c, fill: s.fill ? c : s.fill })); set((p) => (p.fill_color = c)); }} />
          <div className="section">Fill</div>
          <Seg value={fillMode} options={[["o", "Outline"], ["t", "Tinted"], ["f", "Filled"]]} onChange={(v) => apply((s) => ({ ...s, fill: v === "o" ? null : s.color, solid: v === "f" }))} />
          <div className="section">Border</div>
          <Seg value={border} options={[["s", "Solid"], ["d", "Dashed"], ["dot", "Dotted"]]} onChange={(v) => apply((s) => ({ ...s, dashed: v !== "s", dotted: v === "dot" }))} />
          <div style={{ height: 8 }} />
          <Seg value={String(sh.width)} options={[["0.6", "Thin"], ["1.5", "Medium"], ["3", "Thick"], ["5", "Heavy"]]} onChange={(v) => apply((s) => ({ ...s, width: Number(v) }))} />
        </>
      );
    }
    case "moveLayer": {
      const sel = model.selection;
      if (!sel) return <p className="muted">Nothing selected.</p>;
      return (
        <div className="menu">
          <div className="menu-title">Move to layer</div>
          {[...model.page(sel.page).layers].reverse().map((l) => (
            <button key={l.id} disabled={l.locked} onClick={() => { model.moveSelectionToLayer(l.id); onClose(); }}>{l.name}{l.locked ? " 🔒" : ""}</button>
          ))}
        </div>
      );
    }
    case "customize":
      return (
        <>
          <h3>Customize Toolbar</h3>
          {TOOL_NAMES.map((n) => (
            <Toggle key={n} label={n} value={!prefs.hidden_tools.includes(n)} onChange={(v) => set((p) => (p.hidden_tools = v ? p.hidden_tools.filter((x) => x !== n) : [...p.hidden_tools, n]))} />
          ))}
          <button className="btn" onClick={() => set((p) => (p.hidden_tools = []))}>Reset</button>
        </>
      );
    case "template": {
      const page = model.page(model.current);
      const landscape = page.w > page.h;
      return (
        <>
          <h3>Paper</h3>
          <TemplatePicker value={page.template} paper={page.paper} onPick={(t) => model.setTemplate(t, page.paper, false)} />
          <div className="section">Background colour</div>
          <PaperSwatches papers={PAPERS} value={page.paper} bg={paperBg} onPick={(p) => model.setTemplate(page.template, p, false)} />
          <div className="row" style={{ marginTop: 10, flexWrap: "wrap" }}>
            <button className="btn" onClick={() => { model.setTemplate(page.template, page.paper, true); toast("Applied to all pages"); }}>Apply to all pages</button>
            <button className="btn" onClick={() => { set((p) => { p.new_template = page.template; p.new_paper = page.paper; }); toast("Saved as default paper"); }}>Use for new notebooks</button>
          </div>
          <div className="section">Paper size</div>
          <div className="row" style={{ flexWrap: "wrap", gap: 6 }}>
            {PAPER_SIZES.map((s) => {
              const [w, h] = sizeDims(s, landscape);
              return <button key={s} className={"btn" + (Math.abs(w - page.w) < 1 && Math.abs(h - page.h) < 1 ? " primary" : "")} onClick={() => model.setPageSize(w, h, false)}>{SIZE_NAMES[s]}</button>;
            })}
          </div>
          <div style={{ height: 8 }} />
          <Seg value={landscape ? "l" : "p"} options={[["p", "Portrait"], ["l", "Landscape"]]} onChange={(v) => { if ((v === "l") !== landscape) model.setPageSize(page.h, page.w, false); }} />
        </>
      );
    }
    case "more": {
      const i = model.current;
      const page = model.page(i);
      return (
        <div className="menu">
          <div className="row" style={{ gap: 6, padding: "0 2px 6px" }}>
            <button className="btn grow" onClick={() => setPanel("template")}>Paper</button>
            <button className="btn grow" onClick={onExportPdf}>Export PDF</button>
            <button className="btn grow" onClick={() => openDialog({ k: "find", q: "" })}>Find</button>
          </div>
          <div className="menu-title">Page {i + 1}</div>
          <button onClick={() => openDialog({ k: "bookmark", page: i, name: page.bookmark ?? `Page ${i + 1}` })}><Icon name="bookmark" size={18} />{page.bookmark ? "Rename bookmark" : "Bookmark page"}</button>
          {page.bookmark && <button onClick={() => model.setBookmark(i, null)}>Remove bookmark</button>}
          <button onClick={() => { const id = model.addOutline(i); openDialog({ k: "outline", id, title: model.doc.outline.find((o) => o.id === id)?.title ?? "" }); }}><Icon name="outline" size={18} />Add to outline</button>
          <button onClick={() => model.duplicatePage(i)}><Icon name="duplicate" size={18} />Duplicate page</button>
          <button onClick={() => openDialog({ k: "goto", text: String(i + 1) })}><Icon name="page" size={18} />Go to page…</button>
          <button onClick={onOverview}><Icon name="grid" size={18} />All pages</button>
          <button className="danger-text" onClick={() => { model.deletePage(i); onClose(); }}><Icon name="trash" size={18} />Delete page</button>
          <hr />
          <div className="menu-title">Notebook</div>
          <button onClick={() => openDialog({ k: "rename", name: model.title })}><Icon name="text" size={18} />Rename…</button>
          <hr />
          <div className="menu-title">View</div>
          <div style={{ padding: "0 12px" }}>
            <Toggle label="Night mode" value={prefs.night_mode} onChange={(v) => set((p) => (p.night_mode = v))} />
            <Toggle label="Study timer" value={prefs.show_timer} onChange={(v) => set((p) => (p.show_timer = v))} />
            <Toggle label="Keep screen awake" value={prefs.keep_awake} onChange={(v) => set((p) => (p.keep_awake = v))} />
            <Toggle label="Search handwriting" value={prefs.ocr_index} onChange={(v) => set((p) => (p.ocr_index = v))} />
            <Toggle label="Dark interface" value={prefs.dark_ui} onChange={(v) => set((p) => (p.dark_ui = v))} />
          </div>
          <hr />
          <div className="menu-title">Settings</div>
          <div style={{ padding: "0 12px" }}>
            <Toggle label="Only Pencil writes" value={prefs.pencil_only} onChange={(v) => set((p) => (p.pencil_only = v))} />
            <Toggle label="Two-finger tap to undo" value={prefs.two_finger_undo} onChange={(v) => set((p) => (p.two_finger_undo = v))} />
          </div>
          <button onClick={() => setPanel("customize")}><Icon name="gear" size={18} />Customize toolbar…</button>
        </div>
      );
    }
  }
  return null;
}
