//! App shell: owns the library, settings and the open notebook, and routes between screens.

use crate::editor::{Editor, EditorEvent};
use crate::library_view::{LibAction, LibraryView};
use crate::prefs::Prefs;
use crate::saver::{Job, Saver};
use crate::theme;
use egui::{Align2, Color32, FontId, Id as EId, Order, RichText, vec2};
use notebook_core::library::{Library, NotebookMeta};
use notebook_core::storage::Store;
use notebook_core::*;
use std::collections::HashMap;
use std::sync::Arc;

pub struct NotebookApp {
    pub(crate) store: Store,
    saver: Saver,
    pub(crate) lib: Library,
    pub(crate) prefs: Prefs,
    saved_prefs: String,
    pub(crate) editor: Option<Editor>,
    lib_view: LibraryView,
    thumbs: HashMap<Id, (u64, egui::TextureHandle)>,
    clipboard: Vec<Item>,
    toast: Option<(String, f64)>,
    applied_dark: Option<bool>,
    #[cfg(feature = "autotest")]
    pub(crate) autotest: Option<crate::autotest::AutoTest>,
}

impl NotebookApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let store = Store::open_default()
            .or_else(|_| {
                Store::new(
                    std::env::temp_dir().join("notebook-v1/data"),
                    std::env::temp_dir().join("notebook-v1/exports"),
                )
            })
            .expect("storage");
        let saver = Saver::start(store.clone());
        let mut lib = store.load_library();
        let prefs = Prefs::load(&store);

        // Trash older than 30 days is removed for good.
        for id in lib.expired_trash(30) {
            let _ = store.delete_doc(id);
            store.delete_thumb(id);
            lib.notebooks.retain(|m| m.id != id);
        }
        let mut app = NotebookApp {
            saved_prefs: serde_json::to_string(&prefs).unwrap_or_default(),
            store,
            saver,
            lib,
            prefs,
            editor: None,
            lib_view: LibraryView::default(),
            thumbs: HashMap::new(),
            clipboard: Vec::new(),
            toast: None,
            applied_dark: None,
            #[cfg(feature = "autotest")]
            autotest: crate::autotest::AutoTest::from_env(),
        };
        if app.lib.notebooks.is_empty() && app.store.read_file("welcomed").is_none() {
            app.create_welcome();
            let _ = app.store.write_file("welcomed", b"1");
        }
        if app.prefs.auto_backup && !app.lib.notebooks.is_empty() {
            let day = 86_400_000;
            if app
                .store
                .last_backup()
                .is_none_or(|t| now_ms().saturating_sub(t) > day)
            {
                app.saver.send(Job::Backup(false));
            }
        }
        app.save_library();
        app
    }

    fn save_library(&self) {
        self.saver.send(Job::Library(self.lib.clone()));
    }

    fn create(
        &mut self,
        title: &str,
        cover: Rgba,
        doc: Document,
        folder: Option<Id>,
        tags: Vec<String>,
    ) -> Id {
        let first = &doc.pages[0];
        let mut meta = NotebookMeta::new(doc.id, title, cover, first.template, first.paper);
        meta.folder = folder;
        meta.tags = tags;
        meta.page_count = doc.pages.len();
        meta.search_text = doc.search_text();
        // Write synchronously so the notebook exists before it is opened.
        let _ = self.store.save_doc(&doc);
        let _ = self
            .store
            .save_thumb(doc.id, &render::render_png(&doc.pages[0], 320));
        let id = meta.id;
        self.lib.notebooks.push(meta);
        self.save_library();
        id
    }

    #[cfg(feature = "autotest")]
    pub(crate) fn open_new_dialog(&mut self) {
        self.lib_view.start_new(&self.prefs);
    }

    pub(crate) fn open(&mut self, id: Id) {
        self.close_editor();
        match self.store.load_doc(id) {
            Ok(doc) => {
                let title = self
                    .lib
                    .get(id)
                    .map(|m| m.title.clone())
                    .unwrap_or_default();
                if let Some(m) = self.lib.get_mut(id) {
                    m.opened = now_ms();
                }
                self.save_library();
                self.editor = Some(Editor::new(id, title, doc));
            }
            Err(e) => self.show_toast(format!("Could not open notebook: {e}")),
        }
    }

    fn flush_editor(&mut self) {
        let Some(ed) = &mut self.editor else { return };
        if !ed.dirty {
            return;
        }
        ed.dirty = false;
        self.saver.send(Job::Doc(ed.doc.clone()));
        if let Some(m) = self.lib.get_mut(ed.meta_id) {
            m.title = ed.title.clone();
            m.updated = now_ms();
            m.page_count = ed.doc.pages.len();
            m.search_text = ed.search_text();
            m.template = ed.first_page().template;
            m.paper = ed.first_page().paper;
        }
        self.save_library();
    }

    pub(crate) fn close_editor(&mut self) {
        let was_dirty = self.editor.as_ref().is_some_and(|e| e.dirty);
        if let Some(ed) = &mut self.editor {
            ed.finish_text();
        }
        self.flush_editor();
        if let Some(ed) = self.editor.take()
            && (was_dirty || ed.history.can_undo())
        {
            self.saver
                .send(Job::Thumb(ed.meta_id, ed.first_page().clone()));
        }
    }

    fn show_toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), now_ms() as f64 / 1000.0));
    }

    fn export_pdf(&mut self, id: Id) {
        let doc = match (&self.editor, self.store.load_doc(id)) {
            (Some(ed), _) if ed.meta_id == id => ed.doc.clone(),
            (_, Ok(d)) => d,
            (_, Err(e)) => return self.show_toast(format!("Export failed: {e}")),
        };
        let title = self
            .lib
            .get(id)
            .map(|m| m.title.clone())
            .unwrap_or_else(|| "Notebook".into());
        let msg = self.export_file(
            &format!("{title}.pdf"),
            &pdf::export(&doc, &title),
            "application/pdf",
        );
        self.show_toast(msg);
    }

    /// Saves an exported file: into Files › Notebook on iPad, as a download on the web.
    fn export_file(&self, name: &str, bytes: &[u8], mime: &str) -> String {
        #[cfg(target_arch = "wasm32")]
        {
            match crate::web::download(name, bytes, mime) {
                Ok(()) => format!("Downloaded {name}"),
                Err(e) => format!("Export failed: {e}"),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = mime;
            match self.store.export(name, bytes) {
                Ok(_) => "Saved to Files › Notebook › Exports".into(),
                Err(e) => format!("Export failed: {e}"),
            }
        }
    }

    fn handle_lib_actions(&mut self, actions: Vec<LibAction>) {
        for a in actions {
            match a {
                LibAction::Open(id) => self.open(id),
                LibAction::Create {
                    title,
                    cover,
                    template,
                    paper,
                    folder,
                    tags,
                } => {
                    let doc = Document::new(new_id(), template, paper);
                    let id = self.create(&title, cover, doc, folder, tags);
                    self.open(id);
                }
                LibAction::QuickNote => {
                    let title = format!("Notes on {}", today());
                    let doc =
                        Document::new(new_id(), self.prefs.new_template, self.prefs.new_paper);
                    let cover = Rgba::from_hex("#0a84ff").unwrap_or(Rgba::BLACK);
                    let id = self.create(&title, cover, doc, None, Vec::new());
                    self.open(id);
                }
                LibAction::Duplicate(id) => {
                    if let (Ok(doc), Some(meta)) =
                        (self.store.load_doc(id), self.lib.get(id).cloned())
                    {
                        let mut copy = doc.clone();
                        copy.id = new_id();
                        for p in &mut copy.pages {
                            p.id = new_id();
                        }
                        self.create(
                            &format!("{} copy", meta.title),
                            meta.cover,
                            copy,
                            meta.folder,
                            meta.tags.clone(),
                        );
                    }
                }
                LibAction::DeleteForever(ids) => {
                    for id in ids {
                        let _ = self.store.delete_doc(id);
                        self.store.delete_thumb(id);
                        self.lib.notebooks.retain(|m| m.id != id);
                        self.thumbs.remove(&id);
                    }
                    self.save_library();
                }
                LibAction::EmptyTrash => {
                    let ids: Vec<Id> = self
                        .lib
                        .notebooks
                        .iter()
                        .filter(|m| m.trashed())
                        .map(|m| m.id)
                        .collect();
                    self.handle_lib_actions(vec![LibAction::DeleteForever(ids)]);
                }
                LibAction::ExportPdf(id) => self.export_pdf(id),
                LibAction::BackupNow => self.saver.send(Job::Backup(true)),
                LibAction::Changed => self.save_library(),
            }
        }
    }

    fn create_welcome(&mut self) {
        let doc = welcome_doc();
        let cover = Rgba::from_hex("#5b5bd6").unwrap_or(Rgba::BLACK);
        self.create(
            "Welcome to Notebook",
            cover,
            doc,
            None,
            vec!["Sample".into()],
        );
    }

    fn toast_ui(&mut self, ctx: &egui::Context, pal: &theme::Palette) {
        let Some((msg, t)) = &self.toast else { return };
        let age = now_ms() as f64 / 1000.0 - t;
        if age > 3.0 {
            self.toast = None;
            return;
        }
        let screen = ctx.content_rect();
        let alpha = ((3.0 - age) * 3.0).clamp(0.0, 1.0) as f32;
        egui::Area::new(EId::unique("toast"))
            .order(Order::Tooltip)
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(screen.center_bottom() - vec2(0.0, 36.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_black_alpha((210.0 * alpha) as u8))
                    .corner_radius(20)
                    .inner_margin(egui::Margin::symmetric(18, 10))
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(msg.as_str())
                                    .color(Color32::WHITE.gamma_multiply(alpha))
                                    .font(FontId::proportional(15.0)),
                            )
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    });
            });
        let _ = pal;
        ctx.request_repaint();
    }
}

fn today() -> String {
    // Days since 1970 → civil date (no time zone; good enough for a title).
    let days = (now_ms() / 86_400_000) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:02}/{:02}/{:02}", d, m, y % 100)
}

fn welcome_doc() -> Document {
    let mut doc = Document::new(new_id(), Template::Lined, Paper::Cream);
    let page = &mut doc.pages[0];
    let ink = Rgba::from_hex("#1d1d1f").unwrap_or(Rgba::BLACK);
    let blue = Rgba::from_hex("#1e5bd8").unwrap_or(Rgba::BLACK);
    let text = |y: f32, s: &str, size: f32, color: Rgba| {
        Item::new(Content::Text(TextBox {
            x: 70.0,
            y,
            w: 470.0,
            text: s.into(),
            size,
            color,
        }))
    };
    let mut items = vec![
        text(40.0, "Welcome to Notebook", 28.0, ink),
        text(
            92.0,
            "Write with Apple Pencil. Your fingers scroll, pinch to zoom, tap with two fingers to undo and three to redo.",
            14.0,
            ink,
        ),
        text(
            160.0,
            "Pens: fineliner, fountain (thick and thin like a real nib) and brush. Tap the active tool again for sizes, colours, smoothing and pressure.",
            14.0,
            ink,
        ),
        text(
            240.0,
            "Draw a circle, box, triangle or line and keep the Pencil still for a moment: it snaps into a clean shape.",
            14.0,
            ink,
        ),
        text(
            310.0,
            "Lasso to move, resize, recolour, copy or delete. Ruler for straight lines. Laser pointer for presenting.",
            14.0,
            ink,
        ),
        text(
            380.0,
            "Organise with folders and nested #tags (like School/Physics). Search finds typed text. Export any notebook as a sharp vector PDF.",
            14.0,
            ink,
        ),
    ];
    // A highlighted word and a hand-drawn underline.
    let hl: Vec<InkPoint> = (0..=20)
        .map(|i| InkPoint {
            x: 68.0 + i as f32 * 14.0,
            y: 58.0,
            p: 0.5,
        })
        .collect();
    items.push(Item::new(Content::Stroke(Stroke {
        pen: PenKind::Highlighter,
        color: Rgba::rgb(255, 214, 10),
        width: mm(6.0),
        points: hl,
    })));
    let wave: Vec<InkPoint> = (0..=80)
        .map(|i| {
            let t = i as f32 / 80.0;
            InkPoint {
                x: 70.0 + t * 300.0,
                y: 470.0 + (t * 14.0).sin() * 6.0,
                p: 0.3 + 0.6 * (t * std::f32::consts::PI).sin(),
            }
        })
        .collect();
    items.push(Item::new(Content::Stroke(Stroke {
        pen: PenKind::Fountain,
        color: blue,
        width: mm(1.0),
        points: wave,
    })));
    items.push(Item::new(Content::Shape(Shape {
        kind: ShapeKind::Ellipse,
        points: vec![[400.0, 440.0], [530.0, 510.0]],
        color: Rgba::rgb(217, 48, 37),
        width: 2.0,
        fill: None,
        dashed: false,
    })));
    page.items = items.into_iter().map(Arc::new).collect();
    doc.pages.push(Page::new(Template::Grid, Paper::White));
    doc.pages.push(Page::new(Template::Dotted, Paper::Night));
    doc
}

impl eframe::App for NotebookApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.cumulative_frame_nr() < 2 {
            log::info!("Drawing frame {}", ctx.cumulative_frame_nr());
        }
        if self.applied_dark != Some(self.prefs.dark_ui) {
            theme::apply(&ctx, self.prefs.dark_ui);
            self.applied_dark = Some(self.prefs.dark_ui);
        }
        let pal = theme::palette(self.prefs.dark_ui);

        for msg in self.saver.poll() {
            if let Some(id) = msg
                .strip_prefix("thumb:")
                .and_then(|s| s.parse::<Id>().ok())
            {
                self.thumbs.remove(&id);
            } else {
                self.show_toast(msg);
            }
        }

        // Respect the iPad's safe area (status bar, rounded corners, home indicator).
        let full = ui.max_rect();
        ui.painter().rect_filled(
            full,
            0.0,
            if self.editor.is_some() {
                pal.sidebar
            } else {
                pal.bg
            },
        );
        let safe = ctx.content_rect().intersect(full);
        ui.scope_builder(egui::UiBuilder::new().max_rect(safe), |ui| {
            if let Some(ed) = &mut self.editor {
                ed.ui(ui, &mut self.prefs, &mut self.clipboard);
            } else {
                let actions = self.lib_view.ui(
                    ui,
                    &mut self.lib,
                    &mut self.prefs,
                    &mut self.thumbs,
                    &self.store,
                );
                self.handle_lib_actions(actions);
            }
        });

        // Editor events and autosave.
        let mut close = false;
        let mut export_pdf = None;
        let mut pending_png: Vec<(String, Vec<u8>)> = Vec::new();
        if let Some(ed) = &mut self.editor {
            for e in std::mem::take(&mut ed.events) {
                match e {
                    EditorEvent::Close => close = true,
                    EditorEvent::ExportPdf => export_pdf = Some(ed.meta_id),
                    EditorEvent::ExportPng(i) => {
                        let png = render::render_png(&ed.doc.pages[i], 1654);
                        let name = format!("{} - page {}.png", ed.title, i + 1);
                        pending_png.push((name, png));
                    }
                    EditorEvent::Toast(s) => self.toast = Some((s, now_ms() as f64 / 1000.0)),
                }
            }
            let quiet = matches!(ed.action, crate::editor::Action::Idle);
            if ed.dirty && quiet && ed.now - ed.last_edit > 0.6 {
                self.flush_editor();
            } else if ed.dirty {
                ctx.request_repaint_after(std::time::Duration::from_millis(700));
            }
        }
        for (name, png) in pending_png {
            let msg = self.export_file(&name, &png, "image/png");
            self.show_toast(msg);
        }
        if let Some(id) = export_pdf {
            self.flush_editor();
            self.export_pdf(id);
        }
        if close {
            self.close_editor();
        }

        // Settings are saved whenever they change.
        if let Ok(s) = serde_json::to_string(&self.prefs)
            && s != self.saved_prefs
        {
            self.prefs.save(&self.store);
            self.saved_prefs = s;
        }
        self.toast_ui(&ctx, &pal);
        #[cfg(feature = "autotest")]
        crate::autotest::step(self, &ctx);
    }

    #[cfg(feature = "autotest")]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(t) = &mut self.autotest {
            raw.events.append(&mut t.pending);
        }
    }

    #[cfg(any(feature = "autotest", target_arch = "wasm32"))]
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shutdown();
    }

    #[cfg(not(any(feature = "autotest", target_arch = "wasm32")))]
    fn on_exit(&mut self) {
        self.shutdown();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::palette(self.prefs.dark_ui)
            .bg
            .to_normalized_gamma_f32()
    }
}

impl NotebookApp {
    fn shutdown(&mut self) {
        self.close_editor();
        self.save_library();
        self.prefs.save(&self.store);
        // Give the saver a moment to finish.
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}
