//! GPU geometry cache. Each item is tessellated once (in page units) and kept;
//! a page's mesh is a cheap concatenation of its items' meshes, rebuilt only when
//! the page changes. Each frame the page mesh is just offset and scaled.

use crate::theme::c32;
use egui::epaint::{Vertex, WHITE_UV};
use egui::{Color32, Mesh, Pos2, Shape, pos2};
use notebook_core::ink::{self, Sample};
use notebook_core::templates::{Prim, template_prims};
use notebook_core::*;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

struct ItemMesh {
    // Holding the Arc keeps the pointer (the cache key) from being reused.
    _item: Arc<Item>,
    paper: Paper,
    see_through: bool,
    mesh: Mesh,
    used: u64,
}

struct PageMesh {
    key: u64,
    hl: Mesh,
    ink: Mesh,
    tape: Mesh,
    used: u64,
}

#[derive(Default)]
pub struct RenderCache {
    items: HashMap<usize, ItemMesh>,
    pages: HashMap<(u64, u64, bool), PageMesh>,
    paper: HashMap<(Template, Paper, u32, u32), Mesh>,
    frame: u64,
}

pub fn append(core: &ink::Mesh, color: Color32, out: &mut Mesh) {
    let base = out.vertices.len() as u32;
    out.vertices.extend(core.verts.iter().map(|v| Vertex {
        pos: pos2(v[0], v[1]),
        uv: WHITE_UV,
        color,
    }));
    out.indices.extend(core.idx.iter().map(|i| i + base));
}

/// Colour as shown on screen: dark ink turns light on dark paper (night mode).
pub fn display_color(c: Rgba, paper: Paper) -> Rgba {
    if paper.is_dark() && c.luminance() < 0.3 {
        Rgba(255 - c.0 / 2, 255 - c.1 / 2, 255 - c.2 / 2, c.3)
    } else {
        c
    }
}

/// Builds the mesh for one item in page units (text and pictures are drawn separately).
pub fn item_mesh(it: &Item, paper: Paper) -> Mesh {
    item_mesh_with(it, paper, false)
}

/// `see_through`: the page shows pictures, so highlighter must not hide them.
pub fn item_mesh_with(it: &Item, paper: Paper, see_through: bool) -> Mesh {
    let mut out = Mesh::default();
    let mut core = ink::Mesh::default();
    match &it.content {
        Content::Stroke(s) if see_through && s.pen == PenKind::Highlighter => {
            // No multiply blending on screen: a translucent ribbon that never overlaps itself.
            ink::ribbon_mesh(&ink::samples(s), &mut core);
            let c = Color32::from_rgba_unmultiplied(s.color.0, s.color.1, s.color.2, 110);
            append(&core, c, &mut out);
        }
        Content::Stroke(s) => {
            ink::ink_mesh(&ink::samples(s), &mut core);
            let c = if s.pen == PenKind::Highlighter {
                ink::ink_color(s, paper)
            } else {
                display_color(s.color, paper)
            };
            append(&core, c32(c), &mut out);
        }
        Content::Shape(sh) => {
            if let Some(f) = sh.fill {
                ink::shape_fill_mesh(sh, &mut core);
                let a = if sh.solid { 255 } else { 64 };
                append(
                    &core,
                    Color32::from_rgba_unmultiplied(f.0, f.1, f.2, a),
                    &mut out,
                );
                core.clear();
            }
            ink::shape_mesh(sh, &mut core);
            append(&core, c32(display_color(sh.color, paper)), &mut out);
        }
        Content::Tape(t) => {
            if t.revealed {
                ink::tape_outline_mesh(t, &mut core);
                let c = t.color;
                let shade = Rgba(c.0 / 4 * 3, c.1 / 4 * 3, c.2 / 4 * 3, 255);
                append(&core, c32(shade).gamma_multiply(0.8), &mut out);
            } else {
                ink::tape_mesh(t, &mut core);
                append(&core, c32(t.color), &mut out);
                // Diagonal stripes like washi tape: thin darker lines across the strip.
                core.clear();
                let (pts, _) = notebook_core::geom::tape_outline(t);
                let stripe = Rgba(
                    (t.color.0 as f32 * 0.9) as u8,
                    (t.color.1 as f32 * 0.9) as u8,
                    (t.color.2 as f32 * 0.9) as u8,
                    255,
                );
                if !t.rect {
                    let mut acc = 0.0;
                    for w in pts.windows(2) {
                        let (a, b) = (w[0], w[1]);
                        let len = (b[0] - a[0]).hypot(b[1] - a[1]);
                        if len < 1e-3 {
                            continue;
                        }
                        let (dx, dy) = ((b[0] - a[0]) / len, (b[1] - a[1]) / len);
                        let (nx, ny) = (-dy, dx);
                        let h = t.width / 2.0 * 0.82;
                        let mut d = (6.0 - acc % 6.0) % 6.0;
                        while d < len {
                            let c = [a[0] + dx * d, a[1] + dy * d];
                            let p1 = [c[0] + nx * h - dx * h * 0.6, c[1] + ny * h - dy * h * 0.6];
                            let p2 = [c[0] - nx * h + dx * h * 0.6, c[1] - ny * h + dy * h * 0.6];
                            ink::ink_mesh(&ink::polyline_samples(&[p1, p2], 0.9), &mut core);
                            d += 6.0;
                        }
                        acc += len;
                    }
                }
                append(&core, c32(stripe), &mut out);
            }
        }
        Content::Text(_) | Content::Image(_) => {}
    }
    out
}

pub fn samples_mesh(samples: &[Sample], color: Color32) -> Mesh {
    let mut core = ink::Mesh::default();
    ink::ink_mesh(samples, &mut core);
    let mut out = Mesh::default();
    append(&core, color, &mut out);
    out
}

fn page_key(page: &Page, paper: Paper, exclude: &[Id]) -> u64 {
    let mut h = DefaultHasher::new();
    paper.hash(&mut h);
    page.items.len().hash(&mut h);
    for it in &page.items {
        (Arc::as_ptr(it) as usize).hash(&mut h);
    }
    exclude.hash(&mut h);
    h.finish()
}

impl RenderCache {
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        if self.frame.is_multiple_of(240) {
            let f = self.frame;
            self.items.retain(|_, m| f - m.used < 600);
            self.pages.retain(|_, m| f - m.used < 600);
        }
    }

    pub fn item(&mut self, it: &Arc<Item>, paper: Paper) -> &Mesh {
        self.item_with(it, paper, false)
    }

    pub fn item_with(&mut self, it: &Arc<Item>, paper: Paper, see_through: bool) -> &Mesh {
        let key = Arc::as_ptr(it) as usize;
        let frame = self.frame;
        let fresh = self
            .items
            .get(&key)
            .is_some_and(|m| m.paper == paper && m.see_through == see_through);
        if !fresh {
            self.items.insert(
                key,
                ItemMesh {
                    _item: it.clone(),
                    paper,
                    see_through,
                    mesh: item_mesh_with(it, paper, see_through),
                    used: frame,
                },
            );
        }
        let m = self.items.get_mut(&key).expect("just inserted");
        m.used = frame;
        &m.mesh
    }

    /// Highlighter, ink and tape meshes for one layer of a page, excluding items
    /// being moved or edited.
    pub fn layer(
        &mut self,
        page: &Page,
        layer: Id,
        paper: Paper,
        exclude: &[Id],
    ) -> (&Mesh, &Mesh, &Mesh) {
        let see_through = page.has_pictures();
        let key = page_key(page, paper, exclude) ^ see_through as u64;
        let slot = (page.id, layer, !exclude.is_empty());
        let frame = self.frame;
        if self.pages.get(&slot).is_none_or(|m| m.key != key) {
            let (mut hl, mut inkm, mut tape) = (Mesh::default(), Mesh::default(), Mesh::default());
            for it in page.items_in_layer(layer) {
                if exclude.contains(&it.id)
                    || matches!(it.content, Content::Text(_) | Content::Image(_))
                {
                    continue;
                }
                let target = if it.is_highlighter() {
                    &mut hl
                } else if it.is_tape() {
                    &mut tape
                } else {
                    &mut inkm
                };
                let m = self.item_with(it, paper, see_through);
                let base = target.vertices.len() as u32;
                target.vertices.extend_from_slice(&m.vertices);
                target.indices.extend(m.indices.iter().map(|i| i + base));
            }
            self.pages.insert(
                slot,
                PageMesh {
                    key,
                    hl,
                    ink: inkm,
                    tape,
                    used: frame,
                },
            );
        }
        let pm = self.pages.get_mut(&slot).expect("present");
        pm.used = frame;
        (&pm.hl, &pm.ink, &pm.tape)
    }

    pub fn paper(&mut self, page: &Page, paper: Paper) -> &Mesh {
        let key = (page.template, paper, page.w as u32, page.h as u32);
        self.paper.entry(key).or_insert_with(|| {
            let mut core = ink::Mesh::default();
            let mut out = Mesh::default();
            for p in template_prims(page.template, page.w, page.h, 1.0) {
                core.clear();
                let color = match p {
                    Prim::Line { a, b, w, margin } => {
                        ink::ink_mesh(
                            &[
                                Sample {
                                    x: a[0],
                                    y: a[1],
                                    w,
                                },
                                Sample {
                                    x: b[0],
                                    y: b[1],
                                    w,
                                },
                            ],
                            &mut core,
                        );
                        if margin {
                            paper.margin()
                        } else {
                            paper.line()
                        }
                    }
                    Prim::Dot { c, r } => {
                        ink::ink_mesh(
                            &[Sample {
                                x: c[0],
                                y: c[1],
                                w: r * 2.0,
                            }],
                            &mut core,
                        );
                        paper.line()
                    }
                    Prim::Rect { r, w } => {
                        let pts = [
                            [r.x, r.y],
                            [r.right(), r.y],
                            [r.right(), r.bottom()],
                            [r.x, r.bottom()],
                            [r.x, r.y],
                        ];
                        ink::ink_mesh(&ink::polyline_samples(&pts, w), &mut core);
                        paper.line()
                    }
                };
                append(&core, c32(color), &mut out);
            }
            out
        })
    }
}

/// Copies a page-unit mesh to screen space.
pub fn place(mesh: &Mesh, origin: Pos2, zoom: f32) -> Shape {
    let mut m = Mesh {
        indices: mesh.indices.clone(),
        vertices: Vec::with_capacity(mesh.vertices.len()),
        texture_id: mesh.texture_id,
    };
    m.vertices.extend(mesh.vertices.iter().map(|v| Vertex {
        pos: origin + v.pos.to_vec2() * zoom,
        ..*v
    }));
    Shape::mesh(m)
}

/// Like [`place`] but with an extra transform in page units (for moving/scaling a selection).
pub fn place_transformed(mesh: &Mesh, origin: Pos2, zoom: f32, f: impl Fn(Pos2) -> Pos2) -> Shape {
    let mut m = Mesh {
        indices: mesh.indices.clone(),
        vertices: Vec::with_capacity(mesh.vertices.len()),
        texture_id: mesh.texture_id,
    };
    m.vertices.extend(mesh.vertices.iter().map(|v| Vertex {
        pos: origin + f(v.pos).to_vec2() * zoom,
        ..*v
    }));
    Shape::mesh(m)
}
