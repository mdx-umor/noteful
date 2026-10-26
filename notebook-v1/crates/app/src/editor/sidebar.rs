//! Page list sidebar (pages and bookmarks) and the full-screen page overview.

use super::*;
use crate::theme::Palette;
use egui::{
    Align2, Color32, FontId, Id as EId, RichText, Sense, Stroke as EStroke, StrokeKind, Ui,
};

impl Editor {
    pub fn sidebar_ui(&mut self, ui: &mut Ui, _prefs: &mut Prefs, pal: &Palette) {
        self.thumbs.begin_frame();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.side_tab, SideTab::Pages, "Pages");
            ui.selectable_value(&mut self.side_tab, SideTab::Bookmarks, "Bookmarks");
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
                        match self.thumbs.get(&ctx, &page) {
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
                    for (i, b) in marks {
                        let r = ui.add(
                            egui::Button::new(format!("🔖  {b}   ·  p.{}", i + 1)).frame(false),
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
            });
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
                                match self.thumbs.get(ctx, &page) {
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
