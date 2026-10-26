//! GPU textures made by the CPU renderer: page thumbnails and template previews.

use crate::theme::Palette;
use egui::{ColorImage, Sense, TextureHandle, TextureOptions, Ui, vec2};
use notebook_core::assets::AssetSource;
use notebook_core::render;
use notebook_core::{Id, Page, Paper, Template};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub fn to_texture(
    ctx: &egui::Context,
    name: &str,
    (w, h, px): (u32, u32, Vec<u8>),
) -> TextureHandle {
    let img = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &px);
    ctx.load_texture(name, img, TextureOptions::LINEAR)
}

fn page_key(p: &Page) -> u64 {
    let mut h = DefaultHasher::new();
    p.template.hash(&mut h);
    p.paper.hash(&mut h);
    p.background.hash(&mut h);
    (p.w as u32, p.h as u32).hash(&mut h);
    for l in &p.layers {
        (l.id, l.visible).hash(&mut h);
    }
    for it in &p.items {
        (Arc::as_ptr(it) as usize).hash(&mut h);
    }
    h.finish()
}

/// Page thumbnails, re-rendered lazily (a few per frame) when a page changes.
#[derive(Default)]
pub struct PageThumbs {
    map: HashMap<Id, (u64, TextureHandle)>,
    budget: usize,
}

impl PageThumbs {
    pub fn begin_frame(&mut self) {
        self.budget = 2;
    }

    pub fn get(
        &mut self,
        ctx: &egui::Context,
        page: &Page,
        assets: &dyn AssetSource,
    ) -> Option<TextureHandle> {
        let key = page_key(page);
        match self.map.get(&page.id) {
            Some((k, t)) if *k == key => return Some(t.clone()),
            Some((_, t)) if self.budget == 0 => return Some(t.clone()),
            None if self.budget == 0 => {
                ctx.request_repaint();
                return None;
            }
            _ => {}
        }
        self.budget -= 1;
        let tex = to_texture(ctx, "page-thumb", render::render_rgba_with(page, 240, assets));
        self.map.insert(page.id, (key, tex.clone()));
        ctx.request_repaint();
        Some(tex)
    }
}

/// Template preview textures, shared across the app through egui's memory.
#[derive(Clone, Default)]
struct TemplateTextures(HashMap<(Template, Paper), TextureHandle>);

pub fn template_texture(ctx: &egui::Context, t: Template, p: Paper) -> TextureHandle {
    let id = egui::Id::unique("template-textures");
    if let Some(tex) = ctx
        .data(|d| d.get_temp::<TemplateTextures>(id))
        .and_then(|m| m.0.get(&(t, p)).cloned())
    {
        return tex;
    }
    let tex = to_texture(ctx, "template", render::template_preview(t, p, 150));
    ctx.data_mut(|d| {
        let mut m = d.get_temp::<TemplateTextures>(id).unwrap_or_default();
        m.0.insert((t, p), tex.clone());
        d.insert_temp(id, m);
    });
    tex
}

/// Grid of template previews. Returns true when the choice changed.
pub fn template_picker(ui: &mut Ui, value: &mut Template, paper: Paper, pal: &Palette) -> bool {
    template_picker_cols(ui, value, paper, pal, 3)
}

pub fn template_picker_cols(
    ui: &mut Ui,
    value: &mut Template,
    paper: Paper,
    pal: &Palette,
    cols: usize,
) -> bool {
    let mut changed = false;
    egui::Grid::new(ui.next_auto_id())
        .spacing(vec2(8.0, 8.0))
        .show(ui, |ui| {
            for (i, t) in Template::ALL.iter().enumerate() {
                let tex = template_texture(ui.ctx(), *t, paper);
                let (r, resp) = ui.allocate_exact_size(vec2(84.0, 128.0), Sense::click());
                let img = egui::Rect::from_min_size(r.min, vec2(84.0, 110.0));
                ui.painter().image(
                    tex.id(),
                    img,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                let sel = *value == *t;
                ui.painter().rect_stroke(
                    img,
                    6.0,
                    egui::Stroke::new(
                        if sel { 2.5 } else { 1.0 },
                        if sel { pal.accent } else { pal.line },
                    ),
                    egui::StrokeKind::Outside,
                );
                ui.painter().text(
                    egui::pos2(r.center().x, r.bottom() - 8.0),
                    egui::Align2::CENTER_CENTER,
                    t.name(),
                    egui::FontId::proportional(12.0),
                    if sel { pal.accent } else { pal.muted },
                );
                if resp.clicked() {
                    *value = *t;
                    changed = true;
                }
                if i % cols == cols - 1 {
                    ui.end_row();
                }
            }
        });
    changed
}
