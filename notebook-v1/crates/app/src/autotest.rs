//! Desktop test mode (`--features autotest`, env `NOTEBOOK_AUTOTEST=<dir>`).
//! Drives the real app with simulated Apple Pencil and finger touches and saves
//! screenshots, so the full input → ink → render path can be checked without an iPad.

use crate::app::NotebookApp;
use crate::editor::{Popover, Tool};
use egui::{Event, Pos2, TouchDeviceId, TouchId, TouchPhase, ViewportCommand, pos2};
use notebook_core::{Content, PenKind, Rgba, ShapeKind};
use std::path::PathBuf;

type Later = Box<dyn FnOnce(&mut NotebookApp, &mut AutoTest)>;

pub struct AutoTest {
    dir: PathBuf,
    step: usize,
    next_at: f64,
    touch_id: u64,
    pub pending: Vec<Event>,
    /// Work to run a few frames later (after earlier events were processed).
    after: Option<Later>,
    count: usize,
}

impl AutoTest {
    pub fn from_env() -> Option<Self> {
        let dir = PathBuf::from(std::env::var_os("NOTEBOOK_AUTOTEST")?);
        std::fs::create_dir_all(&dir).ok()?;
        Some(AutoTest {
            dir,
            step: 0,
            next_at: 1.5,
            touch_id: 100,
            pending: Vec::new(),
            after: None,
            count: 0,
        })
    }

    fn touch(&mut self, id: u64, phase: TouchPhase, pos: Pos2, force: Option<f32>) {
        self.pending.push(Event::Touch {
            device_id: TouchDeviceId(1),
            id: TouchId(id),
            phase,
            pos,
            force,
        });
    }

    /// A full Pencil stroke delivered in one frame.
    fn pencil(&mut self, pts: &[(Pos2, f32)], end: bool) {
        self.touch_id += 1;
        let id = self.touch_id;
        for (i, (p, f)) in pts.iter().enumerate() {
            let phase = if i == 0 {
                TouchPhase::Start
            } else {
                TouchPhase::Move
            };
            self.touch(id, phase, *p, Some(*f));
        }
        if end && let Some((p, f)) = pts.last() {
            self.touch(id, TouchPhase::End, *p, Some(*f));
        }
    }
}

fn shot(ctx: &egui::Context, dir: &std::path::Path, name: &str) {
    let path = dir.join(name);
    ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::ScreenshotCallback::new(
        move |img| {
            let file = std::fs::File::create(&path).expect("create screenshot");
            let mut enc = png::Encoder::new(
                std::io::BufWriter::new(file),
                img.size[0] as u32,
                img.size[1] as u32,
            );
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            enc.write_header()
                .and_then(|mut w| w.write_image_data(&bytes))
                .expect("write png");
            println!("screenshot {}", path.display());
        },
    )));
}

pub fn step(app: &mut NotebookApp, ctx: &egui::Context) {
    let Some(mut t) = app.autotest.take() else {
        return;
    };
    let now = ctx.input(|i| i.time);
    ctx.request_repaint();
    if now < t.next_at {
        if t.next_at - now < 0.3
            && let Some(f) = t.after.take()
        {
            f(app, &mut t);
        }
        app.autotest = Some(t);
        return;
    }
    let dir = t.dir.clone();
    let mut wait = 0.5;
    match t.step {
        0 => shot(ctx, &dir, "01-library.png"),
        1 => {
            if let Some(id) = app
                .lib
                .notebooks
                .iter()
                .find(|m| !m.trashed())
                .map(|m| m.id)
            {
                app.open(id);
            }
            wait = 1.0;
        }
        2 => {
            shot(ctx, &dir, "02-editor.png");
            let ed = app.editor.as_mut().expect("editor open");
            ed.scroll_to = Some(1);
        }
        3 => {
            let ed = app.editor.as_ref().expect("editor open");
            // Cursive-like loops with changing pressure (fountain pen).
            let pts: Vec<(Pos2, f32)> = (0..=240)
                .map(|i| {
                    let s = i as f32 / 240.0 * 9.0;
                    let x = 70.0 + s * 36.0 + (s * std::f32::consts::TAU).sin() * 12.0;
                    let y = 120.0 - (s * std::f32::consts::TAU).cos() * 14.0;
                    (
                        ed.page_to_screen(1, [x, y]),
                        0.12 + 0.3 * (s * 2.1).sin().abs(),
                    )
                })
                .collect();
            t.pencil(&pts, true);
            t.after = Some(Box::new(|app: &mut NotebookApp, t: &mut AutoTest| {
                app.prefs.pen_kind = PenKind::Brush;
                app.prefs.pen_color = Rgba::rgb(217, 48, 37);
                app.prefs.pen_mm = 1.9;
                let ed = app.editor.as_ref().expect("editor open");
                let brush: Vec<(Pos2, f32)> = (0..=120)
                    .map(|i| {
                        let s = i as f32 / 120.0;
                        (
                            ed.page_to_screen(
                                1,
                                [70.0 + s * 420.0, 190.0 + (s * 9.0).sin() * 10.0],
                            ),
                            0.45 * (s * std::f32::consts::PI).sin(),
                        )
                    })
                    .collect();
                t.pencil(&brush, true);
            }));
        }
        4 => shot(ctx, &dir, "03-ink.png"),
        5 => {
            app.prefs.pen_kind = PenKind::Fountain;
            app.prefs.pen_color = Rgba::rgb(30, 91, 216);
            app.prefs.pen_mm = 0.8;
            let ed = app.editor.as_ref().expect("editor open");
            // A wobbly circle; the Pencil then rests to trigger shape snapping.
            let pts: Vec<(Pos2, f32)> = (0..=90)
                .map(|i| {
                    let a = i as f32 / 90.0 * std::f32::consts::TAU * 1.02;
                    let r = 55.0 + (a * 5.0).sin() * 3.0;
                    (
                        ed.page_to_screen(1, [180.0 + r * a.cos(), 300.0 + r * 0.8 * a.sin()]),
                        0.3,
                    )
                })
                .collect();
            t.pencil(&pts, false);
            wait = 0.9;
        }
        6 => {
            let ed = app.editor.as_ref().expect("editor open");
            let p = ed.page_to_screen(1, [180.0 + 55.0 * 1.02f32.cos(), 300.0]);
            let id = t.touch_id;
            t.touch(id, TouchPhase::End, p, Some(0.3));
        }
        7 => {
            let ed = app.editor.as_ref().expect("editor open");
            let last = ed.doc.pages[1].items.last().map(|i| match &i.content {
                Content::Shape(s) => format!("shape {:?}", s.kind),
                Content::Stroke(_) => "stroke".into(),
                Content::Text(_) => "text".into(),
                Content::Tape(_) => "tape".into(),
                Content::Image(_) => "image".into(),
            });
            println!("after hold: last item = {last:?}");
            let snapped = ed.doc.pages[1].items.last().is_some_and(
                |i| matches!(&i.content, Content::Shape(s) if s.kind == ShapeKind::Ellipse),
            );
            println!("CHECK shape-snap {}", if snapped { "PASS" } else { "FAIL" });
            shot(ctx, &dir, "04-shape-snap.png");
        }
        8 => {
            let ed = app.editor.as_mut().expect("editor open");
            ed.set_tool(Tool::Highlighter);
            let hl: Vec<(Pos2, f32)> = (0..=40)
                .map(|i| {
                    (
                        ed.page_to_screen(1, [72.0 + i as f32 * 7.0, 190.0 + (i % 3) as f32 * 0.4]),
                        0.4,
                    )
                })
                .collect();
            t.pencil(&hl, true);
            t.after = Some(Box::new(|app: &mut NotebookApp, t: &mut AutoTest| {
                let ed = app.editor.as_mut().expect("editor open");
                ed.set_tool(Tool::Lasso);
                let lasso: Vec<(Pos2, f32)> = (0..=60)
                    .map(|i| {
                        let a = i as f32 / 60.0 * std::f32::consts::TAU;
                        (
                            ed.page_to_screen(1, [180.0 + 90.0 * a.cos(), 300.0 + 75.0 * a.sin()]),
                            0.3,
                        )
                    })
                    .collect();
                t.pencil(&lasso, true);
            }));
        }
        9 => {
            let ed = app.editor.as_ref().expect("editor open");
            println!(
                "CHECK lasso-select {}",
                if ed.selection.as_ref().is_some_and(|s| !s.ids.is_empty()) {
                    "PASS"
                } else {
                    "FAIL"
                }
            );
            shot(ctx, &dir, "05-lasso.png");
        }
        10 => {
            // Drag the selection with the Pencil.
            let ed = app.editor.as_ref().expect("editor open");
            let from = ed.page_to_screen(1, [180.0, 300.0]);
            let pts: Vec<(Pos2, f32)> = (0..=20)
                .map(|i| (from + egui::vec2(i as f32 * 8.0, -(i as f32) * 3.0), 0.3))
                .collect();
            t.pencil(&pts, true);
        }
        11 => {
            shot(ctx, &dir, "06-moved.png");
        }
        12 => {
            let ed = app.editor.as_mut().expect("editor open");
            ed.selection = None;
            ed.set_tool(Tool::Pen);
            ed.popover = Popover::ToolSettings(Tool::Pen);
            let r = ed.canvas;
            ed.popover_anchor = egui::Rect::from_min_size(
                pos2(r.left() - 48.0, r.top() + 60.0),
                egui::vec2(40.0, 40.0),
            );
        }
        13 => shot(ctx, &dir, "07-pen-panel.png"),
        14 => {
            let ed = app.editor.as_mut().expect("editor open");
            ed.popover = Popover::None;
            ed.sidebar = true;
            wait = 1.0;
        }
        15 => shot(ctx, &dir, "08-sidebar.png"),
        16 => {
            let ed = app.editor.as_mut().expect("editor open");
            ed.sidebar = false;
            ed.overview = true;
            wait = 1.0;
        }
        17 => shot(ctx, &dir, "09-overview.png"),
        18 => {
            let ed = app.editor.as_mut().expect("editor open");
            ed.overview = false;
            t.after = Some(Box::new(|app: &mut NotebookApp, t: &mut AutoTest| {
                let ed = app.editor.as_mut().expect("editor open");
                // Two-finger pinch out around a point on the page.
                let c = ed.page_to_screen(1, [300.0, 250.0]);
                let z0 = ed.zoom;
                for k in 0..=10 {
                    let d = 60.0 + k as f32 * 12.0;
                    let phase = if k == 0 {
                        TouchPhase::Start
                    } else {
                        TouchPhase::Move
                    };
                    t.touch(1, phase, c - egui::vec2(d, 0.0), None);
                    t.touch(2, phase, c + egui::vec2(d, 0.0), None);
                }
                t.touch(1, TouchPhase::End, c - egui::vec2(180.0, 0.0), None);
                t.touch(2, TouchPhase::End, c + egui::vec2(180.0, 0.0), None);
                println!("zoom before pinch {z0:.2}");
            }));
        }
        19 => {
            let ed = app.editor.as_ref().expect("editor open");
            println!("zoom after pinch {:.2}", ed.zoom);
            shot(ctx, &dir, "10-zoomed.png");
        }
        20 => {
            let ed = app.editor.as_mut().expect("editor open");
            ed.fit_width();
            ed.scroll_to = Some(1);
            ed.set_tool(Tool::Text);
        }
        21 => {
            let ed = app.editor.as_ref().expect("editor open");
            let p = ed.page_to_screen(1, [300.0, 330.0]);
            t.pencil(&[(p, 0.3)], true);
            t.after = Some(Box::new(|_app: &mut NotebookApp, t: &mut AutoTest| {
                t.pending.push(Event::Text(
                    "Typed with the on-screen keyboard. Wraps inside its box.".into(),
                ));
            }));
        }
        22 => shot(ctx, &dir, "13-text-editing.png"),
        23 => {
            let ed = app.editor.as_mut().expect("editor open");
            ed.finish_text();
            let has = ed.doc.pages[1]
                .items
                .iter()
                .any(|i| matches!(&i.content, Content::Text(x) if x.text.starts_with("Typed")));
            println!("CHECK text-tool {}", if has { "PASS" } else { "FAIL" });
            t.count = ed.doc.pages[1].items.len();
            // Two-finger tap → undo (removes the text box).
            let c = ed.canvas.center();
            t.touch(11, TouchPhase::Start, c, None);
            t.touch(12, TouchPhase::Start, c + egui::vec2(60.0, 0.0), None);
            t.touch(11, TouchPhase::End, c, None);
            t.touch(12, TouchPhase::End, c + egui::vec2(60.0, 0.0), None);
        }
        24 => {
            let ed = app.editor.as_mut().expect("editor open");
            let n = ed.doc.pages[1].items.len();
            let gone = !ed.doc.pages[1]
                .items
                .iter()
                .any(|i| matches!(&i.content, Content::Text(_)));
            println!(
                "CHECK two-finger-undo {} ({} -> {n})",
                if n + 1 == t.count && gone {
                    "PASS"
                } else {
                    "FAIL"
                },
                t.count
            );
            app.prefs.eraser_precise = true;
            ed.set_tool(Tool::Eraser);
            t.count = n;
        }
        25 => {
            // Erase a gap through the middle of the loops stroke.
            let ed = app.editor.as_ref().expect("editor open");
            let pts: Vec<(Pos2, f32)> = (0..=20)
                .map(|i| (ed.page_to_screen(1, [250.0, 95.0 + i as f32 * 3.0]), 0.3))
                .collect();
            t.pencil(&pts, true);
        }
        26 => {
            let ed = app.editor.as_mut().expect("editor open");
            let n = ed.doc.pages[1].items.len();
            println!(
                "CHECK precise-eraser-splits {} ({} -> {n})",
                if n == t.count + 1 { "PASS" } else { "FAIL" },
                t.count
            );
            shot(ctx, &dir, "14-erased.png");
            ed.set_tool(Tool::Pen);
            ed.toggle_ruler();
            if let Some(r) = &mut ed.ruler {
                r.angle = 0.3;
            }
            t.count = n;
        }
        27 => {
            // A wobbly stroke along the ruler's lower edge becomes a straight line.
            let ed = app.editor.as_ref().expect("editor open");
            let r = ed.ruler.expect("ruler");
            let (d, nrm) = (r.dir(), r.normal());
            let pts: Vec<(Pos2, f32)> = (0..=30)
                .map(|i| {
                    let along = -150.0 + i as f32 * 10.0;
                    let off = crate::editor::RULER_HALF + 4.0 + (i as f32).sin() * 3.0;
                    (
                        ed.doc_to_screen([
                            r.center[0] + d[0] * along + nrm[0] * off,
                            r.center[1] + d[1] * along + nrm[1] * off,
                        ]),
                        0.4,
                    )
                })
                .collect();
            t.pencil(&pts, true);
        }
        28 => {
            let ed = app.editor.as_ref().expect("editor open");
            let straight = ed
                .doc
                .pages
                .iter()
                .flat_map(|p| p.items.iter())
                .last()
                .is_some_and(|i| matches!(&i.content, Content::Stroke(s) if s.points.len() == 2));
            println!(
                "CHECK ruler-straight-line {}",
                if straight { "PASS" } else { "FAIL" }
            );
            shot(ctx, &dir, "15-ruler.png");
        }
        29 => {
            let ed = app.editor.as_mut().expect("editor open");
            ed.ruler = None;
            ed.events.push(crate::editor::EditorEvent::ExportPdf);
            wait = 1.0;
        }
        30 => {
            let dir_exports = std::env::var("NOTEBOOK_DATA_DIR")
                .map(|d| std::path::PathBuf::from(d).join("exports/Exports"))
                .unwrap_or_default();
            let pdf = std::fs::read_dir(&dir_exports).ok().and_then(|mut it| {
                it.find_map(|e| {
                    e.ok()
                        .filter(|e| e.path().extension().is_some_and(|x| x == "pdf"))
                })
            });
            let ok = pdf.as_ref().is_some_and(|e| {
                std::fs::read(e.path())
                    .map(|b| b.starts_with(b"%PDF"))
                    .unwrap_or(false)
            });
            println!("CHECK pdf-export {}", if ok { "PASS" } else { "FAIL" });
            let n = app
                .editor
                .as_ref()
                .map_or(0, |e| e.doc.pages[1].items.len());
            t.count = n;
            app.close_editor();
            wait = 2.0;
        }
        31 => shot(ctx, &dir, "11-library-after.png"),
        32 => {
            app.prefs.dark_ui = false;
        }
        33 => shot(ctx, &dir, "12-light.png"),
        34 => {
            let id = app
                .lib
                .notebooks
                .iter()
                .find(|m| !m.trashed())
                .map(|m| m.id)
                .expect("notebook");
            let doc = app.store.load_doc(id).expect("load");
            let n = doc.pages[1].items.len();
            println!(
                "CHECK saved-to-disk page2-items={n} {}",
                if n == t.count && n >= 4 {
                    "PASS"
                } else {
                    "FAIL"
                }
            );
            app.prefs.dark_ui = true;
            app.open_new_dialog();
        }
        35 => shot(ctx, &dir, "16-new-notebook.png"),
        _ => {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            app.autotest = None;
            return;
        }
    }
    t.step += 1;
    t.next_at = now + wait;
    app.autotest = Some(t);
}
