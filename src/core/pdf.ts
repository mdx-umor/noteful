// Vector PDF export: ink, shapes and ruling stay sharp. Pictures are embedded as
// JPEG, highlighter uses real multiply blending. Text uses Helvetica.

import type { Doc, Id, Page, Rgba } from "./model";
import { assetIds, hasPictures, isDark, kindOf, paperBg, paperLine, paperMargin, mix } from "./model";
import { highlightTint, inkColor, samples, shapePolylines } from "./ink";
import { shapeOutline, tapeOutline, type Pt } from "./geom";
import { templatePrims, weeklyLabels } from "./render";
import { LINE_HEIGHT, wrap } from "./text";
import { loadAsset } from "./storage";
import { jpegInfo } from "./pictures";

const f = (n: number) => n.toFixed(2);
const rgb = (c: Rgba) => `${(c[0] / 255).toFixed(3)} ${(c[1] / 255).toFixed(3)} ${(c[2] / 255).toFixed(3)}`;

function path(pts: Pt[]): string {
  if (!pts.length) return "";
  let s = `${f(pts[0][0])} ${f(pts[0][1])} m `;
  if (pts.length === 1) s += `${f(pts[0][0] + 0.01)} ${f(pts[0][1])} l `;
  for (const p of pts.slice(1)) s += `${f(p[0])} ${f(p[1])} l `;
  return s;
}
const strokeLine = (pts: Pt[], w: number, c: Rgba) => `${rgb(c)} RG ${f(Math.max(0.05, w))} w ${path(pts)}S\n`;

function circle(c: Pt, r: number): string {
  const k = 0.5523 * r, [x, y] = c;
  return `${f(x + r)} ${f(y)} m ${f(x + r)} ${f(y + k)} ${f(x + k)} ${f(y + r)} ${f(x)} ${f(y + r)} c ${f(x - k)} ${f(y + r)} ${f(x - r)} ${f(y + k)} ${f(x - r)} ${f(y)} c ${f(x - r)} ${f(y - k)} ${f(x - k)} ${f(y - r)} ${f(x)} ${f(y - r)} c ${f(x + k)} ${f(y - r)} ${f(x + r)} ${f(y - k)} ${f(x + r)} ${f(y)} c f\n`;
}

/** WinAnsi text; unsupported characters become '?'. */
function pdfString(s: string): string {
  let out = "(";
  for (const ch of s) {
    const c = ch.codePointAt(0)!;
    if (ch === "(" || ch === ")" || ch === "\\") out += "\\" + ch;
    else if (c >= 32 && c <= 126) out += ch;
    else if (c >= 0xa0 && c <= 0xff) out += "\\" + c.toString(8).padStart(3, "0");
    else out += "?";
  }
  return out + ")";
}

// Helvetica widths are close enough to the screen font for line placement.
function pageContent(page: Page, images: Map<Id, number>): string {
  let s = `q 1 0 0 -1 0 ${f(page.h)} cm 1 J 1 j\n`;
  s += `${rgb(paperBg(page.paper))} rg 0 0 ${f(page.w)} ${f(page.h)} re f\n`;
  for (const p of templatePrims(page.template, page.w, page.h)) {
    if (p.t === "line") s += (p.dashed ? "[4 4] 0 d " : "") + strokeLine([p.a, p.b], p.w, p.margin ? paperMargin(page.paper) : paperLine(page.paper)) + (p.dashed ? "[] 0 d\n" : "");
    else if (p.t === "dot") s += `${rgb(paperLine(page.paper))} rg ` + circle(p.c, p.r);
    else s += strokeLine([[p.x, p.y], [p.x + p.w, p.y], [p.x + p.w, p.y + p.h], [p.x, p.y + p.h], [p.x, p.y]], p.lw, paperLine(page.paper));
  }
  if (page.template === "Weekly")
    for (const l of weeklyLabels(page.w, page.h))
      s += `${rgb(paperLine(page.paper))} rg BT /F1 11 Tf 1 0 0 -1 ${f(l.x)} ${f(l.y + 4)} Tm ${pdfString(l.text)} Tj ET\n`;
  const img = (id: Id, x: number, y: number, w: number, h: number) => {
    const k = images.get(id);
    if (k !== undefined) s += `q ${f(w)} 0 0 ${f(-h)} ${f(x)} ${f(y + h)} cm /Im${k} Do Q\n`;
  };
  if (page.background) img(page.background, 0, 0, page.w, page.h);
  const multiply = hasPictures(page) || !isDark(page.paper);
  const order = { image: 0, highlight: 1, ink: 2, shape: 2, text: 2, tape: 3 } as const;
  for (const layer of page.layers.filter((l) => l.visible)) {
    const items = page.items.filter((it) => (it.layer ?? 0) === layer.id);
    for (let pass = 0; pass < 4; pass++) for (const it of items) {
      if (order[kindOf(it)] !== pass) continue;
      const c = it.content;
      if ("Image" in c) {
        const im = c.Image;
        if (im.crop) {
          // Clip to the visible box, then place the whole picture so the crop lines up.
          const [cx, cy, cw, ch] = im.crop;
          const fw = im.w / cw, fh = im.h / ch;
          s += `q ${f(im.x)} ${f(im.y)} ${f(im.w)} ${f(im.h)} re W n\n`;
          img(im.asset, im.x - cx * fw, im.y - cy * fh, fw, fh);
          s += "Q\n";
        } else img(im.asset, im.x, im.y, im.w, im.h);
      }
      else if ("Tape" in c) {
        const t = c.Tape;
        const [pts, closed] = tapeOutline(t);
        if (t.revealed) s += strokeLine(closed ? [...pts, pts[0]] : pts, 0.6, t.color);
        else if (closed) s += `${rgb(t.color)} rg ${path(pts)}h f\n`;
        else s += `0 J ${rgb(t.color)} RG ${f(t.width)} w ${path(pts)}S 1 J\n`;
      } else if ("Stroke" in c) {
        const st = c.Stroke;
        const ss = samples(st);
        if (st.pen === "Highlighter" && multiply) {
          s += "q /GSM gs 0 J\n" + strokeLine(ss.map((q) => [q.x, q.y]), st.width, highlightTint(st.color)) + "Q\n";
          continue;
        }
        const col = inkColor(st, page.paper);
        if (ss.length === 1) { s += `${rgb(col)} rg ` + circle([ss[0].x, ss[0].y], ss[0].w / 2); continue; }
        // Runs of near-constant width.
        let start = 0;
        while (start < ss.length) {
          const w0 = ss[start].w;
          let end = start + 1, sum = w0;
          while (end < ss.length && Math.abs(ss[end].w - w0) <= w0 * 0.06 + 0.05) { sum += ss[end].w; end++; }
          s += strokeLine(ss.slice(start, Math.min(end + 1, ss.length)).map((q) => [q.x, q.y]), sum / (end - start), col);
          start = end;
        }
      } else if ("Shape" in c) {
        const sh = c.Shape;
        const [pts, closed] = shapeOutline(sh);
        if (sh.fill && closed && pts.length > 2)
          s += `${rgb(sh.solid ? sh.fill : mix(sh.fill, paperBg(page.paper), 0.25))} rg ${path(pts)}h f\n`;
        for (const l of shapePolylines(sh)) s += strokeLine(l, sh.width, sh.color);
      } else {
        const t = c.Text;
        const lh = t.size * LINE_HEIGHT;
        s += `${rgb(t.color)} rg `;
        wrap(t.text, t.size, t.w).forEach((line, i) => {
          const base = t.y + i * lh + lh / 2 + t.size * 0.35;
          s += `BT /F1 ${f(t.size)} Tf 1 0 0 -1 ${f(t.x)} ${f(base)} Tm ${pdfString(line)} Tj ET\n`;
        });
      }
    }
  }
  return s + "Q\n";
}

const enc = new TextEncoder();

export function exportPdf(doc: Doc, title: string): Uint8Array {
  const n = doc.pages.length;
  const objects: Uint8Array[] = [];
  const pageObj = (i: number) => 5 + i * 2;
  const imgObjs: Uint8Array[] = [];
  const images = new Map<Id, number>();
  for (const id of assetIds(doc)) {
    const bytes = loadAsset(id);
    const info = bytes && jpegInfo(bytes);
    if (!bytes || !info || (info.comps !== 1 && info.comps !== 3)) continue;
    const head = enc.encode(`<< /Type /XObject /Subtype /Image /Width ${info.w} /Height ${info.h} /ColorSpace /${info.comps === 1 ? "DeviceGray" : "DeviceRGB"} /BitsPerComponent 8 /Filter /DCTDecode /Length ${bytes.length} >>\nstream\n`);
    images.set(id, imgObjs.length);
    imgObjs.push(concat([head, bytes, enc.encode("\nendstream")]));
  }
  const imgObj = (k: number) => 5 + n * 2 + k;
  const gsObj = 5 + n * 2 + imgObjs.length;
  const xobjects = [...images.values()].map((k) => `/Im${k} ${imgObj(k)} 0 R`).join(" ");
  objects.push(enc.encode("<< /Type /Catalog /Pages 2 0 R >>"));
  objects.push(enc.encode(`<< /Type /Pages /Kids [${doc.pages.map((_, i) => `${pageObj(i)} 0 R`).join(" ")}] /Count ${n} >>`));
  objects.push(enc.encode("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"));
  objects.push(enc.encode(`<< /Title ${pdfString(title)} /Producer (Notebook) >>`));
  doc.pages.forEach((page, i) => {
    objects.push(enc.encode(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${f(page.w)} ${f(page.h)}] /Resources << /Font << /F1 3 0 R >> /XObject << ${xobjects} >> /ExtGState << /GSM ${gsObj} 0 R >> >> /Contents ${pageObj(i) + 1} 0 R >>`));
    const content = enc.encode(pageContent(page, images));
    objects.push(concat([enc.encode(`<< /Length ${content.length} >>\nstream\n`), content, enc.encode("\nendstream")]));
  });
  objects.push(...imgObjs);
  objects.push(enc.encode("<< /Type /ExtGState /BM /Multiply /CA 1 /ca 1 >>"));

  const parts: Uint8Array[] = [enc.encode("%PDF-1.4\n%âã\n")];
  let len = parts[0].length;
  const offsets: number[] = [];
  objects.forEach((o, i) => {
    offsets.push(len);
    const p = concat([enc.encode(`${i + 1} 0 obj\n`), o, enc.encode("\nendobj\n")]);
    parts.push(p);
    len += p.length;
  });
  let xref = `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  for (const o of offsets) xref += `${String(o).padStart(10, "0")} 00000 n \n`;
  xref += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R /Info 4 0 R >>\nstartxref\n${len}\n%%EOF\n`;
  parts.push(enc.encode(xref));
  return concat(parts);
}

function concat(parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((a, p) => a + p.length, 0));
  let o = 0;
  for (const p of parts) { out.set(p, o); o += p.length; }
  return out;
}

export function download(name: string, data: Uint8Array | Blob, mime: string) {
  const blob = data instanceof Blob ? data : new Blob([data], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name.replace(/[/\\:*?"<>|]/g, "-");
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 2000);
}
