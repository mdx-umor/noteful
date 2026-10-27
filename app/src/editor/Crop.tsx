import { useEffect, useRef, useState } from "react";
import type { Item } from "../core/model";
import { picture, pictureReady } from "../core/pictures";
import { Modal } from "../ui/ui";
import type { DocModel } from "./doc";

type Box = [number, number, number, number];

/** Crop a picture: drag the corners or move the box. Keeps the visible part in place on the page. */
export function CropDialog({ model, item, onClose }: { model: DocModel; item: Item; onClose: () => void }) {
  const im = "Image" in item.content ? item.content.Image : null;
  const [box, setBox] = useState<Box>(im?.crop ?? [0, 0, 1, 1]);
  const [ready, setReady] = useState(false);
  const canvas = useRef<HTMLCanvasElement>(null);
  const drag = useRef<{ mode: string; start: [number, number]; box: Box } | null>(null);
  const W = 520, H = 380;

  useEffect(() => { if (im) pictureReady(im.asset).then(() => setReady(true)); }, [im]);
  const bmp = im ? picture(im.asset) : undefined;
  const scale = bmp ? Math.min(W / bmp.width, H / bmp.height) : 1;
  const dw = bmp ? bmp.width * scale : W, dh = bmp ? bmp.height * scale : H;

  useEffect(() => {
    const c = canvas.current;
    if (!c || !bmp) return;
    const dpr = devicePixelRatio || 1;
    c.width = dw * dpr;
    c.height = dh * dpr;
    const ctx = c.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.drawImage(bmp, 0, 0, dw, dh);
    const [x, y, w, h] = [box[0] * dw, box[1] * dh, box[2] * dw, box[3] * dh];
    ctx.fillStyle = "rgba(0,0,0,0.55)";
    ctx.beginPath();
    ctx.rect(0, 0, dw, dh);
    ctx.rect(x + w, y, -w, h); // hole (opposite winding)
    ctx.fill("evenodd");
    ctx.strokeStyle = "#fff";
    ctx.lineWidth = 2;
    ctx.strokeRect(x, y, w, h);
    ctx.fillStyle = "#fff";
    for (const [hx, hy] of [[x, y], [x + w, y], [x, y + h], [x + w, y + h]]) {
      ctx.beginPath();
      ctx.arc(hx, hy, 8, 0, Math.PI * 2);
      ctx.fill();
    }
  }, [bmp, box, dw, dh, ready]);

  if (!im) return null;

  const local = (e: React.PointerEvent): [number, number] => {
    const r = canvas.current!.getBoundingClientRect();
    return [(e.clientX - r.left) / dw, (e.clientY - r.top) / dh];
  };
  const down = (e: React.PointerEvent) => {
    const [px, py] = local(e);
    const [x, y, w, h] = box;
    const near = (ax: number, ay: number) => Math.hypot((px - ax) * dw, (py - ay) * dh) < 22;
    const mode = near(x, y) ? "tl" : near(x + w, y) ? "tr" : near(x, y + h) ? "bl" : near(x + w, y + h) ? "br"
      : px > x && px < x + w && py > y && py < y + h ? "move" : "";
    if (!mode) return;
    drag.current = { mode, start: [px, py], box };
    (e.target as Element).setPointerCapture(e.pointerId);
  };
  const move = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    const [px, py] = local(e);
    const dx = px - d.start[0], dy = py - d.start[1];
    let [x, y, w, h] = d.box;
    const min = 0.05;
    if (d.mode === "move") {
      x = Math.max(0, Math.min(1 - w, x + dx));
      y = Math.max(0, Math.min(1 - h, y + dy));
    } else {
      let x2 = x + w, y2 = y + h;
      if (d.mode.includes("l")) x = Math.max(0, Math.min(x2 - min, x + dx));
      if (d.mode.includes("r")) x2 = Math.min(1, Math.max(x + min, x2 + dx));
      if (d.mode.includes("t")) y = Math.max(0, Math.min(y2 - min, y + dy));
      if (d.mode.includes("b")) y2 = Math.min(1, Math.max(y + min, y2 + dy));
      w = x2 - x;
      h = y2 - y;
    }
    setBox([x, y, w, h]);
  };

  const save = (b: Box) => {
    const old = im.crop ?? [0, 0, 1, 1];
    // Size of the whole picture on the page at the current scale.
    const fw = im.w / old[2], fh = im.h / old[3];
    const ox = im.x - old[0] * fw, oy = im.y - old[1] * fh;
    const full = b[0] === 0 && b[1] === 0 && b[2] === 1 && b[3] === 1;
    model.mapSelection((it) => (it.id === item.id && "Image" in it.content
      ? { ...it, content: { Image: { ...it.content.Image, crop: full ? null : b, x: ox + b[0] * fw, y: oy + b[1] * fh, w: b[2] * fw, h: b[3] * fh } } }
      : it));
    onClose();
  };

  return (
    <Modal onClose={onClose} wide>
      <h2>Crop image</h2>
      <div style={{ display: "grid", placeItems: "center", background: "var(--desk)", borderRadius: 12, padding: 12, touchAction: "none" }}>
        {bmp ? (
          <canvas ref={canvas} style={{ width: dw, height: dh, touchAction: "none" }} onPointerDown={down} onPointerMove={move} onPointerUp={() => (drag.current = null)} />
        ) : <div className="muted" style={{ padding: 40 }}>Loading picture…</div>}
      </div>
      <div className="actions">
        <button className="btn" onClick={() => setBox([0, 0, 1, 1])}>Reset</button>
        <span className="grow" />
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn primary" onClick={() => save(box)}>Crop</button>
      </div>
    </Modal>
  );
}
