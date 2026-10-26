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
    mesh: Mesh,
    used: u64,
}

struct PageMesh {
    key: u64,
    hl: Mesh,
    ink: Mesh,
    used: u64,
}

#[derive(Default)]
pub struct RenderCache {
    items: HashMap<usize, ItemMesh>,
    pages: HashMap<(u64, bool), PageMesh>,
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

/// Builds the mesh for one item in page units (text is drawn separately).
pub fn item_mesh(it: &Item, paper: Paper) -> Mesh {
    let mut out = Mesh::default();
    let mut core = ink::Mesh::default();
    match &it.content {
        Content::Stroke(s) => {
            ink::ink_mesh(&ink::samples(s), &mut core);
            append(&core, c32(ink::ink_color(s, paper)), &mut out);
        }
        Content::Shape(sh) => {
            if let Some(f) = sh.fill {
                ink::shape_fill_mesh(sh, &mut core);
                append(
                    &core,
                    Color32::from_rgba_unmultiplied(f.0, f.1, f.2, 64),
                    &mut out,
                );
                core.clear();
            }
            ink::shape_mesh(sh, &mut core);
            append(&core, c32(sh.color), &mut out);
        }
        Content::Text(_) => {}
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

fn page_key(page: &Page, exclude: &[Id]) -> u64 {
    let mut h = DefaultHasher::new();
    page.paper.hash(&mut h);
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
        let key = Arc::as_ptr(it) as usize;
        let frame = self.frame;
        let fresh = self.items.get(&key).is_some_and(|m| m.paper == paper);
        if !fresh {
            self.items.insert(
                key,
                ItemMesh {
                    _item: it.clone(),
                    paper,
                    mesh: item_mesh(it, paper),
                    used: frame,
                },
            );
        }
        let m = self.items.get_mut(&key).expect("just inserted");
        m.used = frame;
        &m.mesh
    }

    /// Highlighter and ink meshes for a page, excluding items being moved or edited.
    pub fn page(&mut self, page: &Page, exclude: &[Id]) -> (&Mesh, &Mesh) {
        let key = page_key(page, exclude);
        let slot = (page.id, !exclude.is_empty());
        let frame = self.frame;
        if self.pages.get(&slot).is_none_or(|m| m.key != key) {
            let (mut hl, mut inkm) = (Mesh::default(), Mesh::default());
            for it in &page.items {
                if exclude.contains(&it.id) || matches!(it.content, Content::Text(_)) {
                    continue;
                }
                let target = if it.is_highlighter() {
                    &mut hl
                } else {
                    &mut inkm
                };
                let m = self.item(it, page.paper);
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
                    used: frame,
                },
            );
        }
        let pm = self.pages.get_mut(&slot).expect("present");
        pm.used = frame;
        (&pm.hl, &pm.ink)
    }

    pub fn paper(&mut self, page: &Page) -> &Mesh {
        let key = (page.template, page.paper, page.w as u32, page.h as u32);
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
                            page.paper.margin()
                        } else {
                            page.paper.line()
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
                        page.paper.line()
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
                        page.paper.line()
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
