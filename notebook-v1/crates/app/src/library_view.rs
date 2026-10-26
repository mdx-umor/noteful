//! Library screen: sidebar with smart lists, folders and nested tags; notebook grid.

use crate::icons::{self, Icon};
use crate::prefs::Prefs;
use crate::textures;
use crate::theme::{self, Palette, c32};
use egui::{
    Align, Align2, Color32, FontId, Frame, Id as EId, Key, Layout, Margin, Order, Rect, RichText,
    Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use notebook_core::library::{
    Filter, Folder, Library, NotebookMeta, SortBy, TagNode, normalize_tag,
};
use notebook_core::{Id, Paper, Rgba, Template, new_id, now_ms};
use std::collections::{HashMap, HashSet};

pub const COVERS: [&str; 8] = [
    "#5b5bd6", "#0a84ff", "#30b0c7", "#34c759", "#ffcc00", "#ff9500", "#ff3b30", "#af52de",
];

/// Things the library asks the app to do.
pub enum LibAction {
    Open(Id),
    Create {
        title: String,
        cover: Rgba,
        template: Template,
        paper: Paper,
        folder: Option<Id>,
        tags: Vec<String>,
    },
    QuickNote,
    Duplicate(Id),
    DeleteForever(Vec<Id>),
    EmptyTrash,
    ExportPdf(Id),
    BackupNow,
    Changed,
}

enum Dialog {
    New {
        title: String,
        cover: Rgba,
        template: Template,
        paper: Paper,
        folder: Option<Id>,
        tags: String,
    },
    Rename(Id, String),
    Tags(Vec<Id>, String),
    MoveTo(Vec<Id>),
    NewFolder(String),
    RenameFolder(Id, String),
    RenameTag(String, String),
    ConfirmDelete(Vec<Id>),
    ConfirmEmptyTrash,
}

pub struct LibraryView {
    pub filter: Filter,
    search: String,
    searching: bool,
    selecting: bool,
    selected: HashSet<Id>,
    dialog: Option<Dialog>,
    expanded: HashSet<String>,
    notebooks_open: bool,
    folders_open: bool,
    sidebar_open: bool,
}

impl Default for LibraryView {
    fn default() -> Self {
        LibraryView {
            filter: Filter::All,
            search: String::new(),
            searching: false,
            selecting: false,
            selected: HashSet::new(),
            dialog: None,
            expanded: HashSet::new(),
            notebooks_open: true,
            folders_open: true,
            sidebar_open: true,
        }
    }
}

fn filter_title(f: &Filter, lib: &Library) -> String {
    match f {
        Filter::All => "Notebooks".into(),
        Filter::Untagged => "Untagged".into(),
        Filter::Unfiled => "Unfiled".into(),
        Filter::Trashed => "Trash".into(),
        Filter::Pinned => "Pinned".into(),
        Filter::Folder(id) => lib
            .folders
            .iter()
            .find(|x| x.id == *id)
            .map(|x| x.name.clone())
            .unwrap_or_else(|| "Folder".into()),
        Filter::Tag(t) => format!("#{t}"),
    }
}

fn filter_icon(f: &Filter) -> Icon {
    match f {
        Filter::All => Icon::Notebook,
        Filter::Untagged => Icon::Untagged,
        Filter::Unfiled => Icon::Unfiled,
        Filter::Trashed => Icon::Trash,
        Filter::Pinned => Icon::Pin,
        Filter::Folder(_) => Icon::Folder,
        Filter::Tag(_) => Icon::Hash,
    }
}

/// One sidebar row: icon, label, count, optional expand chevron.
fn side_row(
    ui: &mut Ui,
    pal: &Palette,
    icon: Icon,
    label: &str,
    count: Option<usize>,
    selected: bool,
    indent: f32,
    chevron: Option<bool>,
) -> (egui::Response, bool) {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
    if selected {
        ui.painter().rect_filled(r, 14.0, pal.hover);
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(r, 14.0, pal.hover.gamma_multiply(0.5));
    }
    let ir = Rect::from_center_size(
        pos2(r.left() + 22.0 + indent, r.center().y),
        vec2(20.0, 20.0),
    );
    icons::paint(ui.painter(), ir, icon, pal.text);
    let mut text_x = ir.right() + 10.0;
    ui.painter().text(
        pos2(text_x, r.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(16.5),
        pal.text,
    );
    text_x += ui.fonts_mut(|f| {
        f.layout_no_wrap(label.to_string(), FontId::proportional(16.5), pal.text)
            .size()
            .x
    }) + 8.0;
    if let Some(c) = count {
        ui.painter().text(
            pos2(text_x, r.center().y + 1.0),
            Align2::LEFT_CENTER,
            c.to_string(),
            FontId::proportional(13.0),
            pal.muted,
        );
    }
    let mut toggled = false;
    if let Some(open) = chevron {
        let cr = Rect::from_center_size(pos2(r.right() - 20.0, r.center().y), vec2(24.0, 24.0));
        icons::paint(
            ui.painter(),
            cr,
            if open {
                Icon::Chevron
            } else {
                Icon::ChevronRight
            },
            pal.accent,
        );
        if resp.clicked()
            && resp
                .interact_pointer_pos()
                .is_some_and(|p| p.x > r.right() - 40.0)
        {
            toggled = true;
        }
    }
    (resp, toggled)
}

impl LibraryView {
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        lib: &mut Library,
        prefs: &mut Prefs,
        thumbs: &mut HashMap<Id, (u64, egui::TextureHandle)>,
        store: &notebook_core::storage::Store,
    ) -> Vec<LibAction> {
        let pal = theme::palette(prefs.dark_ui);
        let mut actions = Vec::new();
        let narrow = ui.available_width() < 700.0;
        if narrow && self.sidebar_open && ui.ctx().input(|i| i.time) < 0.5 {
            self.sidebar_open = false;
        }
        if self.sidebar_open {
            egui::Panel::left("lib-side")
                .exact_size(if narrow { 240.0 } else { 270.0 })
                .resizable(false)
                .frame(
                    Frame::new()
                        .fill(pal.sidebar)
                        .inner_margin(Margin::symmetric(12, 14)),
                )
                .show(ui, |ui| self.sidebar(ui, lib, prefs, &pal, &mut actions));
        }
        egui::CentralPanel::no_frame()
            .frame(
                Frame::new()
                    .fill(pal.bg)
                    .inner_margin(Margin::symmetric(24, 16)),
            )
            .show(ui, |ui| {
                self.main(ui, lib, prefs, thumbs, store, &pal, &mut actions);
            });
        self.fab(ui.ctx(), &pal, prefs, &mut actions);
        self.dialogs(ui.ctx(), lib, prefs, &pal, &mut actions);
        actions
    }

    fn sidebar(
        &mut self,
        ui: &mut Ui,
        lib: &mut Library,
        prefs: &mut Prefs,
        pal: &Palette,
        actions: &mut Vec<LibAction>,
    ) {
        ui.horizontal(|ui| {
            let (r, resp) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
            ui.painter()
                .rect_filled(r, 9.0, Color32::from_rgb(0x5b, 0x3c, 0xf0));
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                "N",
                FontId::proportional(22.0),
                Color32::WHITE,
            );
            ui.label(RichText::new("Notebook").strong().size(18.0));
            resp.context_menu(|ui| self.settings_menu(ui, prefs, actions));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let g = icons::button(ui, Icon::Gear, false, "Settings");
                egui::Popup::menu(&g).show(|ui| self.settings_menu(ui, prefs, actions));
            });
        });
        ui.add_space(12.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let (r, t) = side_row(ui, pal, Icon::Notebook, "Notebooks", None, self.filter == Filter::All, 0.0, Some(self.notebooks_open));
            if t {
                self.notebooks_open = !self.notebooks_open;
            } else if r.clicked() {
                self.set_filter(Filter::All);
            }
            if self.notebooks_open {
                for f in [Filter::Untagged, Filter::Unfiled, Filter::Trashed] {
                    let (r, _) = side_row(ui, pal, filter_icon(&f), &filter_title(&f, lib), Some(lib.count(&f)), self.filter == f, 20.0, None);
                    if r.clicked() {
                        self.set_filter(f);
                    }
                }
            }
            let (r, _) = side_row(ui, pal, Icon::Pin, "Pinned", Some(lib.count(&Filter::Pinned)), self.filter == Filter::Pinned, 0.0, None);
            if r.clicked() {
                self.set_filter(Filter::Pinned);
            }
            let (r, t) = side_row(ui, pal, Icon::Folder, "Folders", None, false, 0.0, Some(self.folders_open));
            if t || r.clicked() {
                self.folders_open = !self.folders_open;
            }
            if self.folders_open {
                let folders = lib.folders.clone();
                for f in folders {
                    let filt = Filter::Folder(f.id);
                    let (r, _) = side_row(ui, pal, Icon::Folder, &f.name, Some(lib.count(&filt)), self.filter == filt, 20.0, None);
                    if r.clicked() {
                        self.set_filter(filt.clone());
                    }
                    r.context_menu(|ui| {
                        if ui.button("Rename folder").clicked() {
                            self.dialog = Some(Dialog::RenameFolder(f.id, f.name.clone()));
                            ui.close();
                        }
                        if ui.button("Delete folder (keeps notebooks)").clicked() {
                            lib.delete_folder(f.id);
                            if self.filter == filt {
                                self.set_filter(Filter::All);
                            }
                            actions.push(LibAction::Changed);
                            ui.close();
                        }
                    });
                }
                let (r, _) = side_row(ui, pal, Icon::Plus, "New folder", None, false, 20.0, None);
                if r.clicked() {
                    self.dialog = Some(Dialog::NewFolder(String::new()));
                }
            }
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);
            let tree = lib.tag_tree();
            if tree.is_empty() {
                ui.label(RichText::new("Tags appear here. Add one from a notebook's menu, e.g. School/Physics.").color(pal.muted).size(13.0));
            }
            for node in &tree {
                self.tag_rows(ui, lib, pal, node, 0.0, actions);
            }
        });
    }

    fn tag_rows(
        &mut self,
        ui: &mut Ui,
        lib: &mut Library,
        pal: &Palette,
        node: &TagNode,
        indent: f32,
        actions: &mut Vec<LibAction>,
    ) {
        let filt = Filter::Tag(node.path.clone());
        let open = self.expanded.contains(&node.path)
            || indent == 0.0 && !self.expanded.contains(&format!("!{}", node.path));
        let chevron = (!node.children.is_empty()).then_some(open);
        let (r, t) = side_row(
            ui,
            pal,
            Icon::Hash,
            &node.name,
            Some(node.count),
            self.filter == filt,
            indent,
            chevron,
        );
        if t {
            if indent == 0.0 {
                let k = format!("!{}", node.path);
                if !self.expanded.remove(&k) {
                    self.expanded.insert(k);
                }
            } else if !self.expanded.remove(&node.path) {
                self.expanded.insert(node.path.clone());
            }
        } else if r.clicked() {
            self.set_filter(filt.clone());
        }
        r.context_menu(|ui| {
            if ui.button("Rename tag").clicked() {
                self.dialog = Some(Dialog::RenameTag(node.path.clone(), node.path.clone()));
                ui.close();
            }
            if ui.button("Delete tag").clicked() {
                lib.delete_tag(&node.path);
                if self.filter == filt {
                    self.set_filter(Filter::All);
                }
                actions.push(LibAction::Changed);
                ui.close();
            }
        });
        if open {
            for c in &node.children {
                self.tag_rows(ui, lib, pal, c, indent + 20.0, actions);
            }
        }
    }

    fn settings_menu(&mut self, ui: &mut Ui, prefs: &mut Prefs, actions: &mut Vec<LibAction>) {
        ui.set_min_width(240.0);
        ui.checkbox(&mut prefs.dark_ui, "Dark interface");
        ui.checkbox(&mut prefs.pencil_only, "Only Apple Pencil writes");
        ui.checkbox(&mut prefs.two_finger_undo, "Two-finger tap to undo");
        ui.checkbox(&mut prefs.auto_backup, "Daily auto-backup");
        if ui.button("Back up now").clicked() {
            actions.push(LibAction::BackupNow);
            ui.close();
        }
        ui.label(
            RichText::new("Exports and backups are in Files › On My iPad › Notebook.").size(12.0),
        );
    }

    #[cfg(feature = "autotest")]
    pub fn start_new(&mut self, prefs: &Prefs) {
        self.dialog = Some(Dialog::New {
            title: String::new(),
            cover: Rgba::from_hex(COVERS[0]).unwrap_or(Rgba::BLACK),
            template: prefs.new_template,
            paper: prefs.new_paper,
            folder: None,
            tags: String::new(),
        });
    }

    fn set_filter(&mut self, f: Filter) {
        self.filter = f;
        self.selected.clear();
        self.selecting = false;
    }

    #[allow(clippy::too_many_arguments)]
    fn main(
        &mut self,
        ui: &mut Ui,
        lib: &mut Library,
        prefs: &mut Prefs,
        thumbs: &mut HashMap<Id, (u64, egui::TextureHandle)>,
        store: &notebook_core::storage::Store,
        pal: &Palette,
        actions: &mut Vec<LibAction>,
    ) {
        // Header: title on the left, actions on the right.
        ui.horizontal(|ui| {
            if !self.sidebar_open
                && icons::button(ui, Icon::Sidebar, false, "Show sidebar").clicked()
            {
                self.sidebar_open = true;
            } else if self.sidebar_open
                && ui.available_width() < 600.0
                && icons::button(ui, Icon::Sidebar, true, "Hide sidebar").clicked()
            {
                self.sidebar_open = false;
            }
            let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
            icons::paint(ui.painter(), r, filter_icon(&self.filter), pal.text);
            ui.label(
                RichText::new(filter_title(&self.filter, lib))
                    .size(30.0)
                    .strong(),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icons::button(ui, Icon::Quick, false, "Quick note").clicked() {
                    actions.push(LibAction::QuickNote);
                }
                if icons::button(ui, Icon::Select, self.selecting, "Select").clicked() {
                    self.selecting = !self.selecting;
                    self.selected.clear();
                }
                let s = icons::button(ui, Icon::Sort, false, "Sort");
                egui::Popup::menu(&s).show(|ui| {
                    for k in SortBy::ALL {
                        if ui.selectable_label(prefs.sort == k, k.name()).clicked() {
                            prefs.sort = k;
                            ui.close();
                        }
                    }
                });
                if icons::button(
                    ui,
                    if prefs.list_view {
                        Icon::Grid
                    } else {
                        Icon::List
                    },
                    false,
                    "Change view",
                )
                .clicked()
                {
                    prefs.list_view = !prefs.list_view;
                }
                if icons::button(ui, Icon::Search, self.searching, "Search").clicked() {
                    self.searching = !self.searching;
                    if !self.searching {
                        self.search.clear();
                    }
                }
            });
        });
        if self.searching {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search titles, tags and typed text")
                    .desired_width(f32::INFINITY),
            );
            if !r.has_focus() && self.search.is_empty() && ui.input(|i| i.key_pressed(Key::Escape))
            {
                self.searching = false;
            }
            if self.search.is_empty() {
                r.request_focus();
            }
        }
        if self.filter == Filter::Trashed && lib.count(&Filter::Trashed) > 0 {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Notebooks in the trash are deleted after 30 days.")
                        .color(pal.muted),
                );
                if ui
                    .button(RichText::new("Empty trash").color(pal.danger))
                    .clicked()
                {
                    self.dialog = Some(Dialog::ConfirmEmptyTrash);
                }
            });
        }
        ui.add_space(8.0);

        let items: Vec<NotebookMeta> = lib
            .query(&self.filter, &self.search, prefs.sort)
            .into_iter()
            .cloned()
            .collect();
        if items.is_empty() {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                let msg = if !self.search.is_empty() {
                    "Nothing matches your search."
                } else if self.filter == Filter::Trashed {
                    "Trash is empty."
                } else {
                    "No notebooks here yet. Tap + to start one."
                };
                ui.label(RichText::new(msg).color(pal.muted).size(17.0));
            });
        }
        let bottom_pad = if self.selecting { 80.0 } else { 90.0 };
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if prefs.list_view {
                    for m in &items {
                        self.list_row(ui, lib, m, thumbs, store, pal, actions);
                    }
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(26.0, 22.0);
                        for m in &items {
                            self.card(ui, lib, m, thumbs, store, pal, actions);
                        }
                    });
                }
                ui.add_space(bottom_pad);
            });
        if self.selecting {
            self.selection_bar(ui.ctx(), lib, pal, actions);
        }
    }

    fn thumb(
        &self,
        ui: &Ui,
        m: &NotebookMeta,
        thumbs: &mut HashMap<Id, (u64, egui::TextureHandle)>,
        store: &notebook_core::storage::Store,
    ) -> Option<egui::TextureHandle> {
        if let Some((v, t)) = thumbs.get(&m.id)
            && *v == m.updated
        {
            return Some(t.clone());
        }
        let img = store.load_thumb(m.id)?;
        let tex = textures::to_texture(ui.ctx(), "nb-thumb", img);
        thumbs.insert(m.id, (m.updated, tex.clone()));
        Some(tex)
    }

    #[allow(clippy::too_many_arguments)]
    fn card(
        &mut self,
        ui: &mut Ui,
        lib: &mut Library,
        m: &NotebookMeta,
        thumbs: &mut HashMap<Id, (u64, egui::TextureHandle)>,
        store: &notebook_core::storage::Store,
        pal: &Palette,
        actions: &mut Vec<LibAction>,
    ) {
        let (w, h) = (150.0, 212.0);
        let tag_h = if m.tags.is_empty() { 0.0 } else { 28.0 };
        let (r, resp) = ui.allocate_exact_size(vec2(w, h + 40.0 + tag_h), Sense::click());
        if !ui.is_rect_visible(r) {
            return;
        }
        let img = Rect::from_min_size(r.min, vec2(w, h));
        let p = ui.painter();
        p.rect_filled(
            img.translate(vec2(0.0, 3.0)),
            8.0,
            Color32::from_black_alpha(90),
        );
        match self.thumb(ui, m, thumbs, store) {
            Some(t) => {
                let tex_size = t.size_vec2();
                let ih = (w * tex_size.y / tex_size.x).min(h);
                let ir = Rect::from_min_size(pos2(img.left(), img.bottom() - ih), vec2(w, ih));
                p.rect_filled(ir, 6.0, Color32::WHITE);
                p.image(
                    t.id(),
                    ir,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                p.rect_stroke(ir, 6.0, Stroke::new(3.0, c32(m.cover)), StrokeKind::Inside);
            }
            None => {
                p.rect_filled(img, 8.0, c32(m.cover));
            }
        }
        // Time-ago badge, like the reference app.
        let age = time_ago(m.updated);
        let br = Rect::from_min_size(
            img.left_bottom() + vec2(6.0, -26.0),
            vec2(8.0 + age.len() as f32 * 9.0, 20.0),
        );
        p.rect_filled(br, 6.0, Color32::from_black_alpha(110));
        p.text(
            br.center(),
            Align2::CENTER_CENTER,
            age,
            FontId::proportional(13.0),
            Color32::WHITE,
        );
        if m.pinned {
            let pr = Rect::from_center_size(img.right_top() + vec2(-14.0, 14.0), vec2(22.0, 22.0));
            p.circle_filled(pr.center(), 13.0, Color32::from_black_alpha(130));
            icons::paint(p, pr.shrink(3.0), Icon::Pin, Color32::WHITE);
        }
        let mut job =
            egui::text::LayoutJob::simple(m.title.clone(), FontId::proportional(16.0), pal.text, w);
        job.halign = Align::Center;
        let title_galley = ui.fonts_mut(|f| f.layout_job(job));
        p.galley(
            pos2(r.center().x, img.bottom() + 8.0),
            title_galley,
            pal.text,
        );
        if let Some(tag) = m.tags.first() {
            let label = if m.tags.len() > 1 {
                format!("{tag} +{}", m.tags.len() - 1)
            } else {
                tag.clone()
            };
            let g = ui.fonts_mut(|f| f.layout_no_wrap(label, FontId::proportional(13.0), pal.text));
            let tw = g.size().x.min(w) + 16.0;
            let tr = Rect::from_center_size(pos2(r.center().x, r.bottom() - 14.0), vec2(tw, 24.0));
            p.rect_filled(tr, 12.0, pal.hover);
            p.galley(tr.center() - g.size() / 2.0, g, pal.text);
        }
        if self.selecting {
            let on = self.selected.contains(&m.id);
            let cr = img.right_bottom() - vec2(16.0, 16.0);
            p.circle(
                cr,
                12.0,
                if on {
                    pal.accent
                } else {
                    Color32::from_black_alpha(120)
                },
                Stroke::new(2.0, Color32::WHITE),
            );
            if on {
                icons::paint(
                    p,
                    Rect::from_center_size(cr, vec2(16.0, 16.0)),
                    Icon::Check,
                    Color32::WHITE,
                );
            }
        }
        self.card_interaction(&resp, lib, m, actions);
    }

    #[allow(clippy::too_many_arguments)]
    fn list_row(
        &mut self,
        ui: &mut Ui,
        lib: &mut Library,
        m: &NotebookMeta,
        thumbs: &mut HashMap<Id, (u64, egui::TextureHandle)>,
        store: &notebook_core::storage::Store,
        pal: &Palette,
        actions: &mut Vec<LibAction>,
    ) {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::click());
        if !ui.is_rect_visible(r) {
            return;
        }
        if resp.hovered() || self.selected.contains(&m.id) {
            ui.painter().rect_filled(r, 10.0, pal.hover);
        }
        let ir = Rect::from_min_size(r.min + vec2(8.0, 6.0), vec2(37.0, 52.0));
        match self.thumb(ui, m, thumbs, store) {
            Some(t) => {
                ui.painter().image(
                    t.id(),
                    ir,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                ui.painter().rect_filled(ir, 4.0, c32(m.cover));
            }
        }
        let p = ui.painter();
        p.text(
            pos2(ir.right() + 14.0, r.top() + 22.0),
            Align2::LEFT_CENTER,
            &m.title,
            FontId::proportional(16.5),
            pal.text,
        );
        let mut sub = format!(
            "{} · {} page{}",
            time_ago(m.updated),
            m.page_count,
            if m.page_count == 1 { "" } else { "s" }
        );
        if !m.tags.is_empty() {
            sub.push_str(&format!(" · #{}", m.tags.join("  #")));
        }
        p.text(
            pos2(ir.right() + 14.0, r.top() + 44.0),
            Align2::LEFT_CENTER,
            sub,
            FontId::proportional(13.0),
            pal.muted,
        );
        if m.pinned {
            icons::paint(
                p,
                Rect::from_center_size(pos2(r.right() - 24.0, r.center().y), vec2(20.0, 20.0)),
                Icon::Pin,
                pal.muted,
            );
        }
        self.card_interaction(&resp, lib, m, actions);
    }

    fn card_interaction(
        &mut self,
        resp: &egui::Response,
        lib: &mut Library,
        m: &NotebookMeta,
        actions: &mut Vec<LibAction>,
    ) {
        if resp.clicked() {
            if self.selecting {
                if !self.selected.remove(&m.id) {
                    self.selected.insert(m.id);
                }
            } else if m.trashed() {
                // Trashed notebooks open only after restoring.
            } else {
                actions.push(LibAction::Open(m.id));
            }
        }
        let id = m.id;
        resp.context_menu(|ui| {
            ui.set_min_width(220.0);
            if m.trashed() {
                if ui.button("Restore").clicked() {
                    if let Some(x) = lib.get_mut(id) {
                        x.trashed_at = None;
                    }
                    actions.push(LibAction::Changed);
                    ui.close();
                }
                if ui.button("Delete forever").clicked() {
                    self.dialog = Some(Dialog::ConfirmDelete(vec![id]));
                    ui.close();
                }
                return;
            }
            if ui.button("Open").clicked() {
                actions.push(LibAction::Open(id));
                ui.close();
            }
            if ui.button("Rename").clicked() {
                self.dialog = Some(Dialog::Rename(id, m.title.clone()));
                ui.close();
            }
            if ui.button(if m.pinned { "Unpin" } else { "Pin" }).clicked() {
                if let Some(x) = lib.get_mut(id) {
                    x.pinned = !x.pinned;
                }
                actions.push(LibAction::Changed);
                ui.close();
            }
            if ui.button("Tags…").clicked() {
                self.dialog = Some(Dialog::Tags(vec![id], String::new()));
                ui.close();
            }
            if ui.button("Move to folder…").clicked() {
                self.dialog = Some(Dialog::MoveTo(vec![id]));
                ui.close();
            }
            ui.menu_button("Cover colour", |ui| {
                ui.horizontal(|ui| {
                    for hex in COVERS {
                        let c = Rgba::from_hex(hex).unwrap_or(Rgba::BLACK);
                        let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                        ui.painter().circle_filled(r.center(), 11.0, c32(c));
                        if resp.clicked() {
                            if let Some(x) = lib.get_mut(id) {
                                x.cover = c;
                            }
                            actions.push(LibAction::Changed);
                            ui.close();
                        }
                    }
                });
            });
            if ui.button("Duplicate").clicked() {
                actions.push(LibAction::Duplicate(id));
                ui.close();
            }
            if ui.button("Export PDF").clicked() {
                actions.push(LibAction::ExportPdf(id));
                ui.close();
            }
            ui.separator();
            if ui.button("Move to trash").clicked() {
                if let Some(x) = lib.get_mut(id) {
                    x.trashed_at = Some(now_ms());
                    x.pinned = false;
                }
                actions.push(LibAction::Changed);
                ui.close();
            }
        });
    }

    fn selection_bar(
        &mut self,
        ctx: &egui::Context,
        lib: &mut Library,
        pal: &Palette,
        actions: &mut Vec<LibAction>,
    ) {
        let screen = ctx.content_rect();
        egui::Area::new(EId::unique("lib-select-bar"))
            .order(Order::Foreground)
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(pos2(screen.center().x + 130.0, screen.bottom() - 20.0))
            .show(ctx, |ui| {
                Frame::popup(ui.style())
                    .corner_radius(18)
                    .inner_margin(10)
                    .fill(pal.raised)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let ids: Vec<Id> = self.selected.iter().copied().collect();
                            ui.label(format!("{} selected", ids.len()));
                            let any = !ids.is_empty();
                            if self.filter == Filter::Trashed {
                                if ui.add_enabled(any, egui::Button::new("Restore")).clicked() {
                                    for id in &ids {
                                        if let Some(x) = lib.get_mut(*id) {
                                            x.trashed_at = None;
                                        }
                                    }
                                    self.selected.clear();
                                    actions.push(LibAction::Changed);
                                }
                                if ui
                                    .add_enabled(
                                        any,
                                        egui::Button::new(
                                            RichText::new("Delete forever").color(pal.danger),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.dialog = Some(Dialog::ConfirmDelete(ids));
                                }
                            } else {
                                if ui.add_enabled(any, egui::Button::new("Move")).clicked() {
                                    self.dialog = Some(Dialog::MoveTo(ids.clone()));
                                }
                                if ui.add_enabled(any, egui::Button::new("Tag")).clicked() {
                                    self.dialog = Some(Dialog::Tags(ids.clone(), String::new()));
                                }
                                if ui.add_enabled(any, egui::Button::new("Pin")).clicked() {
                                    let all_pinned =
                                        ids.iter().all(|i| lib.get(*i).is_some_and(|m| m.pinned));
                                    for id in &ids {
                                        if let Some(x) = lib.get_mut(*id) {
                                            x.pinned = !all_pinned;
                                        }
                                    }
                                    actions.push(LibAction::Changed);
                                }
                                if ui
                                    .add_enabled(
                                        any,
                                        egui::Button::new(RichText::new("Trash").color(pal.danger)),
                                    )
                                    .clicked()
                                {
                                    for id in &ids {
                                        if let Some(x) = lib.get_mut(*id) {
                                            x.trashed_at = Some(now_ms());
                                            x.pinned = false;
                                        }
                                    }
                                    self.selected.clear();
                                    actions.push(LibAction::Changed);
                                }
                            }
                            if ui.button("Done").clicked() {
                                self.selecting = false;
                                self.selected.clear();
                            }
                        });
                    });
            });
    }

    fn fab(
        &mut self,
        ctx: &egui::Context,
        pal: &Palette,
        prefs: &Prefs,
        actions: &mut Vec<LibAction>,
    ) {
        if self.selecting || self.filter == Filter::Trashed {
            return;
        }
        let screen = ctx.content_rect();
        egui::Area::new(EId::unique("lib-fab"))
            .order(Order::Foreground)
            .pivot(Align2::RIGHT_BOTTOM)
            .fixed_pos(screen.right_bottom() - vec2(28.0, 28.0))
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(58.0, 58.0), Sense::click());
                ui.painter().circle_filled(
                    r.center() + vec2(0.0, 2.0),
                    29.0,
                    Color32::from_black_alpha(80),
                );
                ui.painter().circle_filled(r.center(), 28.0, pal.raised);
                icons::paint(ui.painter(), r.shrink(15.0), Icon::Plus, pal.accent);
                egui::Popup::menu(&resp).show(|ui| {
                    ui.set_min_width(200.0);
                    if ui.button("New notebook").clicked() {
                        let folder = if let Filter::Folder(f) = self.filter {
                            Some(f)
                        } else {
                            None
                        };
                        let tags = if let Filter::Tag(t) = &self.filter {
                            t.clone()
                        } else {
                            String::new()
                        };
                        self.dialog = Some(Dialog::New {
                            title: String::new(),
                            cover: Rgba::from_hex(COVERS[0]).unwrap_or(Rgba::BLACK),
                            template: prefs.new_template,
                            paper: prefs.new_paper,
                            folder,
                            tags,
                        });
                        ui.close();
                    }
                    if ui.button("Quick note").clicked() {
                        actions.push(LibAction::QuickNote);
                        ui.close();
                    }
                    if ui.button("New folder").clicked() {
                        self.dialog = Some(Dialog::NewFolder(String::new()));
                        ui.close();
                    }
                });
            });
    }

    fn dialogs(
        &mut self,
        ctx: &egui::Context,
        lib: &mut Library,
        prefs: &mut Prefs,
        pal: &Palette,
        actions: &mut Vec<LibAction>,
    ) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let mut keep = true;
        let modal = egui::Modal::new(EId::unique("lib-dialog")).show(ctx, |ui| {
            ui.set_width(420.0);
            let enter = ui.input(|i| i.key_pressed(Key::Enter));
            match &mut dialog {
                Dialog::New {
                    title,
                    cover,
                    template,
                    paper,
                    folder,
                    tags,
                } => {
                    ui.heading("New notebook");
                    ui.add_space(6.0);
                    ui.add(
                        egui::TextEdit::singleline(title)
                            .hint_text("Title")
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(6.0);
                    ui.label(RichText::new("Cover").color(pal.muted));
                    ui.horizontal(|ui| {
                        for hex in COVERS {
                            let c = Rgba::from_hex(hex).unwrap_or(Rgba::BLACK);
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
                            ui.painter().circle_filled(r.center(), 12.0, c32(c));
                            if *cover == c {
                                ui.painter().circle_stroke(
                                    r.center(),
                                    15.0,
                                    Stroke::new(2.5, pal.accent),
                                );
                            }
                            if resp.clicked() {
                                *cover = c;
                            }
                        }
                    });
                    ui.label(RichText::new("Paper template").color(pal.muted));
                    textures::template_picker(ui, template, *paper, pal);
                    ui.horizontal(|ui| {
                        for p in Paper::ALL {
                            ui.selectable_value(paper, p, p.name());
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Folder");
                        let name = folder
                            .and_then(|f| lib.folders.iter().find(|x| x.id == f))
                            .map(|f| f.name.clone())
                            .unwrap_or_else(|| "None".into());
                        egui::ComboBox::from_id_salt("nb-folder")
                            .selected_text(name)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(folder, None, "None");
                                for f in &lib.folders {
                                    ui.selectable_value(folder, Some(f.id), &f.name);
                                }
                            });
                    });
                    ui.add(
                        egui::TextEdit::singleline(tags)
                            .hint_text("Tags, comma separated (e.g. School/Physics)")
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if ui.button(RichText::new("Create").strong()).clicked() || enter {
                            prefs.new_template = *template;
                            prefs.new_paper = *paper;
                            let t = if title.trim().is_empty() {
                                "Untitled".to_string()
                            } else {
                                title.trim().to_string()
                            };
                            let tags: Vec<String> =
                                tags.split(',').filter_map(normalize_tag).collect();
                            actions.push(LibAction::Create {
                                title: t,
                                cover: *cover,
                                template: *template,
                                paper: *paper,
                                folder: *folder,
                                tags,
                            });
                            keep = false;
                        }
                    });
                }
                Dialog::Rename(id, name) => {
                    ui.heading("Rename");
                    ui.text_edit_singleline(name).request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if ui.button("Save").clicked() || enter {
                            if let Some(m) = lib.get_mut(*id)
                                && !name.trim().is_empty()
                            {
                                m.title = name.trim().to_string();
                                m.updated = now_ms();
                            }
                            actions.push(LibAction::Changed);
                            keep = false;
                        }
                    });
                }
                Dialog::Tags(ids, input) => {
                    ui.heading("Tags");
                    let current: Vec<String> = {
                        let mut v: Vec<String> = ids
                            .iter()
                            .filter_map(|i| lib.get(*i))
                            .flat_map(|m| m.tags.clone())
                            .collect();
                        v.sort();
                        v.dedup();
                        v
                    };
                    ui.horizontal_wrapped(|ui| {
                        for t in &current {
                            if ui
                                .button(format!("#{t}  ✕"))
                                .on_hover_text("Remove")
                                .clicked()
                            {
                                for id in ids.iter() {
                                    if let Some(m) = lib.get_mut(*id) {
                                        m.tags.retain(|x| x != t);
                                    }
                                }
                                actions.push(LibAction::Changed);
                            }
                        }
                    });
                    let r = ui.add(
                        egui::TextEdit::singleline(input)
                            .hint_text("Add tag, e.g. Sample/Drawing")
                            .desired_width(f32::INFINITY),
                    );
                    r.request_focus();
                    let all = lib.all_tags();
                    let suggestions: Vec<String> = all
                        .into_iter()
                        .filter(|t| {
                            !current.contains(t)
                                && (input.is_empty()
                                    || t.to_lowercase().contains(&input.to_lowercase()))
                        })
                        .take(8)
                        .collect();
                    let mut add: Option<String> = None;
                    ui.horizontal_wrapped(|ui| {
                        for s in suggestions {
                            if ui.small_button(format!("#{s}")).clicked() {
                                add = Some(s);
                            }
                        }
                    });
                    if enter {
                        add = normalize_tag(input);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Add").clicked() {
                            add = normalize_tag(input);
                        }
                        if ui.button("Done").clicked() {
                            keep = false;
                        }
                    });
                    if let Some(t) = add {
                        for id in ids.iter() {
                            if let Some(m) = lib.get_mut(*id)
                                && !m.tags.contains(&t)
                            {
                                m.tags.push(t.clone());
                                m.tags.sort();
                            }
                        }
                        input.clear();
                        actions.push(LibAction::Changed);
                    }
                }
                Dialog::MoveTo(ids) => {
                    ui.heading("Move to folder");
                    let mut target: Option<Option<Id>> = None;
                    if ui.button("No folder").clicked() {
                        target = Some(None);
                    }
                    for f in &lib.folders {
                        if ui.button(&f.name).clicked() {
                            target = Some(Some(f.id));
                        }
                    }
                    if ui.button("+ New folder…").clicked() {
                        let ids = ids.clone();
                        let id = new_id();
                        lib.folders.push(Folder {
                            id,
                            name: format!("Folder {}", lib.folders.len() + 1),
                        });
                        for nb in &ids {
                            if let Some(m) = lib.get_mut(*nb) {
                                m.folder = Some(id);
                            }
                        }
                        actions.push(LibAction::Changed);
                        self.selected.clear();
                        keep = false;
                        self.dialog = Some(Dialog::RenameFolder(
                            id,
                            format!("Folder {}", lib.folders.len()),
                        ));
                        return;
                    }
                    if let Some(t) = target {
                        for id in ids.iter() {
                            if let Some(m) = lib.get_mut(*id) {
                                m.folder = t;
                            }
                        }
                        actions.push(LibAction::Changed);
                        self.selected.clear();
                        keep = false;
                    }
                    if ui.button("Cancel").clicked() {
                        keep = false;
                    }
                }
                Dialog::NewFolder(name) => {
                    ui.heading("New folder");
                    ui.text_edit_singleline(name).request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if (ui.button("Create").clicked() || enter) && !name.trim().is_empty() {
                            lib.folders.push(Folder {
                                id: new_id(),
                                name: name.trim().to_string(),
                            });
                            actions.push(LibAction::Changed);
                            keep = false;
                        }
                    });
                }
                Dialog::RenameFolder(id, name) => {
                    ui.heading("Rename folder");
                    ui.text_edit_singleline(name).request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if (ui.button("Save").clicked() || enter) && !name.trim().is_empty() {
                            if let Some(f) = lib.folders.iter_mut().find(|f| f.id == *id) {
                                f.name = name.trim().to_string();
                            }
                            actions.push(LibAction::Changed);
                            keep = false;
                        }
                    });
                }
                Dialog::RenameTag(old, new) => {
                    ui.heading("Rename tag");
                    ui.label(RichText::new("Use / to nest, e.g. School/Physics").color(pal.muted));
                    ui.text_edit_singleline(new).request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if ui.button("Save").clicked() || enter {
                            lib.rename_tag(old, new);
                            if let Some(n) = normalize_tag(new) {
                                self.filter = Filter::Tag(n);
                            }
                            actions.push(LibAction::Changed);
                            keep = false;
                        }
                    });
                }
                Dialog::ConfirmDelete(ids) => {
                    ui.heading("Delete forever?");
                    ui.label(format!(
                        "{} notebook{} will be permanently deleted. This can't be undone.",
                        ids.len(),
                        if ids.len() == 1 { "" } else { "s" }
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if ui
                            .button(RichText::new("Delete").color(pal.danger))
                            .clicked()
                        {
                            actions.push(LibAction::DeleteForever(ids.clone()));
                            self.selected.clear();
                            keep = false;
                        }
                    });
                }
                Dialog::ConfirmEmptyTrash => {
                    ui.heading("Empty trash?");
                    ui.label("Everything in the trash will be permanently deleted.");
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if ui
                            .button(RichText::new("Empty trash").color(pal.danger))
                            .clicked()
                        {
                            actions.push(LibAction::EmptyTrash);
                            keep = false;
                        }
                    });
                }
            }
        });
        if modal.should_close() {
            keep = false;
        }
        if keep && self.dialog.is_none() {
            self.dialog = Some(dialog);
        }
    }
}

pub fn time_ago(ts: u64) -> String {
    let s = now_ms().saturating_sub(ts) / 1000;
    match s {
        0..60 => "now".into(),
        60..3600 => format!("{}m", s / 60),
        3600..86_400 => format!("{}h", s / 3600),
        86_400..2_592_000 => format!("{}d", s / 86_400),
        _ => format!("{}w", s / 604_800),
    }
}
