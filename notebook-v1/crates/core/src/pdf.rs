//! Vector PDF export: ink, shapes and ruling stay sharp at any zoom.
//! Typed text uses the built-in Helvetica font (Latin characters).

use crate::ink;
use crate::model::{Content, Document, Id, Page, Rgba};
use std::collections::HashMap;
use crate::templates::{Prim, template_prims};
use crate::text;
use std::fmt::Write as _;

fn rgb(c: Rgba) -> String {
    format!(
        "{:.3} {:.3} {:.3}",
        c.0 as f32 / 255.0,
        c.1 as f32 / 255.0,
        c.2 as f32 / 255.0
    )
}

fn path(out: &mut String, pts: &[[f32; 2]]) {
    if let Some(f) = pts.first() {
        let _ = write!(out, "{:.2} {:.2} m ", f[0], f[1]);
        if pts.len() == 1 {
            let _ = write!(out, "{:.2} {:.2} l ", f[0] + 0.01, f[1]);
        }
        for p in &pts[1..] {
            let _ = write!(out, "{:.2} {:.2} l ", p[0], p[1]);
        }
    }
}

fn stroke_line(out: &mut String, pts: &[[f32; 2]], w: f32, c: Rgba) {
    let _ = write!(out, "{} RG {:.2} w ", rgb(c), w.max(0.05));
    path(out, pts);
    out.push_str("S\n");
}

fn circle(out: &mut String, c: [f32; 2], r: f32) {
    let k = 0.5523 * r;
    let (x, y) = (c[0], c[1]);
    let _ = writeln!(
        out,
        "{:.2} {:.2} m {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c f",
        x + r,
        y,
        x + r,
        y + k,
        x + k,
        y + r,
        x,
        y + r,
        x - k,
        y + r,
        x - r,
        y + k,
        x - r,
        y,
        x - r,
        y - k,
        x - k,
        y - r,
        x,
        y - r,
        x + k,
        y - r,
        x + r,
        y - k,
        x + r,
        y
    );
}

/// Maps text to the PDF's WinAnsi encoding; unsupported characters become '?'.
fn pdf_string(s: &str) -> String {
    let mut out = String::from("(");
    for ch in s.chars() {
        match ch {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            c if (' '..='~').contains(&c) => out.push(c),
            c if (c as u32) >= 0xA0 && (c as u32) <= 0xFF => {
                let _ = write!(out, "\\{:03o}", c as u32);
            }
            _ => out.push('?'),
        }
    }
    out.push(')');
    out
}

fn page_content(page: &Page, images: &HashMap<Id, usize>) -> String {
    let mut s = String::new();
    // Flip to top-left origin so page coordinates can be used as-is.
    let _ = writeln!(s, "q 1 0 0 -1 0 {:.2} cm 1 J 1 j", page.h);
    let _ = writeln!(
        s,
        "{} rg 0 0 {:.2} {:.2} re f",
        rgb(page.paper.background()),
        page.w,
        page.h
    );
    for p in template_prims(page.template, page.w, page.h, 1.0) {
        match p {
            Prim::Line { a, b, w, margin } => stroke_line(
                &mut s,
                &[a, b],
                w,
                if margin {
                    page.paper.margin()
                } else {
                    page.paper.line()
                },
            ),
            Prim::Dot { c, r } => {
                let _ = write!(s, "{} rg ", rgb(page.paper.line()));
                circle(&mut s, c, r);
            }
            Prim::Rect { r, w } => stroke_line(
                &mut s,
                &[
                    [r.x, r.y],
                    [r.right(), r.y],
                    [r.right(), r.bottom()],
                    [r.x, r.bottom()],
                    [r.x, r.y],
                ],
                w,
                page.paper.line(),
            ),
        }
    }
    let place_image = |s: &mut String, id: Id, x: f32, y: f32, w: f32, h: f32| {
        if let Some(k) = images.get(&id) {
            let _ = writeln!(
                s,
                "q {:.2} 0 0 {:.2} {:.2} {:.2} cm /Im{k} Do Q",
                w,
                -h,
                x,
                y + h
            );
        }
    };
    if let Some(bg) = page.background {
        place_image(&mut s, bg, 0.0, 0.0, page.w, page.h);
    }
    let first = page.layers.first().map_or(0, |l| l.id);
    let known: Vec<Id> = page.layers.iter().map(|l| l.id).collect();
    let layers: Vec<Id> = if page.layers.is_empty() {
        vec![first]
    } else {
        page.layers.iter().filter(|l| l.visible).map(|l| l.id).collect()
    };
    let mut ordered = Vec::new();
    for lid in layers {
        for pass in 0..4 {
            ordered.extend(page.items.iter().filter(|it| {
                (it.layer == lid || (lid == first && !known.contains(&it.layer)))
                    && crate::render::layer_order(it.kind()) == pass
            }));
        }
    }
    for it in ordered {
        match &it.content {
            Content::Image(im) => place_image(&mut s, im.asset, im.x, im.y, im.w, im.h),
            Content::Tape(t) => {
                let (pts, closed) = crate::geom::tape_outline(t);
                if t.revealed {
                    let mut o = pts.clone();
                    if closed && let Some(f) = pts.first() {
                        o.push(*f);
                    }
                    stroke_line(&mut s, &o, 0.6, t.color);
                } else if closed {
                    let _ = write!(s, "{} rg ", rgb(t.color));
                    path(&mut s, &pts);
                    s.push_str("h f\n");
                } else {
                    let _ = write!(s, "0 J {} RG {:.2} w ", rgb(t.color), t.width);
                    path(&mut s, &pts);
                    s.push_str("S 1 J\n");
                }
            }
            Content::Stroke(st)
                if st.pen == crate::model::PenKind::Highlighter && page.has_pictures() =>
            {
                // Real multiply blending so the picture shows through.
                let pts: Vec<[f32; 2]> = ink::samples(st).iter().map(|q| [q.x, q.y]).collect();
                s.push_str("q /GSM gs 0 J\n");
                stroke_line(&mut s, &pts, st.width, ink::highlight_tint(st.color));
                s.push_str("Q\n");
            }
            Content::Stroke(st) => {
                let c = ink::ink_color(st, page.paper);
                let samples = ink::samples(st);
                if samples.len() == 1 {
                    let _ = write!(s, "{} rg ", rgb(c));
                    circle(&mut s, [samples[0].x, samples[0].y], samples[0].w / 2.0);
                    continue;
                }
                // Runs of near-constant width, like the raster renderer.
                let mut start = 0;
                while start < samples.len() {
                    let w0 = samples[start].w;
                    let mut end = start + 1;
                    let mut sum = w0;
                    while end < samples.len() && (samples[end].w - w0).abs() <= w0 * 0.06 + 0.05 {
                        sum += samples[end].w;
                        end += 1;
                    }
                    let stop = (end + 1).min(samples.len());
                    let pts: Vec<[f32; 2]> =
                        samples[start..stop].iter().map(|q| [q.x, q.y]).collect();
                    stroke_line(&mut s, &pts, sum / (end - start) as f32, c);
                    start = end;
                }
            }
            Content::Shape(sh) => {
                if let Some(fill) = sh.fill {
                    let (pts, closed) = crate::geom::shape_outline(sh);
                    if closed && pts.len() > 2 {
                        let c = if sh.solid {
                            fill
                        } else {
                            fill.mix_over(page.paper.background(), 0.25)
                        };
                        let _ = write!(s, "{} rg ", rgb(c));
                        path(&mut s, &pts);
                        s.push_str("h f\n");
                    }
                }
                for line in ink::shape_polylines(sh) {
                    stroke_line(&mut s, &line, sh.width, sh.color);
                }
            }
            Content::Text(t) => {
                let lh = t.size * text::LINE_HEIGHT;
                let _ = write!(s, "{} rg ", rgb(t.color));
                for (i, line) in text::wrap(&t.text, t.size, t.w).iter().enumerate() {
                    let base =
                        t.y + i as f32 * lh + (lh - t.size) / 2.0 + text::ascent(t.size) * 0.92;
                    let _ = writeln!(
                        s,
                        "BT /F1 {:.2} Tf 1 0 0 -1 {:.2} {:.2} Tm {} Tj ET",
                        t.size,
                        t.x,
                        base,
                        pdf_string(line)
                    );
                }
            }
        }
    }
    s.push_str("Q\n");
    s
}

/// Builds a complete PDF file for a notebook (pictures left out).
pub fn export(doc: &Document, title: &str) -> Vec<u8> {
    export_with(doc, title, &|_| None)
}

/// Builds a complete PDF file; `asset` returns the JPEG bytes of a picture.
pub fn export_with(doc: &Document, title: &str, asset: &dyn Fn(Id) -> Option<Vec<u8>>) -> Vec<u8> {
    let mut objects: Vec<Vec<u8>> = Vec::new();
    let n = doc.pages.len();
    // Pictures become image objects after the pages.
    let mut image_objs: Vec<Vec<u8>> = Vec::new();
    let mut images: HashMap<Id, usize> = HashMap::new();
    for id in doc.asset_ids() {
        let Some(bytes) = asset(id) else { continue };
        let (Some((w, h)), Some(comps)) = (
            crate::assets::jpeg_size(&bytes),
            crate::assets::jpeg_components(&bytes),
        ) else {
            continue;
        };
        let cs = match comps {
            1 => "/DeviceGray",
            3 => "/DeviceRGB",
            _ => continue,
        };
        let mut obj = format!(
            "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace {cs} /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            bytes.len()
        )
        .into_bytes();
        obj.extend_from_slice(&bytes);
        obj.extend_from_slice(b"\nendstream");
        images.insert(id, image_objs.len());
        image_objs.push(obj);
    }
    let image_obj = |k: usize| 5 + n * 2 + k;
    let gs_obj = 5 + n * 2 + image_objs.len();
    let xobjects: String = {
        let mut v: Vec<(usize, Id)> = images.iter().map(|(id, k)| (*k, *id)).collect();
        v.sort();
        v.iter()
            .map(|(k, _)| format!("/Im{k} {} 0 R", image_obj(*k)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    // 1 catalog, 2 pages tree, 3 font, 4 info, then a page + content pair per page.
    let page_obj = |i: usize| 5 + i * 2;
    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", page_obj(i))).collect();
    objects
        .push(format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), n).into_bytes());
    objects.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    objects
        .push(format!("<< /Title {} /Producer (Notebook v1) >>", pdf_string(title)).into_bytes());
    for (i, page) in doc.pages.iter().enumerate() {
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Resources << /Font << /F1 3 0 R >> /XObject << {} >> /ExtGState << /GSM {} 0 R >> >> /Contents {} 0 R >>",
                page.w,
                page.h,
                xobjects,
                gs_obj,
                page_obj(i) + 1
            )
            .into_bytes(),
        );
        let content = page_content(page, &images);
        let mut obj = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        obj.extend_from_slice(content.as_bytes());
        obj.extend_from_slice(b"\nendstream");
        objects.push(obj);
    }
    objects.extend(image_objs);
    objects.push(b"<< /Type /ExtGState /BM /Multiply /CA 1 /ca 1 >>".to_vec());

    let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(obj);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 4 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::sync::Arc;

    #[test]
    fn writes_valid_structure() {
        let mut doc = Document::new(1, Template::Dotted, Paper::White);
        doc.pages[0]
            .items
            .push(Arc::new(Item::new(Content::Text(TextBox {
                x: 50.0,
                y: 50.0,
                w: 300.0,
                text: "Café (notes)".into(),
                size: 14.0,
                color: Rgba::BLACK,
            }))));
        doc.pages.push(Page::new(Template::Grid, Paper::Night));
        let pdf = export(&doc, "Test");
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.contains("/Count 2"));
        assert!(s.contains("\\(notes\\)"));
        assert!(s.trim_end().ends_with("%%EOF"));
        // xref offsets must point at "N 0 obj".
        let xref_pos: usize = s
            .rsplit("startxref\n")
            .next()
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(pdf[xref_pos..].starts_with(b"xref"));
        // Every xref entry must point at its object header.
        let table = &s[s.rfind("xref\n").unwrap()..];
        for (i, line) in table
            .lines()
            .skip(3)
            .take_while(|l| l.ends_with(" n "))
            .enumerate()
        {
            let off: usize = line[..10].parse().unwrap();
            assert!(pdf[off..].starts_with(format!("{} 0 obj", i + 1).as_bytes()));
        }
    }
}
