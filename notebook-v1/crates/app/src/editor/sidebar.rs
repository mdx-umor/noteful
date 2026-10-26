//! Page list sidebar (pages and bookmarks) and the full-screen page overview.

use super::*;
use crate::theme::Palette;
use egui::{
    Align2, Color32, FontId, Id as EId, RichText, Sense, Stroke as EStroke, StrokeKind, Ui,
};

impl Editor {
    pub fn sidebar_ui(&mut self, ui: &mut Ui, _prefs: &mut Prefs, pal: &Palette) {
        self.thumbs.begin_frame();
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.side_tab, SideTab::Pages, "Pages");
            ui.selectable_value(&mut self.side_tab, SideTab::Bookmarks, "🔖");
            ui.selectable_value(&mut self.side_tab, SideTab::Outline, "Outline");
            ui.selectable_value(&mut self.side_tab, SideTab::Layers, "Layers");
        });
        ui.separator();
        let ctx = ui.ctx().clone();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match self.side_tab {
                SideTab::Pages => {
                    let w = (ui.available_width() - 16.0).clamp(60.0, 260.0);
                    for i in 0..self.doc.pages.len() {
                        let page = &self.doc.pages[i];
                        let h = w * page.h / page.w;
                        let (r, resp) = ui.allocate_exact_size(
                            vec2(ui.available_width(), h + 24.0),
                            Sense::click(),
                        );
                        if !ui.is_rect_visible(r) {
                            continue;
                        }
                        let img =
                            Rect::from_min_size(pos2(r.center().x - w / 2.0, r.top()), vec2(w, h));
                        let page = page.clone();
                        match self.thumbs.get(&ctx, &page, &self.assets) {
                            Some(t) => {
                                ui.painter().image(
                                    t.id(),
                                    img,
                                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                            None => {
                                ui.painter().rect_filled(
                                    img,
                                    2.0,
                                    crate::theme::c32(page.paper.background()),
                                );
                            }
                        }
                        let cur = i == self.current;
                        ui.painter().rect_stroke(
                            img,
                            3.0,
                            EStroke::new(
                                if cur { 2.5 } else { 1.0 },
                                if cur { pal.accent } else { pal.line },
                            ),
                            StrokeKind::Outside,
                        );
                        let label = match &page.bookmark {
                            Some(b) => format!("{}  🔖 {b}", i + 1),
                            None => format!("{}", i + 1),
                        };
                        ui.painter().text(
                            pos2(r.center().x, r.bottom() - 10.0),
                            Align2::CENTER_CENTER,
                            label,
                            FontId::proportional(12.0),
                            if cur { pal.accent } else { pal.muted },
                        );
                        if resp.clicked() {
                            self.scroll_to = Some(i);
                        }
                        resp.context_menu(|ui| self.page_context_menu(ui, i));
                    }
                    if ui.button("+ Add page").clicked() {
                        self.insert_page(self.doc.pages.len() - 1);
                    }
                }
                SideTab::Bookmarks => {
                    let marks: Vec<(usize, String)> = self
                        .doc
                        .pages
                        .iter()
                        .enumerate()
                        .filter_map(|(i, p)| p.bookmark.clone().map(|b| (i, b)))
                        .collect();
                    if marks.is_empty() {
                        ui.label(
                            RichText::new("No bookmarks yet. Use ⋯ → Bookmark page.")
                                .color(pal.muted),
                        );
                    }
                    if ui.button("+ Bookmark this page").clicked() {
                        let i = self.current;
                        let name = self.doc.pages[i]
                            .bookmark
                            .clone()
                            .unwrap_or_else(|| format!("Page {}", i + 1));
                        self.bookmark_edit = Some((i, name));
                    }
                    for (i, b) in marks {
                        let r = ui.add(
                            egui::Button::new(
                                RichText::new(format!("🔖  {b}   ·  p.{}", i + 1)).color(
                                    if i == self.current { pal.accent } else { pal.text },
                                ),
                            )
                            .frame(false),
                        );
                        if r.clicked() {
                            self.scroll_to = Some(i);
                        }
                        r.context_menu(|ui| {
                            if ui.button("Rename").clicked() {
                                self.bookmark_edit = Some((i, b.clone()));
                                ui.close();
                            }
                            if ui.button("Remove").clicked() {
                                self.set_bookmark(i, None);
                                ui.close();
                            }
                        });
                    }
                }
                SideTab::Outline => self.outline_tab(ui, pal),
                SideTab::Layers => self.layers_tab(ui, pal),
            });
    }

    fn outline_tab(&mut self, ui: &mut Ui, pal: &Palette) {
        if ui.button("+ Add entry for this page").clicked() {
            self.add_outline();
        }
        if self.doc.outline.is_empty() {
            ui.label(
                RichText::new("No outline yet. Add entries to build a table of contents.")
                    .color(pal.muted),
            );
            return;
        }
        // The section the current page belongs to is shown in bold.
        let cur_section = self
            .doc
            .outline
            .iter()
            .filter_map(|e| self.doc.page_index(e.page).map(|p| (p, e.id)))
            .filter(|(p, _)| *p <= self.current)
            .max_by_key(|(p, _)| *p)
            .map(|(_, id)| id);
        let entries = self.doc.outline.clone();
        let mut action: Option<(usize, u8)> = None; // (index, 0 rename 1 indent 2 outdent 3 delete)
        for (k, e) in entries.iter().enumerate() {
            let page = self.doc.page_index(e.page);
            ui.horizontal(|ui| {
                ui.add_space(e.level as f32 * 16.0);
                let mut text = RichText::new(&e.title);
                if Some(e.id) == cur_section {
                    text = text.strong().color(pal.accent);
                }
                let r = ui.add(egui::Button::new(text).frame(false).truncate());
                if r.clicked()
                    && let Some(p) = page
                {
                    self.scroll_to = Some(p);
                }
                r.context_menu(|ui| {
                    for (label, code) in [("Rename", 0u8), ("Indent", 1), ("Outdent", 2), ("Delete", 3)] {
                        if ui.button(label).clicked() {
                            action = Some((k, code));
                            ui.close();
                        }
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(page.map_or("–".into(), |p| (p + 1).to_string()))
                            .color(pal.muted),
                    );
                });
            });
        }
        if let Some((k, code)) = action {
            match code {
                0 => {
                    let e = &self.doc.outline[k];
                    self.edit = Some(EditDialog::OutlineTitle(e.id, e.title.clone()));
                }
                1 => self.doc.outline[k].level = (self.doc.outline[k].level + 1).min(4),
                2 => self.doc.outline[k].level = self.doc.outline[k].level.saturating_sub(1),
                _ => {
                    self.doc.outline.remove(k);
                }
            }
            self.touch();
        }
        ui.label(
            RichText::new("Right-click / long-press an entry to rename, indent or delete it.")
                .color(pal.muted)
                .size(12.0),
        );
    }

    fn layers_tab(&mut self, ui: &mut Ui, pal: &Palette) {
        let i = self.current;
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Page {}", i + 1)).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+ Layer").clicked() {
                    self.add_layer(i);
                }
            });
        });
        let active = self.active_layer_id(i);
        let layers = self.doc.pages[i].layers.clone();
        let n = layers.len();
        let pid = self.doc.pages[i].id;
        // Top layer first, like a stack of transparent sheets.
        for (pos, l) in layers.iter().enumerate().rev() {
            let is_active = l.id == active;
            let count = self.doc.pages[i].items.iter().filter(|it| it.layer == l.id).count();
            egui::Frame::new()
                .fill(if is_active { pal.hover } else { Color32::TRANSPARENT })
                .corner_radius(10)
                .inner_margin(6)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let name = RichText::new(&l.name).color(if is_active {
                            pal.accent
                        } else {
                            pal.text
                        });
                        let r = ui
                            .add(egui::Button::new(name).frame(false).truncate())
                            .on_hover_text("Draw on this layer");
                        if r.clicked() {
                            self.active_layer.insert(pid, l.id);
                        }
                        r.context_menu(|ui| {
                            if ui.button("Rename").clicked() {
                                self.edit = Some(EditDialog::LayerName(i, l.id, l.name.clone()));
                                ui.close();
                            }
                            if ui.button("Duplicate").clicked() {
                                self.duplicate_layer(i, l.id);
                                ui.close();
                            }
                            if ui.add_enabled(pos > 0, egui::Button::new("Merge down")).clicked() {
                                self.merge_down(i, l.id);
                                ui.close();
                            }
                            if ui.add_enabled(pos + 1 < n, egui::Button::new("Move up")).clicked() {
                                self.move_layer(i, l.id, true);
                                ui.close();
                            }
                            if ui.add_enabled(pos > 0, egui::Button::new("Move down")).clicked() {
                                self.move_layer(i, l.id, false);
                                ui.close();
                            }
                            if ui.add_enabled(n > 1, egui::Button::new("Delete layer")).clicked() {
                                self.delete_layer(i, l.id);
                                ui.close();
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let eye = if l.visible { "👁" } else { "◌" };
                            if ui
                                .selectable_label(!l.visible, eye)
                                .on_hover_text(if l.visible { "Hide" } else { "Show" })
                                .clicked()
                            {
                                self.update_layer(i, l.id, |l| l.visible = !l.visible);
                            }
                            let lock = if l.locked { "🔒" } else { "🔓" };
                            if ui
                                .selectable_label(l.locked, lock)
                                .on_hover_text(if l.locked { "Unlock" } else { "Lock" })
                                .clicked()
                            {
                                self.update_layer(i, l.id, |l| l.locked = !l.locked);
                            }
                            if is_active {
                                ui.label(RichText::new("✏").color(pal.accent))
                                    .on_hover_text("New writing goes here");
                            }
                            ui.label(RichText::new(count.to_string()).color(pal.muted).size(11.0));
                        });
                    });
                });
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Tap a layer to draw on it. Hide a layer to make an answer sheet; lock it to protect it. Right-click / long-press for more.",
            )
            .color(pal.muted)
            .size(12.0),
        );
    }

    fn page_context_menu(&mut self, ui: &mut Ui, i: usize) {
        if ui.button("Insert page after").clicked() {
            self.insert_page(i);
            ui.close();
        }
        if ui.button("Duplicate").clicked() {
            self.duplicate_page(i);
            ui.close();
        }
        if ui
            .add_enabled(i > 0, egui::Button::new("Move up"))
            .clicked()
        {
            self.move_page(i, false);
            ui.close();
        }
        if ui
            .add_enabled(i + 1 < self.doc.pages.len(), egui::Button::new("Move down"))
            .clicked()
        {
            self.move_page(i, true);
            ui.close();
        }
        if ui
            .button(if self.doc.pages[i].bookmark.is_some() {
                "Rename bookmark"
            } else {
                "Bookmark"
            })
            .clicked()
        {
            let name = self.doc.pages[i]
                .bookmark
                .clone()
                .unwrap_or_else(|| format!("Page {}", i + 1));
            self.bookmark_edit = Some((i, name));
            ui.close();
        }
        if ui.button("Add to outline").clicked() {
            self.scroll_to = Some(i);
            self.current = i;
            self.add_outline();
            ui.close();
        }
        if ui.button("Clear page").clicked() {
            self.clear_page(i);
            ui.close();
        }
        if ui.button("Delete page").clicked() {
            self.delete_page(i);
            ui.close();
        }
    }

    pub fn overview_ui(&mut self, ctx: &egui::Context, _prefs: &mut Prefs, pal: &Palette) {
        self.thumbs.begin_frame();
        let screen = ctx.content_rect();
        let mut close = false;
        egui::Modal::new(EId::unique("overview"))
            .frame(
                egui::Frame::new()
                    .fill(pal.bg)
                    .corner_radius(16)
                    .inner_margin(16),
            )
            .show(ctx, |ui| {
                ui.set_width((screen.width() - 80.0).max(300.0));
                ui.set_height((screen.height() - 80.0).max(300.0));
                ui.horizontal(|ui| {
                    ui.heading(format!("{} pages", self.doc.pages.len()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Done").clicked() {
                            close = true;
                        }
                        if ui.button("+ Page").clicked() {
                            self.insert_page(self.doc.pages.len() - 1);
                        }
                    });
                });
                ui.label(
                    RichText::new(
                        "Tap a page to open it. Long-press or right-click for page actions.",
                    )
                    .color(pal.muted),
                );
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let w = 150.0;
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(16.0, 16.0);
                            for i in 0..self.doc.pages.len() {
                                let page = self.doc.pages[i].clone();
                                let h = w * page.h / page.w;
                                let (r, resp) =
                                    ui.allocate_exact_size(vec2(w, h + 26.0), Sense::click());
                                if !ui.is_rect_visible(r) {
                                    continue;
                                }
                                let img = Rect::from_min_size(r.min, vec2(w, h));
                                match self.thumbs.get(ctx, &page, &self.assets) {
                                    Some(t) => {
                                        ui.painter().image(
                                            t.id(),
                                            img,
                                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                            Color32::WHITE,
                                        );
                                    }
                                    None => {
                                        ui.painter().rect_filled(
                                            img,
                                            2.0,
                                            crate::theme::c32(page.paper.background()),
                                        );
                                    }
                                }
                                let cur = i == self.current;
                                ui.painter().rect_stroke(
                                    img,
                                    4.0,
                                    EStroke::new(
                                        if cur { 3.0 } else { 1.0 },
                                        if cur { pal.accent } else { pal.line },
                                    ),
                                    StrokeKind::Outside,
                                );
                                let label = page
                                    .bookmark
                                    .clone()
                                    .map(|b| format!("{}  🔖 {b}", i + 1))
                                    .unwrap_or_else(|| format!("{}", i + 1));
                                ui.painter().text(
                                    pos2(r.center().x, r.bottom() - 10.0),
                                    Align2::CENTER_CENTER,
                                    label,
                                    FontId::proportional(13.0),
                                    pal.muted,
                                );
                                if resp.clicked() {
                                    self.scroll_to = Some(i);
                                    close = true;
                                }
                                resp.context_menu(|ui| self.page_context_menu(ui, i));
                            }
                        });
                    });
            });
        if close {
            self.overview = false;
        }
    }
}
