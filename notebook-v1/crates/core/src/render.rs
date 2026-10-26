//! CPU rasteriser (tiny-skia) for thumbnails, page previews and PNG export.

use crate::assets::{AssetSource, NoAssets, RgbaImage};
use crate::ink::{self, Sample};
use crate::model::{
    Content, ImageBox, Item, ItemKind, Page, Paper, Rgba, Shape, ShapeKind, Tape, Template,
    TextBox,
};
use crate::templates::{Prim, template_prims};
use crate::text;
use ab_glyph::{Font, ScaleFont, point};
use tiny_skia::{
    FillRule, FilterQuality, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, Transform,
};

fn paint(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, c.3);
    p.anti_alias = true;
    p
}

fn stroke_style(w: f32) -> tiny_skia::Stroke {
    tiny_skia::Stroke {
        width: w.max(0.1),
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    }
}

fn polyline(pix: &mut Pixmap, pts: &[[f32; 2]], w: f32, c: Rgba, tf: Transform) {
    if pts.is_empty() {
        return;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0][0], pts[0][1]);
    if pts.len() == 1 {
        pb.line_to(pts[0][0] + 0.01, pts[0][1]);
    }
    for p in &pts[1..] {
        pb.line_to(p[0], p[1]);
    }
    if let Some(path) = pb.finish() {
        pix.stroke_path(&path, &paint(c), &stroke_style(w), tf, None);
    }
}

/// Draws variable-width ink by stroking runs of samples whose width barely changes.
fn ink(pix: &mut Pixmap, samples: &[Sample], c: Rgba, tf: Transform) {
    if samples.is_empty() {
        return;
    }
    let mut start = 0;
    while start < samples.len() {
        let w0 = samples[start].w;
        let mut end = start + 1;
        let mut sum = w0;
        while end < samples.len() && (samples[end].w - w0).abs() <= w0 * 0.06 + 0.05 {
            sum += samples[end].w;
            end += 1;
        }
        // Overlap by one sample so runs join seamlessly.
        let stop = (end + 1).min(samples.len());
        let pts: Vec<[f32; 2]> = samples[start..stop].iter().map(|s| [s.x, s.y]).collect();
        polyline(pix, &pts, sum / (end - start) as f32, c, tf);
        start = end;
    }
}

pub fn paint_paper(
    pix: &mut Pixmap,
    template: Template,
    paper: Paper,
    w: f32,
    h: f32,
    tf: Transform,
) {
    paint_paper_weighted(pix, template, paper, w, h, tf, 1.0);
}

/// Like [`paint_paper`] with line thickness scaled by `weight` (bolder for small previews).
pub fn paint_paper_weighted(
    pix: &mut Pixmap,
    template: Template,
    paper: Paper,
    w: f32,
    h: f32,
    tf: Transform,
    weight: f32,
) {
    let bg = paper.background();
    if let Some(r) = tiny_skia::Rect::from_xywh(0.0, 0.0, w, h) {
        pix.fill_rect(r, &paint(bg), tf, None);
    }
    for p in template_prims(template, w, h, weight) {
        match p {
            Prim::Line { a, b, w, margin } => polyline(
                pix,
                &[a, b],
                w,
                if margin { paper.margin() } else { paper.line() },
                tf,
            ),
            Prim::Dot { c, r } => {
                if let Some(path) = PathBuilder::from_circle(c[0], c[1], r) {
                    pix.fill_path(&path, &paint(paper.line()), FillRule::Winding, tf, None);
                }
            }
            Prim::Rect { r, w } => polyline(
                pix,
                &[
                    [r.x, r.y],
                    [r.right(), r.y],
                    [r.right(), r.bottom()],
                    [r.x, r.bottom()],
                    [r.x, r.y],
                ],
                w,
                paper.line(),
                tf,
            ),
        }
    }
}

fn fill_poly(pix: &mut Pixmap, pts: &[[f32; 2]], c: Rgba, tf: Transform) {
    if pts.len() < 3 {
        return;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0][0], pts[0][1]);
    for p in &pts[1..] {
        pb.line_to(p[0], p[1]);
    }
    pb.close();
    if let Some(path) = pb.finish() {
        pix.fill_path(&path, &paint(c), FillRule::Winding, tf, None);
    }
}

/// Shape fill colour: a light tint, or the colour itself for solid shapes.
pub fn fill_color(sh: &Shape, fill: Rgba) -> Rgba {
    if sh.solid {
        Rgba(fill.0, fill.1, fill.2, 255)
    } else {
        Rgba(fill.0, fill.1, fill.2, 64)
    }
}

fn tape(pix: &mut Pixmap, t: &Tape, tf: Transform) {
    let (pts, closed) = crate::geom::tape_outline(t);
    if t.revealed {
        let mut outline = pts.clone();
        if closed && let Some(f) = pts.first() {
            outline.push(*f);
        }
        polyline(pix, &outline, 0.8, Rgba(t.color.0, t.color.1, t.color.2, 200), tf);
        return;
    }
    if closed {
        fill_poly(pix, &pts, t.color, tf);
    } else {
        let mut p = paint(t.color);
        p.anti_alias = true;
        let mut pb = PathBuilder::new();
        if let Some(f) = pts.first() {
            pb.move_to(f[0], f[1]);
            if pts.len() == 1 {
                pb.line_to(f[0] + 0.01, f[1]);
            }
            for q in &pts[1..] {
                pb.line_to(q[0], q[1]);
            }
        }
        if let Some(path) = pb.finish() {
            let st = tiny_skia::Stroke {
                width: t.width,
                line_cap: LineCap::Butt,
                line_join: LineJoin::Round,
                ..Default::default()
            };
            pix.stroke_path(&path, &p, &st, tf, None);
        }
    }
}

fn picture(pix: &mut Pixmap, img: Option<&RgbaImage>, r: [f32; 4], tf: Transform) {
    let [x, y, w, h] = r;
    let Some(img) = img else {
        if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w.max(1.0), h.max(1.0)) {
            pix.fill_rect(rect, &paint(Rgba(200, 204, 212, 255)), tf, None);
        }
        return;
    };
    let Some(src) = tiny_skia::IntSize::from_wh(img.w, img.h)
        .and_then(|sz| Pixmap::from_vec(img.px.clone(), sz))
    else {
        return;
    };
    let t = tf
        .pre_translate(x, y)
        .pre_scale(w / img.w as f32, h / img.h as f32);
    pix.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..Default::default()
        },
        t,
        None,
    );
}

fn shape(pix: &mut Pixmap, sh: &Shape, tf: Transform) {
    if let Some(fill) = sh.fill {
        let (pts, closed) = crate::geom::shape_outline(sh);
        if closed && pts.len() > 2 {
            let mut pb = PathBuilder::new();
            pb.move_to(pts[0][0], pts[0][1]);
            for p in &pts[1..] {
                pb.line_to(p[0], p[1]);
            }
            pb.close();
            if let Some(path) = pb.finish() {
                pix.fill_path(&path, &paint(fill_color(sh, fill)), FillRule::Winding, tf, None);
            }
        }
    }
    let _ = ShapeKind::Line;
    for line in ink::shape_polylines(sh) {
        polyline(pix, &line, sh.width, sh.color, tf);
    }
}

fn blend(pix: &mut Pixmap, x: i32, y: i32, c: Rgba, cov: f32) {
    if x < 0 || y < 0 || x >= pix.width() as i32 || y >= pix.height() as i32 || cov <= 0.0 {
        return;
    }
    let i = (y as usize * pix.width() as usize + x as usize) * 4;
    let a = (cov.min(1.0) * c.3 as f32 / 255.0).clamp(0.0, 1.0);
    let data = pix.data_mut();
    for (k, v) in [c.0, c.1, c.2].into_iter().enumerate() {
        data[i + k] = (v as f32 * a + data[i + k] as f32 * (1.0 - a)).round() as u8;
    }
    data[i + 3] = (255.0 * a + data[i + 3] as f32 * (1.0 - a)).round() as u8;
}

fn text_box(pix: &mut Pixmap, t: &TextBox, scale: f32, ox: f32, oy: f32) {
    let font = text::font();
    let size = t.size * scale;
    let sf = font.as_scaled(size);
    let lh = t.size * text::LINE_HEIGHT;
    for (i, line) in text::wrap(&t.text, t.size, t.w).iter().enumerate() {
        let baseline = (t.y + i as f32 * lh + (lh - t.size) / 2.0) * scale + sf.ascent() * 0.92;
        let mut x = t.x * scale;
        let mut prev = None;
        for ch in line.chars() {
            let id = sf.glyph_id(ch);
            if let Some(p) = prev {
                x += sf.kern(p, id);
            }
            let g = id.with_scale_and_position(size, point(x + ox, baseline + oy));
            if let Some(og) = font.outline_glyph(g) {
                let b = og.px_bounds();
                og.draw(|gx, gy, cov| {
                    blend(
                        pix,
                        b.min.x as i32 + gx as i32,
                        b.min.y as i32 + gy as i32,
                        t.color,
                        cov,
                    )
                });
            }
            x += sf.h_advance(id);
            prev = Some(id);
        }
    }
}

pub fn paint_item(pix: &mut Pixmap, it: &Item, paper: Paper, scale: f32) {
    paint_item_with(pix, it, paper, scale, &NoAssets, false);
}

fn paint_item_with(
    pix: &mut Pixmap,
    it: &Item,
    paper: Paper,
    scale: f32,
    a: &dyn AssetSource,
    see_through: bool,
) {
    let tf = Transform::from_scale(scale, scale);
    match &it.content {
        Content::Stroke(s) if see_through && s.pen == crate::model::PenKind::Highlighter => {
            let pts: Vec<[f32; 2]> = ink::samples(s).iter().map(|q| [q.x, q.y]).collect();
            let mut pb = PathBuilder::new();
            if let Some(f) = pts.first() {
                pb.move_to(f[0], f[1]);
                if pts.len() == 1 {
                    pb.line_to(f[0] + 0.01, f[1]);
                }
                for q in &pts[1..] {
                    pb.line_to(q[0], q[1]);
                }
            }
            if let Some(path) = pb.finish() {
                let mut p = paint(ink::highlight_tint(s.color));
                p.blend_mode = tiny_skia::BlendMode::Multiply;
                let st = tiny_skia::Stroke {
                    width: s.width,
                    line_cap: LineCap::Butt,
                    line_join: LineJoin::Round,
                    ..Default::default()
                };
                pix.stroke_path(&path, &p, &st, tf, None);
            }
        }
        Content::Stroke(s) => ink(pix, &ink::samples(s), ink::ink_color(s, paper), tf),
        Content::Shape(sh) => shape(pix, sh, tf),
        Content::Text(t) => text_box(pix, t, scale, 0.0, 0.0),
        Content::Tape(t) => tape(pix, t, tf),
        Content::Image(ImageBox { asset, x, y, w, h }) => {
            let img = a.image(*asset);
            picture(pix, img.as_deref(), [*x, *y, *w, *h], tf)
        }
    }
}

/// Renders a page `width_px` pixels wide (pictures as placeholders).
pub fn render_page(page: &Page, width_px: u32) -> Pixmap {
    render_page_with(page, width_px, &NoAssets)
}

/// Drawing order within a layer: pictures, highlighter, ink and text, then tape on top.
pub fn layer_order(kind: ItemKind) -> u8 {
    match kind {
        ItemKind::Image => 0,
        ItemKind::Highlight => 1,
        ItemKind::Tape => 3,
        _ => 2,
    }
}

pub fn render_page_with(page: &Page, width_px: u32, assets: &dyn AssetSource) -> Pixmap {
    let scale = width_px as f32 / page.w;
    let h = (page.h * scale).round().max(1.0) as u32;
    let mut pix = Pixmap::new(width_px.max(1), h).expect("pixmap");
    let tf = Transform::from_scale(scale, scale);
    paint_paper(&mut pix, page.template, page.paper, page.w, page.h, tf);
    if let Some(bg) = page.background {
        picture(&mut pix, assets.image(bg).as_deref(), [0.0, 0.0, page.w, page.h], tf);
    }
    let first = page.layers.first().map_or(0, |l| l.id);
    let layer_ids: Vec<u64> = if page.layers.is_empty() {
        vec![first]
    } else {
        page.layers.iter().filter(|l| l.visible).map(|l| l.id).collect()
    };
    let known: Vec<u64> = page.layers.iter().map(|l| l.id).collect();
    let see_through = page.has_pictures();
    for lid in layer_ids {
        let in_layer = |it: &&std::sync::Arc<Item>| {
            it.layer == lid || (lid == first && !known.contains(&it.layer))
        };
        for pass in 0..4 {
            for it in page
                .items
                .iter()
                .filter(in_layer)
                .filter(|i| layer_order(i.kind()) == pass)
            {
                paint_item_with(&mut pix, it, page.paper, scale, assets, see_through);
            }
        }
    }
    pix
}

/// Straight RGBA bytes (pages are opaque, so premultiplied equals straight).
pub fn render_rgba(page: &Page, width_px: u32) -> (u32, u32, Vec<u8>) {
    render_rgba_with(page, width_px, &NoAssets)
}

pub fn render_rgba_with(page: &Page, width_px: u32, a: &dyn AssetSource) -> (u32, u32, Vec<u8>) {
    let pix = render_page_with(page, width_px, a);
    (pix.width(), pix.height(), pix.data().to_vec())
}

pub fn render_png(page: &Page, width_px: u32) -> Vec<u8> {
    render_page(page, width_px).encode_png().unwrap_or_default()
}

pub fn render_png_with(page: &Page, width_px: u32, a: &dyn AssetSource) -> Vec<u8> {
    render_page_with(page, width_px, a)
        .encode_png()
        .unwrap_or_default()
}

/// Small preview of a blank page with a template, for pickers.
pub fn template_preview(template: Template, paper: Paper, width_px: u32) -> (u32, u32, Vec<u8>) {
    let page = Page::new(template, paper);
    let scale = width_px as f32 / page.w;
    let h = (page.h * scale).round().max(1.0) as u32;
    let mut pix = Pixmap::new(width_px.max(1), h).expect("pixmap");
    let weight = (1.0 / scale).clamp(1.0, 4.0);
    paint_paper_weighted(
        &mut pix,
        template,
        paper,
        page.w,
        page.h,
        Transform::from_scale(scale, scale),
        weight,
    );
    (pix.width(), pix.height(), pix.data().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::sync::Arc;

    #[test]
    fn draws_ink_on_page() {
        let mut page = Page::new(Template::Lined, Paper::White);
        page.items.push(Arc::new(Item::new(Content::Stroke(Stroke {
            pen: PenKind::Fountain,
            color: Rgba::rgb(0, 0, 0),
            width: 4.0,
            points: (0..30)
                .map(|i| InkPoint {
                    x: 100.0 + i as f32 * 8.0,
                    y: 300.0 + (i as f32 * 0.4).sin() * 30.0,
                    p: 0.7,
                })
                .collect(),
        }))));
        page.items.push(Arc::new(Item::new(Content::Text(TextBox {
            x: 80.0,
            y: 100.0,
            w: 400.0,
            text: "Hello notebook".into(),
            size: 24.0,
            color: Rgba::rgb(30, 90, 220),
        }))));
        let pix = render_page(&page, 600);
        // Some pixels must be dark (ink) and some blue-ish (text).
        let px = pix.data();
        let dark = px
            .chunks(4)
            .filter(|c| c[0] < 60 && c[1] < 60 && c[2] < 60)
            .count();
        let blue = px.chunks(4).filter(|c| c[2] > 150 && c[0] < 100).count();
        assert!(dark > 200, "dark={dark}");
        assert!(blue > 50, "blue={blue}");
    }
}
