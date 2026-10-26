//! Picture cache for the editor: decoded images (for thumbnails) and GPU textures
//! (for the canvas). Pictures far off screen are dropped after a while.

use notebook_core::assets::{AssetSource, RgbaImage};
use notebook_core::storage::Store;
use notebook_core::Id;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

pub struct Assets {
    store: Store,
    textures: HashMap<Id, (egui::TextureHandle, [f32; 2], u64)>,
    decoded: RefCell<(HashMap<Id, Arc<RgbaImage>>, VecDeque<Id>)>,
    missing: HashSet<Id>,
    frame: u64,
    /// Decoding is slow; limit how many new pictures become textures per frame.
    budget: usize,
}

impl Assets {
    pub fn new(store: Store) -> Self {
        Assets {
            store,
            textures: HashMap::new(),
            decoded: RefCell::new((HashMap::new(), VecDeque::new())),
            missing: HashSet::new(),
            frame: 0,
            budget: 1,
        }
    }

    pub fn begin_frame(&mut self) {
        self.frame += 1;
        self.budget = 1;
        if self.frame.is_multiple_of(120) {
            let f = self.frame;
            self.textures.retain(|_, t| f - t.2 < 600);
        }
    }

    /// Saves JPEG bytes as a new picture.
    pub fn add_jpeg(&mut self, bytes: &[u8]) -> std::io::Result<Id> {
        self.store.save_asset(bytes)
    }

    /// GPU texture for a picture, or None while it is still loading (or missing).
    pub fn texture(&mut self, ctx: &egui::Context, id: Id) -> Option<egui::TextureId> {
        if let Some(t) = self.textures.get_mut(&id) {
            t.2 = self.frame;
            return Some(t.0.id());
        }
        if self.missing.contains(&id) {
            return None;
        }
        if self.budget == 0 {
            ctx.request_repaint();
            return None;
        }
        self.budget -= 1;
        let Some(img) = self.image(id) else {
            self.missing.insert(id);
            return None;
        };
        let ci =
            egui::ColorImage::from_rgba_unmultiplied([img.w as usize, img.h as usize], &img.px);
        let tex = ctx.load_texture(format!("asset-{id}"), ci, egui::TextureOptions::LINEAR);
        let tid = tex.id();
        self.textures
            .insert(id, (tex, [img.w as f32, img.h as f32], self.frame));
        ctx.request_repaint();
        Some(tid)
    }
}

impl AssetSource for Assets {
    fn image(&self, id: Id) -> Option<Arc<RgbaImage>> {
        let mut d = self.decoded.borrow_mut();
        if let Some(img) = d.0.get(&id) {
            return Some(img.clone());
        }
        let img = self.store.image(id)?;
        d.0.insert(id, img.clone());
        d.1.push_back(id);
        while d.1.len() > 6 {
            if let Some(old) = d.1.pop_front() {
                d.0.remove(&old);
            }
        }
        Some(img)
    }
}
